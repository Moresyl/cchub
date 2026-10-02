use super::*;

pub fn spawn_auto_sync_loop(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(AUTO_SYNC_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            let event = match super::run_auto_sync_if_enabled(&app_handle).await {
                Ok(event) => event,
                Err(error) => Some(WebDavSyncEvent {
                    status: "error".to_string(),
                    message: "Automatic WebDAV sync failed".to_string(),
                    synced_at: None,
                    error: Some(error),
                }),
            };
            if let Some(payload) = event {
                let _ = app_handle.emit("webdav-sync-status-updated", &payload);
            }
        }
    });
}

pub async fn run_auto_sync_if_enabled(
    app_handle: &AppHandle,
) -> Result<Option<WebDavSyncEvent>, String> {
    let db = app_handle.state::<DbState>();
    match super::auto_upload(&db).await {
        Ok(None) => Ok(None),
        Ok(Some(info)) => Ok(Some(WebDavSyncEvent {
            status: "success".to_string(),
            message: "Automatic WebDAV sync completed".to_string(),
            synced_at: info.updated_at,
            error: None,
        })),
        Err(error) => Ok(Some(WebDavSyncEvent {
            status: "error".to_string(),
            message: "Automatic WebDAV sync failed".to_string(),
            synced_at: None,
            error: Some(error),
        })),
    }
}

pub(crate) async fn auto_upload(
    db: &State<'_, DbState>,
) -> Result<Option<WebDavRemoteInfo>, String> {
    let _guard = webdav_sync_lock().lock().await;
    let _workflow = crate::cloud_sync::workflow_lock().lock().await;
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let mut current: WebDavSyncSettings =
            get_json_app_setting(&conn, WEBDAV_SYNC_SETTINGS_KEY)?.unwrap_or_default();
        current.normalize();
        // Decide at this task's turn, before loading secrets or sending requests.
        if !current.enabled
            || !current.auto_sync
            || crate::cloud_http::remaining(&credential_scope(&current)).is_some()
        {
            return Ok(None);
        }
        read_settings(&conn)?
    };
    upload_for_settings(db, &settings, None).await.map(Some)
}
