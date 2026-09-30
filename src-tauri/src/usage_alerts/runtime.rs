use super::{engine, storage, types::*};
use crate::commands::extra_commands::ConfigProfile;
use crate::commands::usage_compat::query_profile_usage;
use crate::db::DbState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{Mutex, Notify};

#[derive(Default)]
pub(crate) struct Runtime {
    pub wake: Notify,
    round: Mutex<()>,
    polling: AtomicBool,
}

impl Runtime {
    pub fn polling(&self) -> bool {
        self.polling.load(Ordering::Acquire)
    }
}

pub(crate) fn init(app: &AppHandle) {
    let runtime = Arc::new(Runtime::default());
    app.manage(runtime.clone());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut delay = std::time::Duration::from_secs(60);
        loop {
            tokio::select! { _ = tokio::time::sleep(delay) => {}, _ = runtime.wake.notified() => {} }
            if let Err(error) = check(&app).await {
                crate::utils::append_runtime_log("warn", "usage-alerts", &error);
            }
            delay = std::time::Duration::from_secs(300);
        }
    });
}

pub(super) fn changed<R: tauri::Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("usage-alerts-changed", ());
}

fn with_state<T, R: tauri::Runtime>(
    app: &AppHandle<R>,
    operation: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    let db = app.state::<DbState>();
    let conn =
        db.0.lock()
            .map_err(|_| "Usage alert storage is unavailable".to_string())?;
    operation(&conn)
}

fn apply_result(
    state: &mut StoredState,
    profile: &ConfigProfile,
    expected: &Rule,
    result: &Result<serde_json::Value, String>,
    now: i64,
) {
    let Some(rule) = state.rules.get_mut(&profile.id) else {
        return;
    };
    if !rule.settings.enabled
        || rule.revision != expected.revision
        || !storage::current(rule, profile)
    {
        return;
    }
    rule.checked_at = Some(now);
    rule.status = match result {
        Ok(payload)
            if payload.get("success").and_then(serde_json::Value::as_bool) == Some(true) =>
        {
            "ok"
        }
        _ => "query_failed",
    }
    .into();
    if let Ok(payload) = result {
        engine::observe(state, profile, payload, now);
    }
}

struct PollingGuard<'a, R: tauri::Runtime> {
    runtime: &'a Runtime,
    app: &'a AppHandle<R>,
}

impl<R: tauri::Runtime> Drop for PollingGuard<'_, R> {
    fn drop(&mut self) {
        self.runtime.polling.store(false, Ordering::Release);
        changed(self.app);
    }
}

pub(super) async fn check<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let runtime = app.state::<Arc<Runtime>>();
    let _round = runtime
        .round
        .try_lock()
        .map_err(|_| "Usage alert check is already running".to_string())?;
    let candidates = with_state(app, |conn| {
        let mut state = storage::load(conn)?;
        let profiles = storage::profiles(conn)?;
        if storage::retain_profiles(&mut state, &profiles) {
            storage::save(conn, &state)?;
        }
        Ok(profiles
            .into_iter()
            .filter_map(|profile| {
                let rule = state.rules.get(&profile.id)?;
                (rule.settings.enabled && storage::current(rule, &profile))
                    .then(|| (profile, rule.clone()))
            })
            .collect::<Vec<_>>())
    })?;
    runtime.polling.store(true, Ordering::Release);
    let _polling = PollingGuard {
        runtime: &runtime,
        app,
    };
    changed(app);
    let result = async {
        // Four simultaneous requests bound load without letting one account stall all others.
        for batch in candidates.chunks(4) {
            let futures = batch.iter().map(|(profile, rule)| async move {
                let result = query_profile_usage(profile).await;
                (profile.id.clone(), rule, result)
            });
            let results = futures_util::future::join_all(futures).await;
            with_state(app, |conn| {
                let mut state = storage::load(conn)?;
                let profiles = storage::profiles(conn)?;
                for (id, expected, result) in results {
                    if let Some(profile) = profiles.iter().find(|profile| profile.id == id) {
                        apply_result(
                            &mut state,
                            profile,
                            expected,
                            &result,
                            chrono::Utc::now().timestamp(),
                        );
                    }
                }
                storage::save(conn, &state)
            })?;
            changed(app);
        }
        deliver(app)?;
        Ok(())
    }
    .await;
    result
}

fn deliver<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    // The DB lock also serializes a last account/enable check with native submission.
    with_state(app, |conn| {
        let mut state = storage::load(conn)?;
        let profiles = storage::profiles(conn)?;
        let dirty = submit_pending(&mut state, &profiles, |event| {
            let body = if event.kind == "quota" {
                format!(
                    "{} · {}: {:.1}% / {:.1}%",
                    event.profile_name, event.label, event.value, event.threshold
                )
            } else {
                format!(
                    "{} · {}: {} {} ≤ {}",
                    event.profile_name,
                    event.label,
                    event.value,
                    event.unit.as_deref().unwrap_or(""),
                    event.threshold
                )
            };
            app.notification()
                .builder()
                .title("CCHub · 用量提醒 / Usage alert")
                .body(body)
                .show()
                .is_ok()
        });
        if dirty {
            storage::save(conn, &state)?;
        }
        Ok(())
    })
}

fn submit_pending(
    state: &mut StoredState,
    profiles: &[ConfigProfile],
    mut submit: impl FnMut(&AlertEvent) -> bool,
) -> bool {
    let mut dirty = false;
    for stored in &mut state.events {
        if stored.event.system_status != "pending" {
            continue;
        }
        let allowed = state
            .rules
            .get(&stored.event.profile_id)
            .zip(
                profiles
                    .iter()
                    .find(|profile| profile.id == stored.event.profile_id),
            )
            .is_some_and(|(rule, profile)| {
                rule.settings.enabled
                    && rule.settings.system_notifications
                    && rule.identity == stored.identity
                    && storage::current(rule, profile)
            });
        dirty = true;
        if !allowed {
            stored.event.system_status = "cancelled".into();
            continue;
        }
        stored.attempts = stored.attempts.saturating_add(1);
        match submit(&stored.event) {
            true => stored.event.system_status = "accepted".into(),
            false if stored.attempts >= 6 => stored.event.system_status = "failed".into(),
            false => {}
        }
    }
    dirty
}

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests;
