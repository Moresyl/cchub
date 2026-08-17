use crate::commands::extended_compat::ModelsDevSyncConfig;
use crate::commands::model_pricing_file::LocalModelPricingEntry;
use crate::db::DbState;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;
use tauri::State;

const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const SYNC_INTERVAL_MS: i64 = 6 * 60 * 60 * 1000;
const MAX_CATALOG_ENTRIES: usize = 8_000;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
static SYNC_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsDevCatalogEntry {
    pub key: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model_id: String,
    pub model_name: String,
    pub release_date: String,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsDevSyncResult {
    pub skipped: bool,
    pub selected: usize,
    pub imported: usize,
    pub changed: usize,
    pub synced_at: Option<i64>,
}

fn normalize_model_id(model_id: &str) -> String {
    let after_slash = model_id.rsplit('/').next().unwrap_or(model_id);
    let before_colon = after_slash.split(':').next().unwrap_or(after_slash);
    let without_long_context = before_colon.strip_suffix("[1m]").unwrap_or(before_colon);
    without_long_context
        .trim()
        .replace('@', "-")
        .to_ascii_lowercase()
}

fn is_text_model(model_id: &str, model: &Value) -> bool {
    if model
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status.eq_ignore_ascii_case("deprecated"))
    {
        return false;
    }
    let output_modalities = model
        .get("modalities")
        .and_then(|value| value.get("output"))
        .and_then(Value::as_array);
    if let Some(modalities) = output_modalities {
        let normalized = modalities
            .iter()
            .filter_map(Value::as_str)
            .map(|value| value.to_ascii_lowercase())
            .collect::<Vec<_>>();
        if !normalized.is_empty()
            && (!normalized.iter().any(|value| value == "text")
                || normalized
                    .iter()
                    .any(|value| matches!(value.as_str(), "audio" | "image" | "video")))
        {
            return false;
        }
    }
    let searchable = format!(
        "{} {}",
        model_id,
        model
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
    )
    .to_ascii_lowercase();
    ![
        "audio",
        "deprecated",
        "embedding",
        "image",
        "moderation",
        "realtime",
        "transcribe",
        "tts",
        "video",
    ]
    .iter()
    .any(|marker| searchable.contains(marker))
}

