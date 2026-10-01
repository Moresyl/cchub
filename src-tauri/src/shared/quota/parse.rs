use super::{Resource, Snapshot};
use serde_json::Value;
use std::collections::HashMap;

fn number(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    let number = value.as_f64().or_else(|| value.as_str()?.parse().ok())?;
    number.is_finite().then_some(number)
}

fn reset(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(timestamp) = value.as_i64().or_else(|| value.as_str()?.parse().ok()) {
        let seconds = if timestamp > 10_000_000_000 {
            timestamp / 1000
        } else {
            timestamp
        };
        return chrono::DateTime::from_timestamp(seconds, 0).map(|date| date.timestamp());
    }
    let text = value.as_str()?;
    chrono::DateTime::parse_from_rfc3339(text)
        .map(|date| date.timestamp())
        .ok()
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|date| date.and_utc().timestamp())
        })
}

fn codex(value: &Value) -> Option<i64> {
    let rate = value.get("rate_limit")?.as_object()?;
    if rate.get("allowed").and_then(Value::as_bool) == Some(true)
        || rate.get("limit_reached").and_then(Value::as_bool) == Some(false)
    {
        return None;
    }
    // Credits can permit paid continuation. Incomplete credit information is
    // unknown rather than proof that the account cannot serve a request.
    if let Some(credits) = value.get("credits").filter(|value| !value.is_null()) {
        if credits.get("unlimited").and_then(Value::as_bool) != Some(false)
            || credits.get("has_credits").and_then(Value::as_bool) != Some(false)
            || number(credits.get("balance")).is_some_and(|balance| balance > 0.0)
        {
            return None;
        }
    }
    let now = chrono::Utc::now().timestamp();
    let mut expired = false;
    let until = ["primary_window", "secondary_window"]
        .into_iter()
        .filter_map(|key| {
            let window = rate.get(key)?;
            if !number(window.get("used_percent")).is_some_and(|used| used >= 100.0) {
                return None;
            }
            let until = reset(window.get("reset_at"))?;
            if until <= now {
                expired = true;
                return None;
            }
            Some(until)
        })
        .max();
    until.or_else(|| {
        (!expired
            && rate.get("allowed").and_then(Value::as_bool) == Some(false)
            && rate.get("limit_reached").and_then(Value::as_bool) == Some(true))
        .then_some(i64::MAX)
    })
}

fn copilot_window(value: Option<&Value>, until: Option<i64>) -> Option<i64> {
    let value = value?;
    if value.get("unlimited").and_then(Value::as_bool) == Some(true)
        || value.get("overage_permitted").and_then(Value::as_bool) == Some(true)
        || value.get("has_quota").and_then(Value::as_bool) == Some(false)
    {
        return None;
    }
    let entitlement = number(value.get("entitlement"))?;
    let remaining = number(
        value
            .get("quota_remaining")
            .or_else(|| value.get("remaining")),
    )?;
    if entitlement <= 0.0
        || remaining > 0.0
        || number(value.get("percent_remaining")).is_some_and(|percent| percent > 0.0)
    {
        return None;
    }
    Some(until.unwrap_or(i64::MAX))
}

pub(super) fn snapshot(resource: Resource, value: &Value) -> Snapshot {
    match resource {
        Resource::CodexUsage => Snapshot::Codex(codex(value)),
        Resource::CopilotUsage => {
            if ["quota_reset_date_utc", "quota_reset_date"]
                .iter()
                .any(|key| {
                    value.get(*key).is_some_and(|field| {
                        !field.is_null()
                            && field.as_str() != Some("")
                            && reset(Some(field)).is_none()
                    })
                })
            {
                return Snapshot::Copilot {
                    chat: None,
                    premium: None,
                };
            }
            let until = reset(value.get("quota_reset_date_utc"))
                .or_else(|| reset(value.get("quota_reset_date")));
            let quotas = value.get("quota_snapshots").unwrap_or(&Value::Null);
            Snapshot::Copilot {
                chat: copilot_window(quotas.get("chat"), until),
                premium: copilot_window(quotas.get("premium_interactions"), until),
            }
        }
        Resource::CopilotModels => {
            let mut models = HashMap::new();
            if let Some(entries) = value
                .get("data")
                .and_then(Value::as_array)
                .filter(|entries| entries.len() <= 512)
            {
                for entry in entries {
                    let Some(id) = entry
                        .get("id")
                        .and_then(Value::as_str)
                        .filter(|id| !id.is_empty() && id.len() <= 1024)
                    else {
                        continue;
                    };
                    let billed = entry.get("billing").is_some_and(|billing| {
                        billing.get("is_premium").and_then(Value::as_bool) == Some(true)
                            && number(billing.get("multiplier"))
                                .is_some_and(|multiplier| multiplier > 0.0)
                    });
                    models
                        .entry(id.into())
                        .and_modify(|previous| *previous &= billed)
                        .or_insert(billed);
                }
            }
            Snapshot::Models(models)
        }
    }
}
