use super::types::*;
use crate::commands::extra_commands::{read_all_config_profiles_from_conn, ConfigProfile};
use crate::commands::usage_compat::usage_identity;
use rusqlite::{Connection, OptionalExtension};

const KEY: &str = "usage_alert_state";
const MAX_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn profiles(conn: &Connection) -> Result<Vec<ConfigProfile>, String> {
    let profiles = read_all_config_profiles_from_conn(conn)?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM config_profiles", [], |row| row.get(0))
        .map_err(|_| "Unable to read configurations for usage alerts".to_string())?;
    if count < 0 || count as usize != profiles.len() {
        return Err(
            "Some configurations could not be read; usage alert state was preserved".into(),
        );
    }
    Ok(profiles)
}

pub(super) fn load(conn: &Connection) -> Result<StoredState, String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| "Unable to read usage alert history".to_string())?;
    let Some(raw) = raw else {
        return Ok(StoredState::default());
    };
    if raw.len() > MAX_BYTES {
        return Err("Usage alert history exceeds the storage limit".into());
    }
    let state: StoredState = serde_json::from_str(&raw)
        .map_err(|_| "Usage alert history is invalid; existing data was preserved".to_string())?;
    validate(&state)?;
    Ok(state)
}

fn validate(state: &StoredState) -> Result<(), String> {
    if state.version != 1
        || state.rules.len() > 256
        || state.events.len() > 200
        || state.marks.len() > 256
        || state.marks.values().any(|marks| marks.len() > 512)
    {
        return Err(
            "Unsupported or oversized usage alert history; existing data was preserved".into(),
        );
    }
    for rule in state.rules.values() {
        rule.settings.clone().validate()?;
    }
    if state
        .events
        .iter()
        .any(|stored| !stored.event.value.is_finite() || !stored.event.threshold.is_finite())
    {
        return Err("Usage alert history contains invalid readings".into());
    }
    Ok(())
}

pub(super) fn save(conn: &Connection, state: &StoredState) -> Result<(), String> {
    validate(state)?;
    let raw = serde_json::to_string(state)
        .map_err(|_| "Unable to encode usage alert history".to_string())?;
    if raw.len() > MAX_BYTES {
        return Err("Usage alert history exceeds the storage limit".into());
    }
    conn.execute("INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![KEY, raw]).map_err(|_| "Unable to save usage alert history".to_string())?;
    Ok(())
}

pub(super) fn current(rule: &Rule, profile: &ConfigProfile) -> bool {
    usage_identity(profile).is_ok_and(|identity| identity == rule.identity)
}

pub(super) fn retain_profiles(state: &mut StoredState, profiles: &[ConfigProfile]) -> bool {
    let ids: std::collections::HashSet<_> =
        profiles.iter().map(|profile| profile.id.as_str()).collect();
    let before = (state.rules.len(), state.marks.len());
    state.rules.retain(|id, _| ids.contains(id.as_str()));
    state.marks.retain(|id, _| ids.contains(id.as_str()));
    let mut changed = before != (state.rules.len(), state.marks.len());
    for stored in &mut state.events {
        if !ids.contains(stored.event.profile_id.as_str())
            && stored.event.system_status == "pending"
        {
            stored.event.system_status = "cancelled".into();
            changed = true;
        }
    }
    changed
}

pub(super) fn overview(
    state: &StoredState,
    profiles: &[ConfigProfile],
    polling: bool,
) -> AlertOverview {
    let rules = profiles
        .iter()
        .map(|profile| {
            let rule = state.rules.get(&profile.id);
            let paused = rule.is_some_and(|rule| rule.settings.enabled && !current(rule, profile));
            RuleView {
                profile_id: profile.id.clone(),
                query_identity: usage_identity(profile).ok(),
                settings: rule.map(|rule| rule.settings.clone()).unwrap_or_default(),
                paused,
                status: if paused {
                    "account_changed".into()
                } else {
                    rule.map(|rule| rule.status.clone())
                        .unwrap_or_else(|| "off".into())
                },
                checked_at: rule.and_then(|rule| rule.checked_at),
            }
        })
        .collect();
    AlertOverview {
        rules,
        events: state
            .events
            .iter()
            .rev()
            .map(|stored| stored.event.clone())
            .collect(),
        polling,
    }
}

pub(super) fn set_rule(
    state: &mut StoredState,
    profile: &ConfigProfile,
    mut settings: AlertSettings,
) -> Result<(), String> {
    settings.validate()?;
    let identity = usage_identity(profile)?;
    if !state.rules.contains_key(&profile.id) && state.rules.len() >= 256 {
        return Err("At most 256 usage alert rules are allowed".into());
    }
    let old = state.rules.get(&profile.id);
    if old.is_none_or(|old| {
        old.identity != identity
            || old.settings.quota_percent != settings.quota_percent
            || old.settings.balances != settings.balances
    }) {
        state.marks.remove(&profile.id);
    }
    for stored in &mut state.events {
        if stored.event.profile_id == profile.id
            && stored.event.system_status == "pending"
            && (!settings.enabled || !settings.system_notifications || stored.identity != identity)
        {
            stored.event.system_status = "cancelled".into();
        }
    }
    let status = if settings.enabled { "ready" } else { "off" }.into();
    state.rules.insert(
        profile.id.clone(),
        Rule {
            settings,
            identity,
            revision: uuid::Uuid::new_v4().to_string(),
            checked_at: None,
            status,
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests;

pub(super) fn set_rule_if_current(
    state: &mut StoredState,
    profile: &ConfigProfile,
    settings: AlertSettings,
    expected_identity: &str,
) -> Result<(), String> {
    if usage_identity(profile)? != expected_identity {
        return Err("Configuration changed; reload alert settings before saving".into());
    }
    set_rule(state, profile, settings)
}