fn finite_cost(value: Option<&Value>) -> Option<f64> {
    let value = value.and_then(Value::as_f64)?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn parse_catalog(payload: &Value) -> Vec<ModelsDevCatalogEntry> {
    let mut entries = Vec::new();
    let Some(providers) = payload.as_object() else {
        return entries;
    };
    for (provider_id, provider) in providers {
        let provider_name = provider
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(provider_id)
            .to_string();
        let Some(models) = provider.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (model_id, model) in models {
            if !is_text_model(model_id, model) {
                continue;
            }
            let Some(cost) = model.get("cost") else {
                continue;
            };
            let input = finite_cost(cost.get("input"));
            let output = finite_cost(cost.get("output"));
            if input.is_none() && output.is_none() {
                continue;
            }
            let normalized_id = normalize_model_id(model_id);
            if normalized_id.is_empty() {
                continue;
            }
            entries.push(ModelsDevCatalogEntry {
                key: format!("{provider_id}/{model_id}"),
                provider_id: provider_id.clone(),
                provider_name: provider_name.clone(),
                model_id: model_id.clone(),
                model_name: model
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(model_id)
                    .to_string(),
                release_date: model
                    .get("release_date")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                input: input.unwrap_or(0.0),
                output: output.unwrap_or(0.0),
                cache_read: finite_cost(cost.get("cache_read")).unwrap_or(0.0),
                cache_write: finite_cost(cost.get("cache_write")).unwrap_or(0.0),
            });
        }
    }
    entries.sort_by(|left, right| {
        right
            .release_date
            .cmp(&left.release_date)
            .then_with(|| left.model_name.cmp(&right.model_name))
    });
    entries.truncate(MAX_CATALOG_ENTRIES);
    entries
}

fn common_model_keys(entries: &[ModelsDevCatalogEntry]) -> HashSet<String> {
    let rules: [(&str, &[&str]); 11] = [
        ("anthropic", &["claude-"]),
        ("openai", &["gpt-", "o1-", "o3-", "o4-"]),
        ("google", &["gemini-"]),
        ("xai", &["grok-"]),
        ("deepseek", &["deepseek-"]),
        ("alibaba", &["qwen"]),
        ("xiaomi", &["mimo-"]),
        ("longcat", &["longcat-"]),
        ("moonshotai", &["kimi-"]),
        ("minimax-cn", &["minimax-m"]),
        ("zai", &["glm-"]),
    ];
    let mut result = HashSet::new();
    for (provider, prefixes) in rules {
        let mut count = 0;
        for entry in entries {
            if entry.provider_id == provider
                && prefixes
                    .iter()
                    .any(|prefix| entry.model_id.to_ascii_lowercase().starts_with(prefix))
            {
                result.insert(entry.key.clone());
                count += 1;
                if count == 6 {
                    break;
                }
            }
        }
    }
    result
}

fn selected_entries(
    entries: Vec<ModelsDevCatalogEntry>,
    config: &ModelsDevSyncConfig,
) -> Vec<ModelsDevCatalogEntry> {
    let explicit = config
        .selected_model_keys
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let excluded = config
        .excluded_common_model_keys
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let common = common_model_keys(&entries);
    entries
        .into_iter()
        .filter(|entry| {
            explicit.contains(entry.key.as_str())
                || (config.include_common_models
                    && common.contains(&entry.key)
                    && !excluded.contains(entry.key.as_str()))
        })
        .collect()
}

fn read_config(conn: &Connection) -> ModelsDevSyncConfig {
    let config = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'models_dev_sync_config'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    normalize_config(config)
}

fn normalize_config(mut config: ModelsDevSyncConfig) -> ModelsDevSyncConfig {
    for values in [
        &mut config.selected_model_keys,
        &mut config.excluded_common_model_keys,
    ] {
        values.retain(|value| !value.trim().is_empty());
        values
            .iter_mut()
            .for_each(|value| *value = value.trim().to_string());
        values.sort();
        values.dedup();
    }
    config.last_sync_error = config.last_sync_error.and_then(|error| {
        let error = error.trim();
        (!error.is_empty()).then(|| error.chars().take(1_000).collect())
    });
    config
}

fn write_config(conn: &Connection, config: &ModelsDevSyncConfig) -> Result<(), String> {
    let payload = serde_json::to_string(config).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('models_dev_sync_config', ?1)",
        [payload],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn mark_sync_error(conn: &Connection, mut config: ModelsDevSyncConfig, error: &str) {
    config.last_sync_error = Some(error.chars().take(1_000).collect());
    let _ = write_config(conn, &config);
}

fn upsert_pricing(
    conn: &mut Connection,
    entries: &[ModelsDevCatalogEntry],
) -> Result<usize, String> {
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    let now = Utc::now().to_rfc3339();
    let mut changed = 0;
    let mut seen = HashSet::new();
    for entry in entries {
        let model_id = normalize_model_id(&entry.model_id);
        if model_id.is_empty() || !seen.insert(model_id.clone()) {
            continue;
        }
        let values = [
            format_cost(entry.input),
            format_cost(entry.output),
            format_cost(entry.cache_read),
            format_cost(entry.cache_write),
        ];
        let previous = transaction
            .query_row(
                "SELECT input_cost_per_million, output_cost_per_million, cache_read_cost_per_million, cache_write_cost_per_million FROM model_pricing WHERE model_id = ?1",
                [&model_id],
                |row| {
                    Ok([
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ])
                },
            )
            .ok();
        if previous.as_ref() != Some(&values) {
            changed += 1;
        }
        transaction
            .execute(
                "INSERT INTO model_pricing (model_id, normalized_model_id, input_cost_per_million, output_cost_per_million, cache_read_cost_per_million, cache_write_cost_per_million, created_at, updated_at)
                 VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?6)
                 ON CONFLICT(model_id) DO UPDATE SET normalized_model_id = excluded.normalized_model_id, input_cost_per_million = excluded.input_cost_per_million, output_cost_per_million = excluded.output_cost_per_million, cache_read_cost_per_million = excluded.cache_read_cost_per_million, cache_write_cost_per_million = excluded.cache_write_cost_per_million, updated_at = excluded.updated_at",
                rusqlite::params![model_id, values[0], values[1], values[2], values[3], now],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(changed)
}

fn format_cost(value: f64) -> String {
    format!("{value:.6}")
}

async fn fetch_catalog() -> Result<Vec<ModelsDevCatalogEntry>, String> {
    let response = crate::shared::http_client::build_http_client(
        None,
        Some("CCHub"),
        Duration::from_secs(15),
    )?
    .get(MODELS_DEV_URL)
    .send()
    .await
    .map_err(|error| format!("models.dev request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("models.dev returned HTTP {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("models.dev response is too large".to_string());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("models.dev response could not be read: {error}"))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("models.dev response is too large".to_string());
    }
    let payload = serde_json::from_slice::<Value>(&bytes)
        .map_err(|error| format!("models.dev response was invalid JSON: {error}"))?;
    Ok(parse_catalog(&payload))
}

#[tauri::command]
pub async fn get_models_dev_catalog() -> Result<Vec<ModelsDevCatalogEntry>, String> {
    fetch_catalog().await
}

#[tauri::command]
pub async fn sync_models_dev_pricing(
    force: bool,
    db: State<'_, DbState>,
) -> Result<ModelsDevSyncResult, String> {
    let _sync_guard = SYNC_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let config = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_config(&conn)
    };
    let now = Utc::now().timestamp_millis();
    if !force
        && (!config.auto_sync_enabled
            || config
                .last_sync_at
                .is_some_and(|last| now.saturating_sub(last) < SYNC_INTERVAL_MS))
    {
        return Ok(ModelsDevSyncResult {
            skipped: true,
            selected: 0,
            imported: 0,
            changed: 0,
            synced_at: config.last_sync_at,
        });
    }

    let catalog = match fetch_catalog().await {
        Ok(catalog) => catalog,
        Err(error) => {
            let conn = db.0.lock().map_err(|value| value.to_string())?;
            mark_sync_error(&conn, config, &error);
            return Err(error);
        }
    };
    let selected = selected_entries(catalog, &config);
    let selected_count = selected.len();
    let changed = {
        let mut conn = db.0.lock().map_err(|error| error.to_string())?;
        let changed = upsert_pricing(&mut conn, &selected)?;
        crate::commands::model_pricing_file::save_overrides(
            &mut conn,
            selected
                .iter()
                .map(|entry| LocalModelPricingEntry {
                    model_id: normalize_model_id(&entry.model_id),
                    display_name: Some(entry.model_name.clone()),
                    input_cost_per_million: format_cost(entry.input),
                    output_cost_per_million: format_cost(entry.output),
                    cache_read_cost_per_million: format_cost(entry.cache_read),
                    cache_write_cost_per_million: format_cost(entry.cache_write),
                })
                .collect(),
        )?;
        let mut next_config = config;
        next_config.last_sync_at = Some(now);
        next_config.last_sync_error = None;
        write_config(&conn, &next_config)?;
        changed
    };
    Ok(ModelsDevSyncResult {
        skipped: false,
        selected: selected_count,
        imported: selected_count,
        changed,
        synced_at: Some(now),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_non_text_models_and_normalizes_ids() {
        let payload = serde_json::json!({
            "demo": {"name": "Demo", "models": {
                "vendor/Model@low": {"name": "Text", "cost": {"input": 1.0, "output": 2.0}},
                "demo-image": {"name": "Image", "cost": {"input": 1.0, "output": 2.0}, "modalities": {"output": ["image"]}},
                "demo-embedding": {"name": "Embedding", "cost": {"input": 1.0}}
            }}
        });
        let entries = parse_catalog(&payload);
        assert_eq!(entries.len(), 1);
        assert_eq!(normalize_model_id(&entries[0].model_id), "model-low");
    }

    #[test]
    fn common_selection_is_bounded_and_respects_exclusions() {
        let entries = (0..8)
            .map(|index| ModelsDevCatalogEntry {
                key: format!("openai/gpt-{index}"),
                provider_id: "openai".to_string(),
                provider_name: "OpenAI".to_string(),
                model_id: format!("gpt-{index}"),
                model_name: format!("GPT {index}"),
                release_date: format!("2026-01-{index:02}"),
                input: 1.0,
                output: 2.0,
                cache_read: 0.0,
                cache_write: 0.0,
            })
            .collect::<Vec<_>>();
        let config = ModelsDevSyncConfig {
            excluded_common_model_keys: vec!["openai/gpt-7".to_string()],
            ..Default::default()
        };
        let selected = selected_entries(entries, &config);
        assert_eq!(selected.len(), 6);
        assert!(!selected.iter().any(|entry| entry.key == "openai/gpt-7"));
    }
}
