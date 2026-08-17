use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const MODEL_PRICING_FILE_VERSION: u32 = 1;
const MODEL_PRICING_FILE_NAME: &str = "model-pricing.json";

static FILE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn file_lock() -> &'static Mutex<()> {
    FILE_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelPricingEntry {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub input_cost_per_million: String,
    pub output_cost_per_million: String,
    pub cache_read_cost_per_million: String,
    #[serde(alias = "cacheCreationCostPerMillion")]
    pub cache_write_cost_per_million: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalModelPricingFile {
    #[serde(default = "default_file_version")]
    version: u32,
    #[serde(default)]
    models: Vec<LocalModelPricingEntry>,
    #[serde(default)]
    deleted_model_ids: Vec<String>,
}

impl Default for LocalModelPricingFile {
    fn default() -> Self {
        Self {
            version: MODEL_PRICING_FILE_VERSION,
            models: Vec::new(),
            deleted_model_ids: Vec::new(),
        }
    }
}

fn default_file_version() -> u32 {
    MODEL_PRICING_FILE_VERSION
}

pub fn model_pricing_file_path() -> PathBuf {
    std::env::var_os("CCHUB_MODEL_PRICING_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::utils::cchub_state_dir().join(MODEL_PRICING_FILE_NAME))
}

fn normalize_cost(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok("0.000000".to_string());
    }
    let parsed = trimmed
        .parse::<f64>()
        .map_err(|_| format!("Invalid cost value: {trimmed}"))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(format!("Invalid cost value: {trimmed}"));
    }
    Ok(format!("{parsed:.6}"))
}

fn normalize_model_id(model_id: &str) -> String {
    let normalized = model_id.trim().to_ascii_lowercase().replace('@', "-");
    let without_prefix = normalized.strip_prefix("models/").unwrap_or(&normalized);
    without_prefix
        .rsplit('/')
        .next()
        .unwrap_or(without_prefix)
        .split(':')
        .next()
        .unwrap_or(without_prefix)
        .trim()
        .to_string()
}

fn normalize_entry(mut entry: LocalModelPricingEntry) -> Result<LocalModelPricingEntry, String> {
    entry.model_id = entry.model_id.trim().to_string();
    if entry.model_id.is_empty() {
        return Err("Model ID is required".to_string());
    }
    entry.display_name = entry
        .display_name
        .and_then(|value| (!value.trim().is_empty()).then(|| value.trim().to_string()));
    entry.input_cost_per_million = normalize_cost(&entry.input_cost_per_million)?;
    entry.output_cost_per_million = normalize_cost(&entry.output_cost_per_million)?;
    entry.cache_read_cost_per_million = normalize_cost(&entry.cache_read_cost_per_million)?;
    entry.cache_write_cost_per_million = normalize_cost(&entry.cache_write_cost_per_million)?;
    Ok(entry)
}

fn normalize_ids(values: Vec<String>) -> BTreeSet<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_file(mut file: LocalModelPricingFile) -> Result<LocalModelPricingFile, String> {
    if file.version > MODEL_PRICING_FILE_VERSION {
        return Err(format!(
            "model-pricing.json version {} is newer than supported version {}",
            file.version, MODEL_PRICING_FILE_VERSION
        ));
    }
    let deleted = normalize_ids(file.deleted_model_ids);
    let mut models = BTreeMap::new();
    for entry in file.models {
        let entry = normalize_entry(entry)?;
        if !deleted.contains(&entry.model_id) {
            models.insert(entry.model_id.clone(), entry);
        }
    }
    file.version = MODEL_PRICING_FILE_VERSION;
    file.models = models.into_values().collect();
    file.deleted_model_ids = deleted.into_iter().collect();
    Ok(file)
}

fn read_file_unlocked() -> Result<Option<LocalModelPricingFile>, String> {
    let path = model_pricing_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "Failed to read model pricing file {}: {error}",
            path.display()
        )
    })?;
    let file = serde_json::from_str::<LocalModelPricingFile>(&content)
        .map_err(|error| format!("Invalid model pricing file {}: {error}", path.display()))?;
    normalize_file(file).map(Some)
}

