use tauri::State;

use crate::db::DbState;
use crate::s3_sync::{self, S3RemoteInfo, S3SyncSettings};

#[tauri::command]
pub fn get_s3_sync_settings(db: State<'_, DbState>) -> Result<S3SyncSettings, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(s3_sync::read_settings(&conn)?.masked_for_frontend())
}

#[tauri::command]
pub fn set_s3_sync_settings(
    settings: S3SyncSettings,
    secret_touched: Option<bool>,
    db: State<'_, DbState>,
) -> Result<S3SyncSettings, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    s3_sync::write_settings(&conn, settings, secret_touched.unwrap_or(false))
}

#[tauri::command]
pub fn s3_sync_save_settings(
    settings: S3SyncSettings,
    secret_touched: Option<bool>,
    db: State<'_, DbState>,
) -> Result<S3SyncSettings, String> {
    set_s3_sync_settings(settings, secret_touched, db)
}

#[tauri::command]
pub async fn s3_test_connection(
    settings: S3SyncSettings,
    preserve_empty_secret: Option<bool>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let existing = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        s3_sync::read_settings(&conn).ok()
    };
    s3_sync::test_connection(settings, existing, preserve_empty_secret.unwrap_or(true)).await
}

#[tauri::command]
pub async fn s3_sync_fetch_remote_info(db: State<'_, DbState>) -> Result<S3RemoteInfo, String> {
    s3_sync::fetch_remote_info(&db).await
}

#[tauri::command]
pub async fn s3_sync_upload(db: State<'_, DbState>) -> Result<S3RemoteInfo, String> {
    match s3_sync::upload(&db).await {
        Ok(info) => Ok(info),
        Err(error) => {
            if let Ok(conn) = db.0.lock() {
                let _ = s3_sync::update_error(&conn, &error);
            }
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn s3_sync_download(db: State<'_, DbState>) -> Result<String, String> {
    match s3_sync::download(&db).await {
        Ok(message) => Ok(message),
        Err(error) => {
            if let Ok(conn) = db.0.lock() {
                let _ = s3_sync::update_error(&conn, &error);
            }
            Err(error)
        }
    }
}
