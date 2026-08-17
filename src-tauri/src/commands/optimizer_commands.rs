use tauri::{AppHandle, State};

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
    app_handle: AppHandle,
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
    crate::provider_proxy::update_optimizer_config_cache(&app_handle, config);
    Ok(())
}

#[tauri::command]
pub fn get_copilot_optimizer_config(db: State<'_, DbState>) -> Result<OptimizerConfig, String> {
    get_optimizer_config(db)
}

#[tauri::command]
pub fn set_copilot_optimizer_config(
    app_handle: AppHandle,
    db: State<'_, DbState>,
    config: OptimizerConfig,
) -> Result<(), String> {
    set_optimizer_config(app_handle, db, config)
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
    app_handle: AppHandle,
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
    crate::provider_proxy::update_rectifier_config_cache(&app_handle, config);
    Ok(())
}

#[tauri::command]
pub fn get_circuit_breaker_stats(
    app_handle: AppHandle,
) -> Result<crate::provider_proxy::CircuitBreakerStats, String> {
    crate::provider_proxy::get_circuit_breaker_stats(&app_handle)
}

#[tauri::command]
pub fn reset_circuit_breakers(app_handle: AppHandle) -> Result<usize, String> {
    crate::provider_proxy::reset_circuit_breakers(&app_handle)
}

#[tauri::command]
pub fn reset_circuit_breaker(
    app_handle: AppHandle,
    provider_id: String,
    app_type: String,
) -> Result<usize, String> {
    crate::provider_proxy::reset_circuit_breaker_for_profile(&app_handle, &app_type, &provider_id)
}

#[tauri::command]
pub fn get_circuit_breaker_config(db: State<'_, DbState>) -> Result<OptimizerConfig, String> {
    get_optimizer_config(db)
}
