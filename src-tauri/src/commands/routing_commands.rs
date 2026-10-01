use crate::db::DbState;
use crate::provider_proxy::routing::{self, RoutingDocument, RoutingPolicy, RoutingPreview};
use tauri::State;

#[tauri::command]
pub fn get_provider_routing(
    app_type: String,
    db: State<'_, DbState>,
) -> Result<RoutingDocument, String> {
    let conn = db.0.lock().map_err(|_| "Database lock failed")?;
    routing::load(&conn, &app_type)
}

#[tauri::command]
pub fn set_provider_routing(
    app_type: String,
    expected_revision: Option<String>,
    policy: RoutingPolicy,
    db: State<'_, DbState>,
) -> Result<RoutingDocument, String> {
    let conn = db.0.lock().map_err(|_| "Database lock failed")?;
    routing::save(&conn, &app_type, expected_revision.as_deref(), policy)
}

#[tauri::command]
pub fn preview_provider_routing(
    app_type: String,
    policy: RoutingPolicy,
    request: serde_json::Value,
    relative_path: Option<String>,
    request_bytes: Option<u64>,
    db: State<'_, DbState>,
) -> Result<RoutingPreview, String> {
    let conn = db.0.lock().map_err(|_| "Database lock failed")?;
    routing::preview(
        &conn,
        &app_type,
        policy,
        request,
        relative_path.as_deref().unwrap_or(""),
        request_bytes,
    )
}
