use super::*;

// Both automatic and manual transfers finish here with the settings captured
// after acquiring the workflow lock, not a later account read by the IPC layer.
pub(super) fn finish_transfer<T>(
    db: &DbState,
    expected: &S3SyncSettings,
    result: Result<T, String>,
) -> Result<T, String> {
    match &result {
        Ok(_) => crate::cloud_http::complete(&credential_scope(expected)),
        Err(error) => {
            if let Ok(conn) = db.0.lock() {
                // Preserve the original transfer error even if status storage fails.
                let _ = update_transfer_status(&conn, expected, None, Some(error.clone()));
            }
        }
    }
    result
}

pub(super) fn update_upload_status(
    conn: &rusqlite::Connection,
    expected: &S3SyncSettings,
    synced_at: String,
) -> Result<(), String> {
    update_transfer_status(conn, expected, Some(synced_at), None)
}

pub(super) fn update_transfer_status(
    conn: &rusqlite::Connection,
    expected: &S3SyncSettings,
    synced_at: Option<String>,
    error: Option<String>,
) -> Result<(), String> {
    let mut current: S3SyncSettings = get_json_app_setting(conn, SETTINGS_KEY)?.unwrap_or_default();
    current.normalize();
    if same_remote(&current, expected) {
        if let Some(synced_at) = synced_at {
            current.last_sync_at = Some(synced_at);
        }
        current.last_error = error;
        set_json_app_setting(conn, SETTINGS_KEY, &current.masked_for_frontend())?;
    }
    Ok(())
}
