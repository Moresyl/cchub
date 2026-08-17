//! Official balance endpoints for common API providers.
//!
//! Unknown relays intentionally fall back to the generic compatibility query;
//! this module only claims hosts whose endpoint and response shape are known.

use std::time::Duration;

use serde_json::{json, Value};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    DeepSeek,
    StepFun,
    SiliconFlowCn,
    SiliconFlowEn,
    OpenRouter,
    Novita,
}

fn detect_provider(base_url: &str) -> Option<Provider> {
    let url = base_url.to_ascii_lowercase();
    if url.contains("api.deepseek.com") {
        Some(Provider::DeepSeek)
    } else if url.contains("api.stepfun.ai") || url.contains("api.stepfun.com") {
        Some(Provider::StepFun)
    } else if url.contains("api.siliconflow.cn") {
        Some(Provider::SiliconFlowCn)
    } else if url.contains("api.siliconflow.com") {
        Some(Provider::SiliconFlowEn)
    } else if url.contains("openrouter.ai") {
        Some(Provider::OpenRouter)
    } else if url.contains("api.novita.ai") {
        Some(Provider::Novita)
    } else {
        None
    }
}

fn as_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).or_else(|| {
        value
            .and_then(Value::as_str)
            .and_then(|item| item.parse().ok())
    })
}

fn result(provider: &str, row: Value) -> Value {
    json!({"success": true, "provider": provider, "data": [row], "error": null})
}

fn failure(provider: &str, error: impl Into<String>) -> Value {
    json!({"success": false, "provider": provider, "data": [], "error": error.into()})
}

async fn request_json(url: &str, api_key: &str) -> Result<Result<Value, String>, String> {
    let client = crate::shared::http_client::build_http_client(
        None,
        Some("CCHub Balance"),
        Duration::from_secs(15),
    )?;
    let response = client
        .get(url)
        .bearer_auth(api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| format!("Balance request failed: {error}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read balance response: {error}"))?;
    if body.len() > MAX_RESPONSE_BYTES {
        return Ok(Err("Balance response is too large".to_string()));
    }
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Ok(Err(format!("Authentication failed (HTTP {status})")));
        }
        return Ok(Err(format!("Balance API returned HTTP {status}")));
    }
    serde_json::from_slice(&body)
        .map_err(|error| format!("Invalid balance response: {error}"))
        .map(Ok)
}

fn parse(provider: Provider, body: &Value) -> Option<Value> {
    match provider {
        Provider::DeepSeek => {
            let available = body.get("is_available").and_then(Value::as_bool).unwrap_or(true);
            let items = body.get("balance_infos").and_then(Value::as_array)?;
            let rows: Vec<Value> = items
                .iter()
                .filter_map(|item| {
                    let remaining = as_f64(item.get("total_balance"))?;
                    Some(json!({
                        "planName": item.get("currency").and_then(Value::as_str).unwrap_or("CNY"),
                        "remaining": remaining,
                        "unit": item.get("currency").and_then(Value::as_str).unwrap_or("CNY"),
                        "isValid": available,
                    }))
                })
                .collect();
            rows.into_iter().next().map(|row| result("deepseek", row))
        }
        Provider::StepFun => as_f64(body.get("balance")).map(|remaining| {
            result(
                "stepfun",
                json!({"planName": "StepFun", "remaining": remaining, "unit": "CNY", "isValid": true}),
            )
        }),
        Provider::SiliconFlowCn | Provider::SiliconFlowEn => {
            let data = body.get("data").unwrap_or(body);
            let remaining = as_f64(data.get("totalBalance").or_else(|| data.get("balance")))?;
            let provider_name = if provider == Provider::SiliconFlowCn {
                "siliconflow"
            } else {
                "siliconflow_en"
            };
            let unit = if provider == Provider::SiliconFlowCn { "CNY" } else { "USD" };
            Some(result(provider_name, json!({"planName": provider_name, "remaining": remaining, "unit": unit, "isValid": true})))
        }
        Provider::OpenRouter => {
            let data = body.get("data").unwrap_or(body);
            let total = as_f64(data.get("total_credits"))?;
            let used = as_f64(data.get("total_usage")).unwrap_or(0.0);
            let remaining = total - used;
            Some(result(
                "openrouter",
                json!({"planName": "OpenRouter", "remaining": remaining, "total": total, "used": used, "unit": "USD", "isValid": remaining > 0.0}),
            ))
        }
        Provider::Novita => as_f64(body.get("availableBalance")).map(|value| {
            result(
                "novita",
                json!({"planName": "Novita AI", "remaining": value / 10000.0, "unit": "USD", "isValid": value > 0.0}),
            )
        }),
    }
}

fn endpoint(provider: Provider) -> (&'static str, &'static str) {
    match provider {
        Provider::DeepSeek => ("https://api.deepseek.com/user/balance", "deepseek"),
        Provider::StepFun => ("https://api.stepfun.com/v1/accounts", "stepfun"),
        Provider::SiliconFlowCn => ("https://api.siliconflow.cn/v1/user/info", "siliconflow"),
        Provider::SiliconFlowEn => ("https://api.siliconflow.com/v1/user/info", "siliconflow_en"),
        Provider::OpenRouter => ("https://openrouter.ai/api/v1/credits", "openrouter"),
        Provider::Novita => ("https://api.novita.ai/v3/user/balance", "novita"),
    }
}

/// Query a provider-specific balance endpoint. `None` means the host is not a
/// known official balance provider and should use the generic fallback.
pub async fn query(base_url: &str, api_key: &str) -> Result<Option<Value>, String> {
    let Some(provider) = detect_provider(base_url) else {
        return Ok(None);
    };
    let (_, provider_name) = endpoint(provider);
    if api_key.trim().is_empty() {
        return Ok(Some(failure(provider_name, "API key is empty")));
    }
    let (url, provider_name) = endpoint(provider);
    let body = match request_json(url, api_key).await? {
        Ok(body) => body,
        Err(error) => return Ok(Some(failure(provider_name, error))),
    };
    Ok(Some(parse(provider, &body).unwrap_or_else(|| {
        failure(
            provider_name,
            "Provider returned no recognized balance fields",
        )
    })))
}

#[cfg(test)]
mod tests {
    use super::{detect_provider, parse, Provider};
    use serde_json::json;

    #[test]
    fn detects_official_balance_hosts() {
        assert_eq!(
            detect_provider("https://api.deepseek.com/v1"),
            Some(Provider::DeepSeek)
        );
        assert_eq!(
            detect_provider("https://openrouter.ai/api/v1"),
            Some(Provider::OpenRouter)
        );
        assert_eq!(detect_provider("https://relay.example.test"), None);
    }

    #[test]
    fn normalizes_openrouter_credits() {
        let value = parse(
            Provider::OpenRouter,
            &json!({"data": {"total_credits": 10, "total_usage": 2.5}}),
        )
        .expect("OpenRouter response should parse");
        assert_eq!(value["data"][0]["remaining"], 7.5);
        assert_eq!(value["data"][0]["used"], 2.5);
    }

    #[test]
    fn converts_novita_units() {
        let value = parse(Provider::Novita, &json!({"availableBalance": 12500}))
            .expect("Novita response should parse");
        assert_eq!(value["data"][0]["remaining"], 1.25);
    }
}
