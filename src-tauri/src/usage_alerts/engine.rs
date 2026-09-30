use super::types::*;
use crate::commands::extra_commands::ConfigProfile;
use crate::shared::usage_http::finite_number;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

struct Reading {
    key: String,
    label: String,
    kind: &'static str,
    value: f64,
    threshold: f64,
    unit: Option<String>,
    reset_at: Option<i64>,
}

fn text<'a>(row: &'a Value, key: &str) -> Option<&'a str> {
    row.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty() && text.len() <= 256)
}

fn percentage(row: &Value) -> Option<f64> {
    let explicit = finite_number(row.get("utilization").or_else(|| row.get("percentage")));
    if let Some(value) = explicit.filter(|value| *value >= 0.0) {
        return Some(value.min(100.0));
    }
    let total = finite_number(row.get("total").or_else(|| row.get("limit")))
        .filter(|value| *value > 0.0)?;
    let ratio = match finite_number(row.get("used")).filter(|value| *value >= 0.0) {
        Some(used) => used / total,
        None => 1.0 - finite_number(row.get("remaining"))? / total,
    };
    let value = ratio * 100.0;
    value.is_finite().then_some(value.clamp(0.0, 100.0))
}

fn readings(settings: &AlertSettings, payload: &Value, now: i64) -> Vec<Reading> {
    let Some(data) = payload.get("data") else {
        return Vec::new();
    };
    let rows = match data {
        Value::Array(rows) if rows.len() <= 256 => rows.as_slice(),
        Value::Object(_) => std::slice::from_ref(data),
        _ => return Vec::new(),
    };
    let mut readings = Vec::new();
    for row in rows {
        if row.get("asOf").is_some_and(|value| !value.is_null())
            || row.get("stale").and_then(Value::as_bool) == Some(true)
            || row.get("isValid").and_then(Value::as_bool) == Some(false)
            || row.get("success").and_then(Value::as_bool) == Some(false)
        {
            continue;
        }
        let label = text(row, "planName")
            .or_else(|| text(row, "name"))
            .unwrap_or("usage");
        let unit = text(row, "unit");
        let identity = serde_json::json!([
            label,
            text(row, "accountId"),
            text(row, "metric"),
            unit.map(str::to_ascii_lowercase)
        ])
        .to_string();
        let reset = row
            .get("resetAt")
            .or_else(|| row.get("resetsAt"))
            .filter(|value| !value.is_null());
        let reset_at = reset
            .and_then(Value::as_str)
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
            .map(|date| date.timestamp());
        // An invalid or elapsed reset is not evidence of the current quota window.
        let current_window = reset.is_none() || reset_at.is_some_and(|reset| reset > now);
        if let (Some(threshold), Some(value), true) =
            (settings.quota_percent, percentage(row), current_window)
        {
            readings.push(Reading {
                key: format!("q:{}", hash(&identity)),
                label: label.into(),
                kind: "quota",
                value,
                threshold,
                unit: None,
                reset_at,
            });
        }
        if let (Some(unit), Some(value)) = (unit, finite_number(row.get("remaining"))) {
            if let Some(balance) = settings
                .balances
                .iter()
                .find(|balance| balance.unit.eq_ignore_ascii_case(unit))
            {
                readings.push(Reading {
                    key: format!("b:{}", hash(&identity)),
                    label: label.into(),
                    kind: "balance",
                    value,
                    threshold: balance.amount,
                    unit: Some(unit.into()),
                    reset_at: None,
                });
            }
        }
    }
    // Repeated indistinguishable rows must not select an arbitrary account/window.
    let mut counts = BTreeMap::new();
    for reading in &readings {
        *counts.entry(reading.key.clone()).or_insert(0) += 1;
    }
    readings.retain(|reading| counts[&reading.key] == 1);
    readings
}

pub(crate) fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub(crate) fn observe(
    state: &mut StoredState,
    profile: &ConfigProfile,
    payload: &Value,
    now: i64,
) -> usize {
    let Some(rule) = state.rules.get(&profile.id) else {
        return 0;
    };
    if !rule.settings.enabled
        || payload.get("success").and_then(Value::as_bool) != Some(true)
        || payload.get("asOf").is_some_and(|value| !value.is_null())
        || payload.get("stale").and_then(Value::as_bool) == Some(true)
    {
        return 0;
    }
    let readings = readings(&rule.settings, payload, now);
    let identity = rule.identity.clone();
    let system = rule.settings.system_notifications;
    let marks = state.marks.entry(profile.id.clone()).or_default();
    let seen: HashSet<_> = readings.iter().map(|reading| reading.key.clone()).collect();
    marks
        .retain(|key, mark| seen.contains(key) || now.saturating_sub(mark.updated_at) < 40 * 86400);
    let mut added = 0;
    for reading in readings {
        let mark = marks.entry(reading.key).or_insert(Mark {
            reset_at: reading.reset_at,
            notified: false,
            updated_at: now,
        });
        mark.updated_at = now;
        if reading.kind == "quota" {
            match (mark.reset_at, reading.reset_at) {
                (Some(old), Some(next)) if now >= old && next.saturating_sub(old) > 60 => {
                    mark.reset_at = Some(next);
                    mark.notified = false;
                }
                (Some(_), None) => continue,
                (None, Some(next)) => {
                    mark.reset_at = Some(next);
                }
                _ => {}
            }
            if reading.value < reading.threshold {
                if mark.reset_at.is_none() && reading.value <= (reading.threshold - 5.0).max(0.0) {
                    mark.notified = false;
                }
                continue;
            }
        } else if reading.value > reading.threshold {
            if reading.value > reading.threshold * 1.1 {
                mark.notified = false;
            }
            continue;
        }
        if mark.notified {
            continue;
        }
        if state.events.len() >= 200 {
            // Pending delivery must not be evicted before the system accepts it.
            if let Some(index) = state
                .events
                .iter()
                .position(|event| event.event.system_status != "pending")
            {
                state.events.remove(index);
            } else {
                continue;
            }
        }
        mark.notified = true;
        state.events.push(StoredEvent {
            event: AlertEvent {
                id: uuid::Uuid::new_v4().to_string(),
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                tool_id: profile.tool_id.clone(),
                kind: reading.kind.into(),
                label: reading.label,
                value: reading.value,
                threshold: reading.threshold,
                unit: reading.unit,
                reset_at: reading.reset_at,
                created_at: now,
                read: false,
                system_status: if system { "pending" } else { "off" }.into(),
            },
            identity: identity.clone(),
            attempts: 0,
        });
        added += 1;
    }
    added
}

#[cfg(test)]
pub(crate) mod tests;
