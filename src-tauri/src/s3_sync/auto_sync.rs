use super::*;

pub fn spawn_auto_sync_loop(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            let db = app_handle.state::<DbState>();
            let result = match super::auto_upload(&db).await {
                Ok(None) => continue,
                Ok(Some(info)) => Ok(info),
                Err(error) => Err(error),
            };
            let payload = serde_json::json!({
                "status": if result.is_ok() { "success" } else { "error" },
                "message": result.as_ref().map(|_| "S3 sync completed").unwrap_or("S3 sync failed"),
                "error": result.err(),
            });
            let _ = app_handle.emit("s3-sync-status-updated", payload);
        }
    });
}

pub(crate) async fn auto_upload(db: &State<'_, DbState>) -> Result<Option<S3RemoteInfo>, String> {
    let _guard = sync_lock().lock().await;
    let _workflow = crate::cloud_sync::workflow_lock().lock().await;
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let mut current: S3SyncSettings =
            get_json_app_setting(&conn, SETTINGS_KEY)?.unwrap_or_default();
        current.normalize();
        if !current.enabled
            || !current.auto_sync
            || crate::cloud_http::remaining(&credential_scope(&current)).is_some()
        {
            return Ok(None);
        }
        read_settings(&conn)?
    };
    finish_transfer(db, &settings, upload_inner(db, &settings, None).await).map(Some)
}
