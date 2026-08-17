use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::DbState;

const SETTINGS_KEY: &str = "stream_check_config";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckConfig {
    pub timeout_ms: u64,
    pub retries: u32,
    pub concurrency: u32,
}

impl Default for StreamCheckConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 12_000,
            retries: 1,
            concurrency: 4,
        }
    }
}

fn read_config(db: &DbState) -> Result<StreamCheckConfig, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![SETTINGS_KEY],
            |row| row.get(0),
        )
        .ok();
    Ok(raw
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn get_stream_check_config(db: State<'_, DbState>) -> Result<StreamCheckConfig, String> {
    read_config(db.inner())
}

#[tauri::command]
pub fn save_stream_check_config(
    config: StreamCheckConfig,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    if !(500..=120_000).contains(&config.timeout_ms) {
        return Err("timeoutMs must be between 500 and 120000".to_string());
    }
    if config.retries > 5 {
        return Err("retries must be between 0 and 5".to_string());
    }
    if !(1..=32).contains(&config.concurrency) {
        return Err("concurrency must be between 1 and 32".to_string());
    }
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let value = serde_json::to_string(&config).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![SETTINGS_KEY, value],
    )
    .map_err(|error| error.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::StreamCheckConfig;

    #[test]
    fn defaults_are_conservative() {
        let config = StreamCheckConfig::default();
        assert_eq!(config.timeout_ms, 12_000);
        assert_eq!(config.retries, 1);
        assert_eq!(config.concurrency, 4);
    }
}
