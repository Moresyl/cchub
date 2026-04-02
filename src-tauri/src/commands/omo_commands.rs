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
