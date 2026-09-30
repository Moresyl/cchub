//! Provider-specific Coding Plan quota queries.
//!
//! Most relay providers expose a generic usage endpoint, but the major Coding
//! Plan vendors use independent APIs and response shapes.  This module keeps
//! that knowledge out of the compatibility command and returns one stable JSON
//! shape for the frontend/Pi usage-script bridge.

use std::time::Duration;

use crate::shared::usage_http::{finite_number as as_f64, official_url};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    Kimi,
    Zhipu,
    ZhipuEn,
    MiniMaxCn,
    MiniMaxEn,
}

fn detect_provider(base_url: &str, explicit: Option<&str>) -> Option<Provider> {
    let explicit = explicit.unwrap_or_default().trim().to_ascii_lowercase();
    let url = official_url(base_url);
    let host = url
        .as_ref()
        .and_then(url::Url::host_str)
        .unwrap_or_default();
    match explicit.as_str() {
        "kimi" | "kimi-coding" | "kimi_coding" => return Some(Provider::Kimi),
        "zhipu_cn" => return Some(Provider::Zhipu),
        "zhipu_en" | "zai" | "z.ai" => return Some(Provider::ZhipuEn),
        "zhipu" => {
            return Some(if host == "api.z.ai" {
                Provider::ZhipuEn
            } else {
                Provider::Zhipu
            })
        }
        "minimax_cn" => return Some(Provider::MiniMaxCn),
        "minimax_en" => return Some(Provider::MiniMaxEn),
        "minimax" => {
            return Some(if host == "api.minimaxi.com" {
                Provider::MiniMaxCn
            } else {
                Provider::MiniMaxEn
            })
        }
        _ => {}
    }
    match host {
        "api.kimi.com"
            if url.as_ref().is_some_and(|url| {
                url.path() == "/coding" || url.path().starts_with("/coding/")
            }) =>
        {
            Some(Provider::Kimi)
        }
        "open.bigmodel.cn" => Some(Provider::Zhipu),
        "api.z.ai" => Some(Provider::ZhipuEn),
        "api.minimaxi.com" => Some(Provider::MiniMaxCn),
        "api.minimax.io" => Some(Provider::MiniMaxEn),
        _ => None,
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

fn reset_at(value: Option<&Value>) -> Option<String> {
    if let Some(text) = value.and_then(Value::as_str) {
        if let Ok(date) = chrono::DateTime::parse_from_rfc3339(text.trim()) {
            return Some(date.to_rfc3339());
        }
    }
    let timestamp = value
        .and_then(Value::as_i64)
        .or_else(|| value?.as_str()?.trim().parse().ok())?;
    if timestamp <= 0 {
        return None;
    }
    let millis = if timestamp < 1_000_000_000_000 {
        timestamp.checked_mul(1000)?
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

fn utilization(total: Option<f64>, used: Option<f64>, remaining: Option<f64>) -> Option<f64> {
    let total = total.filter(|number| *number > 0.0)?;
    let ratio = match used {
        Some(used) if used >= 0.0 => used / total,
        _ => 1.0 - remaining? / total,
    };
    let value = ratio * 100.0;
    value.is_finite().then_some(value.max(0.0))
}

fn parse_kimi(body: &Value) -> Vec<Value> {
    let mut tiers = Vec::new();
    if let Some(items) = body.get("limits").and_then(Value::as_array) {
        for item in items {
            let detail = item.get("detail").unwrap_or(item);
            if let Some(usage) = utilization(
                as_f64(detail.get("limit")),
                None,
                as_f64(detail.get("remaining")),
            ) {
                tiers.push(tier("five_hour", usage, reset_at(detail.get("resetTime"))));
            }
        }
    }
    if let Some(usage) = body.get("usage") {
        if let Some(percent) = utilization(
            as_f64(usage.get("limit")),
            None,
            as_f64(usage.get("remaining")),
        ) {
            tiers.push(tier(
                "weekly_limit",
                percent,
                reset_at(usage.get("resetTime")),
            ));
        }
    }
    tiers
}

fn parse_zhipu(body: &Value) -> Vec<Value> {
    let data = body.get("data").unwrap_or(body);
    let mut tiers = Vec::new();
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
        let Some(percentage) = as_f64(item.get("percentage")).filter(|number| *number >= 0.0)
        else {
            continue;
        };
        let name = match item.get("unit").and_then(Value::as_i64) {
            Some(3) => "five_hour".to_string(),
            Some(6) => "weekly_limit".to_string(),
            Some(unit) => format!("quota_unit_{unit}"),
            None => "quota".to_string(),
        };
        let mut value = tier(&name, percentage, reset_at(item.get("nextResetTime")));
        value["metric"] = json!(kind.to_ascii_lowercase());
        tiers.push(value);
    }
    tiers.sort_by_key(|value| match value["name"].as_str() {
        Some("five_hour") => 0,
        Some("weekly_limit") => 1,
        _ => 2,
    });
    tiers
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
                let Some(utilization) = utilization(total, used, remaining)
                    .or_else(|| as_f64(item.get("percentage")).filter(|number| *number >= 0.0))
                else {
                    continue;
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

async fn query_known(provider: Provider, api_key: &str) -> Result<Value, String> {
    let (provider_name, endpoint, auth_header) = match provider {
        Provider::Kimi => (
            "kimi",
            "https://api.kimi.com/coding/v1/usages".to_string(),
            true,
        ),
        Provider::Zhipu | Provider::ZhipuEn => {
            let host = if provider == Provider::ZhipuEn {
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
    let client = crate::shared::usage_http::client()?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let body = match crate::shared::usage_http::request_json(
        &client,
        &endpoint,
        api_key,
        auth_header,
        deadline,
    )
    .await?
    {
        Ok(body) => body,
        Err(error) => return Ok(error_result(provider_name, error.message)),
    };
    let tiers = match provider {
        Provider::Kimi => parse_kimi(&body),
        Provider::Zhipu | Provider::ZhipuEn => parse_zhipu(&body),
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
    Ok(Some(query_known(provider, api_key).await?))
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

    #[test]
    fn only_exact_official_hosts_or_explicit_known_names_select_vendors() {
        assert_eq!(
            detect_provider("https://api.minimaxi.com/v1", None),
            Some(Provider::MiniMaxCn)
        );
        assert_eq!(
            detect_provider("https://api.z.ai/api", None),
            Some(Provider::ZhipuEn)
        );
        assert_eq!(
            detect_provider("https://relay.test", Some("zhipu_en")),
            Some(Provider::ZhipuEn)
        );
        for url in [
            "https://evil.test/bigmodel.cn",
            "https://evil.test?next=api.z.ai",
            "https://api.minimax.io.evil.test",
            "https://api.kimi.com/coding-other",
            "http://api.kimi.com/coding",
        ] {
            assert_eq!(detect_provider(url, Some("my-kimi-relay")), None, "{url}");
        }
    }

    #[test]
    fn incomplete_and_non_finite_windows_are_not_reported_as_zero_or_exhausted() {
        assert!(parse_kimi(
            &json!({"usage": {"limit": 100}, "limits": [{"detail": {"remaining": 1}}]})
        )
        .is_empty());
        assert!(
            parse_kimi(&json!({"usage": {"limit": "1e-300", "remaining": "-1e300"}})).is_empty()
        );
        assert!(parse_zhipu(&json!({"limits": [{"type": "TOKENS_LIMIT", "unit": 3}, {"type": "CREDIT_LIMIT", "percentage": "NaN"}]})).is_empty());
        assert!(
            parse_minimax(&json!({"five_hour": {}, "weekly": {"percentage": "inf"}})).is_empty()
        );
        assert!(parse_minimax(&json!({"weekly": {"used": 1, "total": 0}})).is_empty());
    }

    #[test]
    fn unknown_windows_keep_their_identity_and_only_valid_resets_are_used() {
        let tiers = parse_zhipu(&json!({"limits": [
            {"type": "TOKENS_LIMIT", "unit": 9, "percentage": 22, "nextResetTime": "nonsense"},
            {"type": "CREDIT_LIMIT", "unit": 3, "percentage": 50, "nextResetTime": "1800000000"}
        ]}));
        assert_eq!(tiers[0]["name"], "five_hour");
        assert!(tiers[0]["resetsAt"].as_str().unwrap().contains("2027"));
        assert_eq!(tiers[1]["name"], "quota_unit_9");
        assert!(tiers[1].get("resetsAt").is_none());
    }
}
