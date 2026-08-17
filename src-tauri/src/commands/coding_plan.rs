//! Provider-specific Coding Plan quota queries.
//!
//! Most relay providers expose a generic usage endpoint, but the major Coding
//! Plan vendors use independent APIs and response shapes.  This module keeps
//! that knowledge out of the compatibility command and returns one stable JSON
//! shape for the frontend/Pi usage-script bridge.

use std::time::Duration;

use serde_json::{json, Value};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    Kimi,
    Zhipu,
    MiniMaxCn,
    MiniMaxEn,
}

fn detect_provider(base_url: &str, explicit: Option<&str>) -> Option<Provider> {
    let explicit = explicit.unwrap_or_default().trim().to_ascii_lowercase();
    let url = base_url.to_ascii_lowercase();
    if explicit.contains("kimi") || url.contains("api.kimi.com/coding") {
        Some(Provider::Kimi)
    } else if explicit.contains("zhipu") || url.contains("bigmodel.cn") || url.contains("api.z.ai")
    {
        Some(Provider::Zhipu)
    } else if explicit.contains("minimax")
        && (explicit.contains("cn") || url.contains("minimaxi.com"))
    {
        Some(Provider::MiniMaxCn)
    } else if explicit.contains("minimax") || url.contains("minimax.io") {
        Some(Provider::MiniMaxEn)
    } else {
        None
    }
}

fn not_found(provider: &str, error: impl Into<String>) -> Value {
    json!({"status": "not_found", "provider": provider, "tiers": [], "error": error.into()})
}

fn error_result(provider: &str, error: impl Into<String>) -> Value {
    json!({"status": "error", "provider": provider, "tiers": [], "error": error.into()})
}

fn ok_result(provider: &str, tiers: Vec<Value>) -> Value {
    json!({"status": "ok", "provider": provider, "tiers": tiers})
}

fn as_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).or_else(|| {
        value
            .and_then(Value::as_str)
            .and_then(|item| item.parse().ok())
    })
}

fn reset_at(value: Option<&Value>) -> Option<String> {
    if let Some(text) = value.and_then(Value::as_str) {
        return (!text.trim().is_empty()).then(|| text.to_string());
    }
    let timestamp = value.and_then(Value::as_i64)?;
    if timestamp <= 0 {
        return None;
    }
    let millis = if timestamp < 1_000_000_000_000 {
        timestamp.saturating_mul(1000)
    } else {
        timestamp
    };
    chrono::DateTime::from_timestamp_millis(millis).map(|date| date.to_rfc3339())
}

fn tier(name: &str, utilization: f64, resets_at: Option<String>) -> Value {
    let mut value = json!({
        "name": name,
        "utilization": utilization.clamp(0.0, 100.0),
    });
    if let Some(reset) = resets_at {
        value["resetsAt"] = json!(reset);
    }
    value
}

async fn request_json(
    url: &str,
    api_key: &str,
    auth_header: bool,
) -> Result<Result<Value, String>, String> {
    let client = crate::shared::http_client::build_http_client(
        None,
        Some("CCHub Coding Plan"),
        Duration::from_secs(15),
    )?;
    let mut request = client.get(url).header("Accept", "application/json");
    if auth_header {
        request = request.bearer_auth(api_key);
    } else {
        request = request.header("Authorization", api_key);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("Coding Plan request failed: {error}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read Coding Plan response: {error}"))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Ok(Err("Coding Plan response is too large".to_string()));
    }
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Ok(Err(format!("Authentication failed (HTTP {status})")));
        }
        return Ok(Err(format!("Coding Plan API returned HTTP {status}")));
    }
    serde_json::from_slice(&body)
        .map_err(|error| format!("Invalid Coding Plan response: {error}"))
        .map(Ok)
}

fn parse_kimi(body: &Value) -> Vec<Value> {
    let mut tiers = Vec::new();
    if let Some(items) = body.get("limits").and_then(Value::as_array) {
        for item in items {
            let detail = item.get("detail").unwrap_or(item);
            let limit = as_f64(detail.get("limit")).unwrap_or(0.0);
            let remaining = as_f64(detail.get("remaining")).unwrap_or(0.0);
            if limit > 0.0 {
                tiers.push(tier(
                    "five_hour",
                    ((limit - remaining).max(0.0) / limit) * 100.0,
                    reset_at(detail.get("resetTime")),
                ));
            }
        }
    }
    if let Some(usage) = body.get("usage") {
        let limit = as_f64(usage.get("limit")).unwrap_or(0.0);
        let remaining = as_f64(usage.get("remaining")).unwrap_or(0.0);
        if limit > 0.0 {
            tiers.push(tier(
                "weekly_limit",
                ((limit - remaining).max(0.0) / limit) * 100.0,
                reset_at(usage.get("resetTime")),
            ));
        }
    }
    tiers
}

fn parse_zhipu(body: &Value) -> Vec<Value> {
    let data = body.get("data").unwrap_or(body);
    let mut five_hour = None;
    let mut weekly = None;
    let mut fallback = Vec::new();
    for item in data
        .get("limits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
        if !kind.eq_ignore_ascii_case("TOKENS_LIMIT") && !kind.eq_ignore_ascii_case("CREDIT_LIMIT")
        {
            continue;
        }
        let value = tier(
            "five_hour",
            as_f64(item.get("percentage")).unwrap_or(0.0),
            reset_at(item.get("nextResetTime")),
        );
        match item.get("unit").and_then(Value::as_i64) {
            Some(3) if five_hour.is_none() => five_hour = Some(value),
            Some(6) if weekly.is_none() => {
                weekly = Some(Value::Object({
                    let mut object = value.as_object().cloned().unwrap_or_default();
                    object.insert("name".to_string(), json!("weekly_limit"));
                    object
                }))
            }
            _ => fallback.push(value),
        }
    }
    for value in fallback {
        if five_hour.is_none() {
            five_hour = Some(value);
        } else if weekly.is_none() {
            let mut object = value.as_object().cloned().unwrap_or_default();
            object.insert("name".to_string(), json!("weekly_limit"));
            weekly = Some(Value::Object(object));
        }
    }
    [five_hour, weekly].into_iter().flatten().collect()
}