fn write_file_unlocked(file: &LocalModelPricingFile) -> Result<(), String> {
    let path = model_pricing_file_path();
    let parent = path
        .parent()
        .ok_or_else(|| "Model pricing file has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut payload = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    payload.push('\n');
    crate::utils::atomic_write_string(&path, &payload).map_err(|error| error.to_string())
}

fn load_or_create_file_unlocked() -> Result<LocalModelPricingFile, String> {
    if let Some(file) = read_file_unlocked()? {
        return Ok(file);
    }
    let file = LocalModelPricingFile::default();
    write_file_unlocked(&file)?;
    Ok(file)
}

fn apply_file_to_database(
    conn: &mut Connection,
    file: &LocalModelPricingFile,
) -> Result<usize, String> {
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    let mut changed = 0;
    for entry in &file.models {
        changed += transaction
            .execute(
                "INSERT INTO model_pricing (
                    model_id, normalized_model_id, input_cost_per_million,
                    output_cost_per_million, cache_read_cost_per_million,
                    cache_write_cost_per_million, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE((SELECT created_at FROM model_pricing WHERE model_id = ?1), ?7), ?7)
                 ON CONFLICT(model_id) DO UPDATE SET
                    normalized_model_id = excluded.normalized_model_id,
                    input_cost_per_million = excluded.input_cost_per_million,
                    output_cost_per_million = excluded.output_cost_per_million,
                    cache_read_cost_per_million = excluded.cache_read_cost_per_million,
                    cache_write_cost_per_million = excluded.cache_write_cost_per_million,
                    updated_at = excluded.updated_at
                 WHERE input_cost_per_million <> excluded.input_cost_per_million
                    OR output_cost_per_million <> excluded.output_cost_per_million
                    OR cache_read_cost_per_million <> excluded.cache_read_cost_per_million
                    OR cache_write_cost_per_million <> excluded.cache_write_cost_per_million",
                rusqlite::params![
                    entry.model_id,
                    normalize_model_id(&entry.model_id),
                    entry.input_cost_per_million,
                    entry.output_cost_per_million,
                    entry.cache_read_cost_per_million,
                    entry.cache_write_cost_per_million,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    for model_id in &file.deleted_model_ids {
        changed += transaction
            .execute("DELETE FROM model_pricing WHERE model_id = ?1", [model_id])
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(changed)
}

pub fn sync_local_model_pricing(conn: &mut Connection) -> Result<usize, String> {
    let _guard = file_lock()
        .lock()
        .map_err(|error| format!("Model pricing file lock failed: {error}"))?;
    let file = load_or_create_file_unlocked()?;
    apply_file_to_database(conn, &file)
}

pub fn save_overrides(
    conn: &mut Connection,
    entries: Vec<LocalModelPricingEntry>,
) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    let entries = entries
        .into_iter()
        .map(normalize_entry)
        .collect::<Result<Vec<_>, _>>()?;
    let _guard = file_lock()
        .lock()
        .map_err(|error| format!("Model pricing file lock failed: {error}"))?;
    let mut file = load_or_create_file_unlocked()?;
    let mut models = file
        .models
        .into_iter()
        .map(|item| (item.model_id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    for entry in &entries {
        models.insert(entry.model_id.clone(), entry.clone());
        file.deleted_model_ids.retain(|id| id != &entry.model_id);
    }
    file.models = models.into_values().collect();
    write_file_unlocked(&file)?;
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    for entry in &entries {
        transaction
            .execute(
                "INSERT INTO model_pricing (
                    model_id, normalized_model_id, input_cost_per_million,
                    output_cost_per_million, cache_read_cost_per_million,
                    cache_write_cost_per_million, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE((SELECT created_at FROM model_pricing WHERE model_id = ?1), ?7), ?7)
                 ON CONFLICT(model_id) DO UPDATE SET
                    input_cost_per_million = excluded.input_cost_per_million,
                    output_cost_per_million = excluded.output_cost_per_million,
                    cache_read_cost_per_million = excluded.cache_read_cost_per_million,
                    cache_write_cost_per_million = excluded.cache_write_cost_per_million,
                    updated_at = excluded.updated_at",
                rusqlite::params![
                    entry.model_id,
                    normalize_model_id(&entry.model_id),
                    entry.input_cost_per_million,
                    entry.output_cost_per_million,
                    entry.cache_read_cost_per_million,
                    entry.cache_write_cost_per_million,
                    now,
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(())
}

pub fn save_override(conn: &mut Connection, entry: LocalModelPricingEntry) -> Result<(), String> {
    save_overrides(conn, vec![entry])
}

pub fn delete_override(conn: &mut Connection, model_id: &str) -> Result<(), String> {
    let model_id = model_id.trim();
    if model_id.is_empty() {
        return Err("Model ID is required".to_string());
    }
    let _guard = file_lock()
        .lock()
        .map_err(|error| format!("Model pricing file lock failed: {error}"))?;
    let mut file = load_or_create_file_unlocked()?;
    file.models.retain(|entry| entry.model_id != model_id);
    if !file.deleted_model_ids.iter().any(|id| id == model_id) {
        file.deleted_model_ids.push(model_id.to_string());
        file.deleted_model_ids.sort();
    }
    write_file_unlocked(&file)?;
    conn.execute("DELETE FROM model_pricing WHERE model_id = ?1", [model_id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_duplicate_entries_and_tombstones() {
        let file = LocalModelPricingFile {
            version: 0,
            models: vec![
                LocalModelPricingEntry {
                    model_id: " demo ".to_string(),
                    display_name: None,
                    input_cost_per_million: "1".to_string(),
                    output_cost_per_million: "2".to_string(),
                    cache_read_cost_per_million: "".to_string(),
                    cache_write_cost_per_million: "3".to_string(),
                },
                LocalModelPricingEntry {
                    model_id: "demo".to_string(),
                    display_name: None,
                    input_cost_per_million: "4".to_string(),
                    output_cost_per_million: "5".to_string(),
                    cache_read_cost_per_million: "6".to_string(),
                    cache_write_cost_per_million: "7".to_string(),
                },
            ],
            deleted_model_ids: vec!["deleted".to_string(), " demo ".to_string()],
        };
        let normalized = normalize_file(file).expect("normalize pricing file");
        assert!(normalized.models.is_empty());
        assert_eq!(normalized.deleted_model_ids, vec!["deleted", "demo"]);
        assert_eq!(normalize_cost("1.25").unwrap(), "1.250000");
    }

    #[test]
    fn rejects_future_file_versions_and_invalid_costs() {
        let file = LocalModelPricingFile {
            version: MODEL_PRICING_FILE_VERSION + 1,
            ..Default::default()
        };
        assert!(normalize_file(file).is_err());
        assert!(normalize_cost("-1").is_err());
        assert!(normalize_cost("nan").is_err());
    }
}
