use super::{
    runtime::{self, Runtime},
    storage,
    types::*,
};
use crate::db::DbState;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub(crate) fn get_usage_alerts(app: AppHandle) -> Result<AlertOverview, String> {
    let db = app.state::<DbState>();
    let conn =
        db.0.lock()
            .map_err(|_| "Usage alert storage is unavailable".to_string())?;
    let state = storage::load(&conn)?;
    let profiles = storage::profiles(&conn)?;
    Ok(storage::overview(
        &state,
        &profiles,
        app.state::<Arc<Runtime>>().polling(),
    ))
}

#[tauri::command]
pub(crate) fn set_usage_alert_rule(
    app: AppHandle,
    profile_id: String,
    settings: AlertSettings,
    expected_identity: String,
) -> Result<AlertOverview, String> {
    {
        let db = app.state::<DbState>();
        let conn =
            db.0.lock()
                .map_err(|_| "Usage alert storage is unavailable".to_string())?;
        let profiles = storage::profiles(&conn)?;
        let profile = profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or("Configuration no longer exists")?;
        let mut state = storage::load(&conn)?;
        storage::retain_profiles(&mut state, &profiles);
        storage::set_rule_if_current(&mut state, profile, settings, &expected_identity)?;
        storage::save(&conn, &state)?;
    }
    runtime::changed(&app);
    app.state::<Arc<Runtime>>().wake.notify_one();
    get_usage_alerts(app)
}

#[tauri::command]
pub(crate) fn mark_usage_alerts_read(
    app: AppHandle,
    event_id: Option<String>,
) -> Result<AlertOverview, String> {
    {
        let db = app.state::<DbState>();
        let conn =
            db.0.lock()
                .map_err(|_| "Usage alert storage is unavailable".to_string())?;
        let mut state = storage::load(&conn)?;
        for stored in &mut state.events {
            if event_id.as_ref().is_none_or(|id| *id == stored.event.id) {
                stored.event.read = true;
            }
        }
        storage::save(&conn, &state)?;
    }
    runtime::changed(&app);
    get_usage_alerts(app)
}

#[tauri::command]
pub(crate) async fn check_usage_alerts(app: AppHandle) -> Result<AlertOverview, String> {
    runtime::check(&app).await?;
    get_usage_alerts(app)
}

#[tauri::command]
pub(crate) fn retry_usage_alert(app: AppHandle, event_id: String) -> Result<AlertOverview, String> {
    {
        let db = app.state::<DbState>();
        let conn =
            db.0.lock()
                .map_err(|_| "Usage alert storage is unavailable".to_string())?;
        let mut state = storage::load(&conn)?;
        let stored = state
            .events
            .iter_mut()
            .find(|stored| stored.event.id == event_id)
            .ok_or("Notification no longer exists")?;
        if stored.event.system_status != "failed" {
            return Err("Only failed system notifications can be retried".into());
        }
        stored.event.system_status = "pending".into();
        stored.attempts = 0;
        storage::save(&conn, &state)?;
    }
    runtime::changed(&app);
    app.state::<Arc<Runtime>>().wake.notify_one();
    get_usage_alerts(app)
}
