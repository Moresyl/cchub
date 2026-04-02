use tauri::State;

use crate::db::DbState;
use crate::webdav_sync::{self, WebDavRemoteInfo, WebDavSyncSettings};

#[tauri::command]
pub fn get_webdav_sync_settings(db: State<'_, DbState>) -> Result<WebDavSyncSettings, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(webdav_sync::read_settings(&conn)?.masked_for_frontend())
}

#[tauri::command]
pub fn set_webdav_sync_settings(
    settings: WebDavSyncSettings,
    password_touched: Option<bool>,
    db: State<'_, DbState>,
) -> Result<WebDavSyncSettings, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    webdav_sync::write_settings(&conn, settings, password_touched.unwrap_or(false))
}

#[tauri::command]
pub async fn webdav_test_connection(
    settings: WebDavSyncSettings,
    preserve_empty_password: Option<bool>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let existing = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        webdav_sync::read_settings(&conn).ok()
    };
    webdav_sync::test_connection(settings, existing, preserve_empty_password.unwrap_or(true)).await
}

#[tauri::command]
pub async fn webdav_sync_fetch_remote_info(
    db: State<'_, DbState>,
) -> Result<WebDavRemoteInfo, String> {
    webdav_sync::fetch_remote_info(&db).await
}

#[tauri::command]
pub async fn webdav_sync_upload(db: State<'_, DbState>) -> Result<WebDavRemoteInfo, String> {
    webdav_sync::upload(&db).await
}

#[tauri::command]
pub async fn webdav_sync_download(db: State<'_, DbState>) -> Result<String, String> {
    webdav_sync::download(&db).await
}
