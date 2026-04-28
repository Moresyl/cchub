use tauri::State;

use crate::db::DbState;
use crate::proxy_optimizer::config::{
    OptimizerConfig, RectifierConfig, OPTIMIZER_CONFIG_SETTINGS_KEY, RECTIFIER_CONFIG_SETTINGS_KEY,
};

#[tauri::command]
pub fn get_optimizer_config(db: State<'_, DbState>) -> Result<OptimizerConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![OPTIMIZER_CONFIG_SETTINGS_KEY],
            |row| row.get(0),
        )
        .ok();

    Ok(raw
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn set_optimizer_config(
    db: State<'_, DbState>,
    config: OptimizerConfig,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let payload = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![OPTIMIZER_CONFIG_SETTINGS_KEY, payload],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_rectifier_config(db: State<'_, DbState>) -> Result<RectifierConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![RECTIFIER_CONFIG_SETTINGS_KEY],
            |row| row.get(0),
        )
        .ok();

    Ok(raw
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn set_rectifier_config(
    db: State<'_, DbState>,
    config: RectifierConfig,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let payload = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![RECTIFIER_CONFIG_SETTINGS_KEY, payload],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
