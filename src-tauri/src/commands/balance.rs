//! Official balance endpoints for common API providers.
//!
//! Unknown relays intentionally fall back to the generic compatibility query;
//! this module only claims hosts whose endpoint and response shape are known.

use std::time::Duration;

use crate::shared::usage_http::{finite_number as as_f64, official_url};
use serde_json::{json, Value};

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
    match official_url(base_url)?.host_str()? {
        "api.deepseek.com" => Some(Provider::DeepSeek),
        "api.stepfun.ai" | "api.stepfun.com" => Some(Provider::StepFun),
        "api.siliconflow.cn" => Some(Provider::SiliconFlowCn),
        "api.siliconflow.com" => Some(Provider::SiliconFlowEn),
        "openrouter.ai" => Some(Provider::OpenRouter),
        "api.novita.ai" => Some(Provider::Novita),
        _ => None,
    }
}

fn result(provider: &str, row: Value) -> Value {
    json!({"success": true, "provider": provider, "data": [row], "error": null})
}

fn failure(provider: &str, error: impl Into<String>) -> Value {
    json!({"success": false, "provider": provider, "data": [], "error": error.into()})
}

fn parse(provider: Provider, body: &Value) -> Option<Value> {
    match provider {
        Provider::DeepSeek => {
            let available = body.get("is_available").and_then(Value::as_bool);
            let items = body.get("balance_infos").and_then(Value::as_array)?;
            let rows: Vec<Value> = items
                .iter()
                .filter_map(|item| {
                    let remaining = as_f64(item.get("total_balance"))?;
                    let currency = item.get("currency").and_then(Value::as_str)
                        .map(str::trim).filter(|unit| !unit.is_empty());
                    let mut row = json!({
                        "planName": currency.unwrap_or("DeepSeek"),
                        "remaining": remaining,
                    });
                    if let Some(currency) = currency { row["unit"] = json!(currency); }
                    if let Some(available) = available { row["isValid"] = json!(available); }
                    Some(row)
                })
                .collect();
            (!rows.is_empty()).then(|| json!({"success": true, "provider": "deepseek", "data": rows, "error": null}))
        }
        Provider::StepFun => as_f64(body.get("balance")).map(|remaining| {
            result(
                "stepfun",
                json!({"planName": "StepFun", "remaining": remaining, "unit": "CNY", "isValid": remaining > 0.0}),
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
            Some(result(provider_name, json!({"planName": provider_name, "remaining": remaining, "unit": unit, "isValid": remaining > 0.0})))
        }
        Provider::OpenRouter => {
            let data = body.get("data").unwrap_or(body);
            let total = as_f64(data.get("total_credits"))?;
            let used = as_f64(data.get("total_usage"))?;
            if total < 0.0 || used < 0.0 { return None; }
            let remaining = total - used;
            if !remaining.is_finite() { return None; }
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
    let client = crate::shared::usage_http::client()?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let body = match crate::shared::usage_http::request_json(&client, url, api_key, true, deadline)
        .await?
    {
        Ok(body) => body,
        Err(error) => return Ok(Some(failure(provider_name, error.message))),
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

    #[test]
    fn rejects_host_lookalikes_and_embedded_official_names() {
        for url in [
            "https://api.deepseek.com.evil.test",
            "https://evil.test/api.deepseek.com",
            "https://evil.test?host=openrouter.ai",
            "https://openrouter.ai@evil.test",
            "https://user@api.deepseek.com",
            "http://api.deepseek.com",
            "https://api.deepseek.com:8443",
        ] {
            assert_eq!(detect_provider(url), None, "{url}");
        }
    }

    #[test]
    fn keeps_every_valid_currency_without_inventing_a_unit_or_availability() {
        let value = parse(
            Provider::DeepSeek,
            &json!({"balance_infos": [
                {"currency": "USD", "total_balance": " 12.50 "},
                {"currency": "CNY", "total_balance": -1},
                {"total_balance": 3}, {"total_balance": "NaN"}
            ]}),
        )
        .unwrap();
        assert_eq!(value["data"].as_array().unwrap().len(), 3);
        assert_eq!(value["data"][0]["remaining"], 12.5);
        assert_eq!(value["data"][1]["remaining"], -1.0);
        assert!(value["data"][2].get("unit").is_none());
        assert!(value["data"][0].get("isValid").is_none());
    }

    #[test]
    fn missing_or_invalid_used_credits_never_become_zero() {
        for body in [
            json!({"total_credits": 10}),
            json!({"total_credits": 10, "total_usage": "inf"}),
            json!({"total_credits": "1e999", "total_usage": 1}),
        ] {
            assert!(parse(Provider::OpenRouter, &body).is_none());
        }
        let value = parse(
            Provider::OpenRouter,
            &json!({"total_credits": 10, "total_usage": 12}),
        )
        .unwrap();
        assert_eq!(value["data"][0]["remaining"], -2.0);
    }
}
