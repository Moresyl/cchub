use crate::shared::usage_http::finite_number;
use serde_json::{json, Value};

fn number_at(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| finite_number(value.get(*key)))
}

fn row(provider: &str, value: &Value) -> Option<Value> {
    if value.get("success").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let remaining = number_at(value, &["remaining", "balance", "credits", "total_balance"]);
    let used = number_at(value, &["used", "usage", "total_usage"]).filter(|value| *value >= 0.0);
    let total =
        number_at(value, &["limit", "total", "total_credits"]).filter(|value| *value >= 0.0);
    let utilization =
        number_at(value, &["utilization", "percentage"]).filter(|value| *value >= 0.0);
    if remaining.is_none() && used.is_none() && total.is_none() && utilization.is_none() {
        return None;
    }
    let mut row = json!({"planName": provider});
    for (key, number) in [
        ("remaining", remaining),
        ("used", used),
        ("total", total),
        ("utilization", utilization),
    ] {
        if let Some(number) = number {
            row[key] = json!(number);
        }
    }
    for (target, keys) in [
        ("planName", ["planName", "name"].as_slice()),
        ("unit", ["unit", "currency"].as_slice()),
        ("metric", ["metric"].as_slice()),
        ("accountId", ["accountId"].as_slice()),
        ("resetAt", ["resetAt", "resetsAt", "reset_at"].as_slice()),
    ] {
        if let Some(text) = keys
            .iter()
            .find_map(|key| value.get(*key).and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            row[target] = json!(text);
        }
    }
    if let Some(valid) = value.get("isValid").and_then(Value::as_bool) {
        row["isValid"] = json!(valid);
    }
    for key in ["asOf", "stale"] {
        if let Some(value) = value.get(key) {
            row[key] = value.clone();
        }
    }
    Some(row)
}

pub(super) fn normalize_usage(provider: &str, payload: &Value) -> Value {
    if payload.get("success").and_then(Value::as_bool) == Some(false)
        || matches!(
            payload.get("status").and_then(Value::as_str),
            Some("error" | "not_found")
        )
    {
        return json!({"success": false, "provider": provider, "data": [], "error": "Provider reported an unsuccessful usage query"});
    }
    let values = payload
        .get("data")
        .or_else(|| payload.get("tiers"))
        .unwrap_or(payload);
    let rows: Vec<Value> = if let Some(items) = values.as_array() {
        items
            .iter()
            .filter_map(|item| row(provider, item))
            .collect()
    } else {
        row(provider, values).into_iter().collect()
    };
    let mut result = json!({"success": !rows.is_empty(), "provider": provider, "data": rows,
        "error": if rows.is_empty() { Some("Provider returned no recognized usage fields") } else { None }});
    for key in ["asOf", "stale"] {
        if let Some(value) = payload.get(key) {
            result[key] = value.clone();
        }
    }
    result
}

pub(super) fn quota_from_usage(provider: &str, result: &Value) -> Value {
    let tiers: Vec<Value> = result.get("data").and_then(Value::as_array).into_iter().flatten().filter_map(|row| {
        let percentage = finite_number(row.get("utilization")).filter(|value| *value >= 0.0).or_else(|| {
            let total = finite_number(row.get("total")).filter(|value| *value > 0.0)?;
            let ratio = match finite_number(row.get("used")).filter(|value| *value >= 0.0) {
                Some(used) => used / total,
                None => 1.0 - finite_number(row.get("remaining"))? / total,
            };
            let percent = ratio * 100.0;
            percent.is_finite().then_some(percent.max(0.0))
        })?;
        let mut tier = json!({"name": row.get("planName").and_then(Value::as_str).unwrap_or("quota"), "utilization": percentage.clamp(0.0, 100.0)});
        if let Some(metric) = row.get("metric").and_then(Value::as_str) { tier["metric"] = json!(metric); }
        if let Some(reset) = row.get("resetAt").and_then(Value::as_str)
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok()) {
            tier["resetsAt"] = json!(reset.to_rfc3339());
        }
        Some(tier)
    }).collect();
    if result.get("success").and_then(Value::as_bool) != Some(true) || tiers.is_empty() {
        return json!({"status": "not_found", "provider": provider, "tiers": [], "error": "Provider returned no recognized quota windows"});
    }
    json!({"status": "ok", "provider": provider, "tiers": tiers})
}

#[cfg(test)]
mod tests;