fn parse_minimax(body: &Value) -> Vec<Value> {
    let payload = body.get("data").unwrap_or(body);
    let mut tiers = Vec::new();
    let candidates = [
        ("five_hour", ["five_hour", "fiveHour", "5h"].as_slice()),
        ("weekly_limit", ["weekly", "weekly_limit", "7d"].as_slice()),
    ];
    for (name, keys) in candidates {
        for key in keys {
            if let Some(item) = payload.get(*key) {
                let remaining = as_f64(item.get("remaining").or_else(|| item.get("remain")));
                let used = as_f64(item.get("used").or_else(|| item.get("usage")));
                let total = as_f64(item.get("total").or_else(|| item.get("limit")));
                let utilization = match (used, total, remaining, total) {
                    (Some(used), Some(total), _, _) if total > 0.0 => used / total * 100.0,
                    (_, _, Some(remaining), Some(total)) if total > 0.0 => {
                        (1.0 - remaining / total) * 100.0
                    }
                    _ => as_f64(item.get("percentage")).unwrap_or(0.0),
                };
                tiers.push(tier(
                    name,
                    utilization,
                    reset_at(item.get("resetTime").or_else(|| item.get("reset_at"))),
                ));
                break;
            }
        }
    }
    tiers
}

async fn query_known(provider: Provider, base_url: &str, api_key: &str) -> Result<Value, String> {
    let (provider_name, endpoint, auth_header) = match provider {
        Provider::Kimi => (
            "kimi",
            "https://api.kimi.com/coding/v1/usages".to_string(),
            true,
        ),
        Provider::Zhipu => {
            let host = if base_url.to_ascii_lowercase().contains("api.z.ai") {
                "api.z.ai"
            } else {
                "open.bigmodel.cn"
            };
            (
                "zhipu",
                format!("https://{host}/api/monitor/usage/quota/limit"),
                false,
            )
        }
        Provider::MiniMaxCn => (
            "minimax_cn",
            "https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains".to_string(),
            true,
        ),
        Provider::MiniMaxEn => (
            "minimax_en",
            "https://api.minimax.io/v1/api/openplatform/coding_plan/remains".to_string(),
            true,
        ),
    };
    let body = match request_json(&endpoint, api_key, auth_header).await? {
        Ok(body) => body,
        Err(error) => return Ok(error_result(provider_name, error)),
    };
    let tiers = match provider {
        Provider::Kimi => parse_kimi(&body),
        Provider::Zhipu => parse_zhipu(&body),
        Provider::MiniMaxCn | Provider::MiniMaxEn => parse_minimax(&body),
    };
    if tiers.is_empty() {
        return Ok(error_result(
            provider_name,
            "Provider returned no recognized quota windows",
        ));
    }
    Ok(ok_result(provider_name, tiers))
}

/// Query a known Coding Plan provider, or return `None` for generic relays.
pub async fn query(
    base_url: &str,
    api_key: &str,
    explicit_provider: Option<&str>,
) -> Result<Option<Value>, String> {
    let provider = detect_provider(base_url, explicit_provider);
    let Some(provider) = provider else {
        return Ok(None);
    };
    if api_key.trim().is_empty() {
        return Ok(Some(not_found("coding_plan", "API key is empty")));
    }
    Ok(Some(query_known(provider, base_url, api_key).await?))
}

#[cfg(test)]
mod tests {
    use super::{detect_provider, parse_kimi, parse_minimax, parse_zhipu, Provider};
    use serde_json::json;

    #[test]
    fn detects_supported_hosts_and_explicit_provider() {
        assert_eq!(
            detect_provider("https://api.kimi.com/coding", None),
            Some(Provider::Kimi)
        );
        assert_eq!(
            detect_provider("https://open.bigmodel.cn/api/coding", None),
            Some(Provider::Zhipu)
        );
        assert_eq!(
            detect_provider("https://example.test", Some("minimax_cn")),
            Some(Provider::MiniMaxCn)
        );
        assert_eq!(detect_provider("https://example.test", None), None);
    }

    #[test]
    fn parses_kimi_windows() {
        let tiers = parse_kimi(&json!({
            "limits": [{"detail": {"limit": 100, "remaining": 25, "resetTime": 1_800_000_000_000_i64}}],
            "usage": {"limit": "200", "remaining": "100"}
        }));
        assert_eq!(tiers[0]["name"], "five_hour");
        assert_eq!(tiers[0]["utilization"], 75.0);
        assert_eq!(tiers[1]["name"], "weekly_limit");
    }

    #[test]
    fn parses_zhipu_units_without_relying_on_order() {
        let tiers = parse_zhipu(&json!({"data": {"limits": [
            {"type": "TOKENS_LIMIT", "unit": 6, "percentage": 31},
            {"type": "TOKENS_LIMIT", "unit": 3, "percentage": 12}
        ]}}));
        assert_eq!(tiers[0]["name"], "five_hour");
        assert_eq!(tiers[1]["name"], "weekly_limit");
    }

    #[test]
    fn parses_minimax_common_shapes() {
        let tiers = parse_minimax(&json!({"data": {
            "five_hour": {"used": 20, "total": 100},
            "weekly": {"percentage": 35}
        }}));
        assert_eq!(tiers[0]["utilization"], 20.0);
        assert_eq!(tiers[1]["utilization"], 35.0);
    }
}
