use super::*;

pub(super) fn update_upload_status(
    conn: &rusqlite::Connection,
    expected: &WebDavSyncSettings,
    synced_at: String,
) -> Result<(), String> {
    update_transfer_status(conn, expected, Some(synced_at), None)
}

pub(super) fn update_transfer_status(
    conn: &rusqlite::Connection,
    expected: &WebDavSyncSettings,
    synced_at: Option<String>,
    error: Option<String>,
) -> Result<(), String> {
    let mut current: WebDavSyncSettings =
        get_json_app_setting(conn, WEBDAV_SYNC_SETTINGS_KEY)?.unwrap_or_default();
    current.normalize();
    if same_remote(&current, expected) {
        if let Some(synced_at) = synced_at {
            current.last_sync_at = Some(synced_at);
        }
        current.last_error = error;
        set_json_app_setting(
            conn,
            WEBDAV_SYNC_SETTINGS_KEY,
            &current.masked_for_frontend(),
        )?;
    }
    Ok(())
}
