use crate::commands::extended_compat::ModelsDevSyncConfig;
use crate::commands::model_pricing_file::LocalModelPricingEntry;
use crate::db::DbState;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;
use tauri::State;

pub(super) mod settings;
use settings::{mark_sync_error, read_config, same_preferences, write_config};

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
    pub is_common: bool,
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
                is_common: false,
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
    let common = common_model_keys(&entries);
    for entry in &mut entries {
        entry.is_common = common.contains(&entry.key);
    }
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

fn distinct_entries(entries: Vec<ModelsDevCatalogEntry>) -> Vec<ModelsDevCatalogEntry> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(normalize_model_id(&entry.model_id)))
        .collect()
}

fn count_pricing_changes(
    conn: &Connection,
    entries: &[ModelsDevCatalogEntry],
) -> Result<usize, String> {
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
        let previous = conn
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
            .optional()
            .map_err(|_| "PRICING_READ_FAILED".to_string())?;
        if previous.as_ref() != Some(&values) {
            changed += 1;
        }
    }
    Ok(changed)
}

fn prepare_sync(
    conn: &Connection,
    expected: &ModelsDevSyncConfig,
    catalog: Vec<ModelsDevCatalogEntry>,
) -> Result<(ModelsDevSyncConfig, usize, Vec<ModelsDevCatalogEntry>), String> {
    let current = read_config(conn)?;
    if !same_preferences(&current, expected) {
        return Err("PRICING_SETTINGS_CONFLICT".to_string());
    }
    let selected = selected_entries(catalog, &current);
    let selected_count = selected.len();
    Ok((current, selected_count, distinct_entries(selected)))
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
    expected_config: Option<ModelsDevSyncConfig>,
    db: State<'_, DbState>,
) -> Result<ModelsDevSyncResult, String> {
    let _sync_guard = SYNC_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    let config = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_config(&conn)?
    };
    let now = Utc::now().timestamp_millis();
    if expected_config
        .as_ref()
        .is_some_and(|expected| !same_preferences(&config, expected))
    {
        return Err("PRICING_SETTINGS_CONFLICT".to_string());
    }
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
            let _ = mark_sync_error(&conn);
            return Err(error);
        }
    };
    let (selected_count, imported, changed) = {
        let mut conn = db.0.lock().map_err(|error| error.to_string())?;
        let (current, selected_count, selected) = prepare_sync(&conn, &config, catalog)?;
        let imported = selected.len();
        let changed = count_pricing_changes(&conn, &selected)?;
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
        let mut next_config = current;
        next_config.last_sync_at = Some(now);
        next_config.last_sync_error = None;
        write_config(&conn, &next_config)?;
        (selected_count, imported, changed)
    };
    Ok(ModelsDevSyncResult {
        skipped: false,
        selected: selected_count,
        imported,
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
                is_common: false,
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

    #[test]
    fn catalog_marks_exactly_the_recent_common_models_used_by_sync() {
        let models = (1..=8).map(|index| (
            format!("gpt-{index}"),
            serde_json::json!({"name":format!("GPT {index}"), "release_date":format!("2026-01-{index:02}"), "cost":{"input":1,"output":2}}),
        )).collect::<serde_json::Map<_, _>>();
        let catalog = parse_catalog(&serde_json::json!({"openai":{"models":models}}));
        let marked = catalog
            .iter()
            .filter(|entry| entry.is_common)
            .map(|entry| entry.key.clone())
            .collect::<HashSet<_>>();
        assert_eq!(marked.len(), 6);
        assert!(!marked.contains("openai/gpt-1"));
        assert!(!marked.contains("openai/gpt-2"));
        let actual = selected_entries(catalog.clone(), &ModelsDevSyncConfig::default())
            .into_iter()
            .map(|entry| entry.key)
            .collect::<HashSet<_>>();
        assert_eq!(marked, actual);
        let config = ModelsDevSyncConfig {
            selected_model_keys: vec!["openai/gpt-1".into()],
            excluded_common_model_keys: vec!["openai/gpt-8".into()],
            ..Default::default()
        };
        let selected = selected_entries(catalog.clone(), &config);
        assert_eq!(selected.len(), 6);
        assert!(selected.iter().any(|entry| entry.key == "openai/gpt-1"));
        assert!(!selected.iter().any(|entry| entry.key == "openai/gpt-8"));
        assert_eq!(serde_json::to_value(&catalog[0]).unwrap()["isCommon"], true);
    }

    #[test]
    fn duplicate_price_ids_have_one_deterministic_winner() {
        let catalog = parse_catalog(&serde_json::json!({"demo":{"models":{
            "vendor/model": {"name":"Newer", "release_date":"2026-02-01", "cost":{"input":3,"output":4}},
            "model:low": {"name":"Older", "release_date":"2026-01-01", "cost":{"input":1,"output":2}}
        }}}));
        let distinct = distinct_entries(catalog);
        assert_eq!(distinct.len(), 1);
        assert_eq!(distinct[0].model_name, "Newer");
        assert_eq!(distinct[0].input, 3.0);
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE model_pricing (model_id TEXT PRIMARY KEY, input_cost_per_million TEXT, output_cost_per_million TEXT, cache_read_cost_per_million TEXT, cache_write_cost_per_million TEXT)", []).unwrap();
        assert_eq!(count_pricing_changes(&conn, &distinct).unwrap(), 1);
        assert_eq!(
            conn.query_row("SELECT count(*) FROM model_pricing", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        conn.execute("INSERT INTO model_pricing VALUES ('model','3.000000','4.000000','0.000000','0.000000')", []).unwrap();
        assert_eq!(count_pricing_changes(&conn, &distinct).unwrap(), 0);
        assert!(count_pricing_changes(&Connection::open_in_memory().unwrap(), &distinct).is_err());
    }

    #[test]
    fn sync_preparation_rechecks_preferences_after_the_catalog_request() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();
        let before_request = read_config(&conn).unwrap();
        let changed = ModelsDevSyncConfig {
            include_common_models: false,
            ..before_request.clone()
        };
        write_config(&conn, &changed).unwrap();
        assert_eq!(
            prepare_sync(&conn, &before_request, vec![]).unwrap_err(),
            "PRICING_SETTINGS_CONFLICT"
        );
        let after = read_config(&conn).unwrap();
        assert!(!after.include_common_models);
        let metadata_only = ModelsDevSyncConfig {
            last_sync_at: Some(42),
            ..before_request.clone()
        };
        write_config(&conn, &metadata_only).unwrap();
        let (current, selected, imported) = prepare_sync(&conn, &before_request, vec![]).unwrap();
        assert_eq!(current.last_sync_at, Some(42));
        assert_eq!(selected, 0);
        assert!(imported.is_empty());
    }
}
