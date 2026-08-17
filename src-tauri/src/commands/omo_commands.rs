use serde_json::Value;
use tauri::State;

use crate::db::DbState;
use crate::omo::{self, OmoLocalConfigData};

#[tauri::command(rename_all = "camelCase")]
pub fn omo_read_local_config(
    variant: String,
    db: State<'_, DbState>,
) -> Result<OmoLocalConfigData, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let variant = omo::variant_from_id(&variant)?;
    omo::read_local_config(&conn, variant)
}

#[tauri::command(rename_all = "camelCase")]
pub fn omo_write_local_config(
    variant: String,
    agents: Value,
    categories: Option<Value>,
    other_fields: Option<Value>,
    db: State<'_, DbState>,
) -> Result<OmoLocalConfigData, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let variant = omo::variant_from_id(&variant)?;
    omo::write_local_config(&conn, variant, agents, categories, other_fields)
}

#[tauri::command(rename_all = "camelCase")]
pub fn disable_current_omo(variant: String, db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    omo::disable_local_plugin(&conn, omo::variant_from_id(&variant)?)
}

#[tauri::command]
pub fn disable_current_omo_slim(db: State<'_, DbState>) -> Result<bool, String> {
    disable_current_omo("slim".to_string(), db)
}

fn current_omo_provider_id(variant: &str, db: &DbState) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let config = omo::read_local_config(&conn, omo::variant_from_id(variant)?)?;
    Ok(config
        .agents
        .as_object()
        .and_then(|agents| agents.keys().next().cloned()))
}

#[tauri::command]
pub fn get_current_omo_provider_id(db: State<'_, DbState>) -> Result<Option<String>, String> {
    current_omo_provider_id("standard", db.inner())
}

#[tauri::command]
pub fn get_current_omo_slim_provider_id(db: State<'_, DbState>) -> Result<Option<String>, String> {
    current_omo_provider_id("slim", db.inner())
}

#[tauri::command(rename_all = "camelCase")]
pub fn read_omo_local_file(
    variant: String,
    db: State<'_, DbState>,
) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let config = omo::read_local_config(&conn, omo::variant_from_id(&variant)?)?;
    if config.file_path.trim().is_empty() || !std::path::Path::new(&config.file_path).exists() {
        return Ok(None);
    }
    std::fs::read_to_string(config.file_path)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_omo_slim_local_file(db: State<'_, DbState>) -> Result<Option<String>, String> {
    read_omo_local_file("slim".to_string(), db)
}
