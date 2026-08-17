//! Provider usage compatibility commands.
//!
//! These endpoints deliberately return normalized JSON instead of pretending
//! that every provider exposes the same billing schema.  A provider response is
//! kept private to the caller and converted to a small, stable result shape.

use std::time::Duration;

use serde_json::{json, Value};
use tauri::State;

use crate::commands::extra_commands::read_all_config_profiles_from_conn;
use crate::db::DbState;

fn validate_base_url(raw: &str) -> Result<url::Url, String> {
    let parsed =
        url::Url::parse(raw.trim()).map_err(|error| format!("Invalid base URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Base URL must be an http(s) URL".to_string());
    }
    Ok(parsed)
}

fn endpoint_candidates(base: &url::Url, paths: &[&str]) -> Vec<url::Url> {
    let mut candidates = Vec::new();
    let base_path = base.path().trim_end_matches('/');
    for path in paths {
        let mut candidate = base.clone();
        candidate.set_path(&format!("{base_path}/{}", path.trim_start_matches('/')));
        if !candidates.iter().any(|item: &url::Url| item == &candidate) {
            candidates.push(candidate);
        }
    }
    candidates
}

fn number_at(value: &Value, paths: &[&str]) -> Option<f64> {
    paths.iter().find_map(|path| {
        let mut cursor = value;
        for part in path.split('.') {
            cursor = cursor.get(part)?;
        }
        cursor
            .as_f64()
            .or_else(|| cursor.as_i64().map(|number| number as f64))
    })
}

fn normalize_usage(provider: &str, payload: &Value) -> Value {
    let remaining = number_at(
        payload,
        &[
            "remaining",
            "balance",
            "data.balance",
            "data.remaining",
            "credits",
            "data.credits",
            "total_balance",
            "data.total_balance",
        ],
    );
    let used = number_at(payload, &["used", "data.used", "usage", "data.usage"]);
    let limit = number_at(
        payload,
        &[
            "limit",
            "data.limit",
            "total",
            "data.total",
            "total_credits",
            "data.total_credits",
        ],
    );
    let mut row = json!({ "planName": provider });
    if let Some(value) = remaining {
        row["remaining"] = json!(value);
    }
    if let Some(value) = used {
        row["used"] = json!(value);
    }
    if let Some(value) = limit {
        row["limit"] = json!(value);
    }
    json!({
        "success": remaining.is_some() || used.is_some() || limit.is_some(),
        "provider": provider,
        "data": [row],
        "error": if remaining.is_none() && used.is_none() && limit.is_none() {
            Some("Provider returned no recognized usage fields".to_string())
        } else {
            None
        },
    })
}

#[derive(Debug, Clone)]
struct ConfiguredUsageScript {
    code: String,
    timeout: Option<u64>,
    api_key: Option<String>,
    base_url: Option<String>,
    access_token: Option<String>,
    user_id: Option<String>,
    template_type: Option<String>,
}

fn configured_usage_script(snapshot: &str) -> Result<Option<ConfiguredUsageScript>, String> {
    let value: Value = serde_json::from_str(snapshot).map_err(|error| error.to_string())?;
    let Some(script) = value
        .pointer("/metadata/usageScript")
        .and_then(Value::as_object)
    else {
        return Ok(None);
    };
    if script.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Ok(None);
    }
    let Some(code) = script
        .get("code")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
    else {
        return Ok(None);
    };
    let text = |key: &str| script.get(key).and_then(Value::as_str).map(str::to_string);
    Ok(Some(ConfiguredUsageScript {
        code: code.to_string(),
        timeout: script.get("timeout").and_then(Value::as_u64),
        api_key: text("apiKey"),
        base_url: text("baseUrl"),
        access_token: text("accessToken"),
        user_id: text("userId"),
        template_type: text("templateType"),
    }))
}

async fn query_usage(base_url: &str, api_key: &str, paths: &[&str]) -> Result<Value, String> {
    let base = validate_base_url(base_url)?;
    let key = api_key.trim();
    if key.is_empty() {
        return Ok(json!({
            "success": false,
            "provider": base.host_str().unwrap_or("provider"),
            "data": [],
            "error": "API key is required"
        }));
    }

    let client = crate::shared::http_client::build_http_client(
        None,
        Some("CCHub"),
        Duration::from_secs(20),
    )?;
    let provider = base.host_str().unwrap_or("provider").to_string();
    let mut last_error = None;
    for endpoint in endpoint_candidates(&base, paths) {
        let response = match client.get(endpoint).bearer_auth(key).send().await {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(error.to_string());
                continue;
            }
        };
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            continue;
        }
        if !response.status().is_success() {
            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                return Ok(json!({
                    "success": false,
                    "provider": provider,
                    "data": [],
                    "error": format!("Provider rejected credentials ({status})")
                }));
            }
            last_error = Some(format!("Provider returned HTTP {status}"));
            continue;
        }
        let payload = response
            .json::<Value>()
            .await
            .map_err(|error| format!("Invalid usage response: {error}"))?;
        return Ok(normalize_usage(&provider, &payload));
    }
    Ok(json!({
        "success": false,
        "provider": provider,
        "data": [],
        "error": last_error.unwrap_or_else(|| "Provider does not expose a supported usage endpoint".to_string())
    }))
}

#[tauri::command]
pub async fn get_balance(base_url: String, api_key: String) -> Result<Value, String> {
    if let Some(result) = crate::commands::balance::query(&base_url, &api_key).await? {
        return Ok(result);
    }
    query_usage(
        &base_url,
        &api_key,
        &["balance", "api/v1/dashboard/billing/credit_grants"],
    )
    .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_coding_plan_quota(
    base_url: String,
    api_key: String,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    coding_plan_provider: Option<String>,
    team_organization_id: Option<String>,
    team_project_id: Option<String>,
) -> Result<Value, String> {
    if let Some(result) =
        crate::commands::coding_plan::query(&base_url, &api_key, coding_plan_provider.as_deref())
            .await?
    {
        return Ok(result);
    }
    let _ = (
        access_key_id,
        secret_access_key,
        team_organization_id,
        team_project_id,
    );
    let provider = coding_plan_provider.unwrap_or_else(|| "generic".to_string());
    let result = query_usage(&base_url, &api_key, &["usage", "quota", "api/v1/usage"]).await?;
    if result.get("success").and_then(Value::as_bool) == Some(false) {
        return Ok(json!({
            "status": "not_found",
            "provider": provider,
            "tiers": [],
            "error": result.get("error").cloned().unwrap_or_else(|| json!("Usage endpoint unavailable"))
        }));
    }
    Ok(json!({
        "status": "ok",
        "provider": provider,
        "tiers": result.get("data").cloned().unwrap_or_else(|| json!([]))
    }))
}

fn config_credentials(tool_id: &str, snapshot: &str) -> Result<(String, String), String> {
    let value: Value = serde_json::from_str(snapshot).map_err(|error| error.to_string())?;
    let text = |path: &[&str]| {
        let mut cursor = &value;
        for part in path {
            cursor = cursor.get(*part)?;
        }
        cursor.as_str().map(str::to_string)
    };
    match tool_id {
        "claude" => Ok((
            text(&["env", "ANTHROPIC_BASE_URL"]).unwrap_or_default(),
            text(&["env", "ANTHROPIC_AUTH_TOKEN"])
                .or_else(|| text(&["env", "ANTHROPIC_API_KEY"]))
                .unwrap_or_default(),
        )),
        "gemini" => Ok((
            text(&["env", "GEMINI_BASE_URL"])
                .or_else(|| text(&["env", "GOOGLE_GEMINI_BASE_URL"]))
                .unwrap_or_default(),
            text(&["env", "GEMINI_API_KEY"]).unwrap_or_default(),
        )),
        "openclaw" => Ok((
            text(&["baseUrl"]).unwrap_or_default(),
            text(&["apiKey"]).unwrap_or_default(),
        )),
        "opencode" => Ok((
            text(&["options", "baseURL"])
                .or_else(|| text(&["options", "baseUrl"]))
                .unwrap_or_default(),
            text(&["options", "apiKey"]).unwrap_or_default(),
        )),
        "hermes" => {
            let base_url = text(&["config", "model", "base_url"]).unwrap_or_default();
            let env_name = text(&["metadata", "hermesApiKeyEnv"]);
            let key = env_name
                .as_deref()
                .and_then(|name| value.get("env")?.get(name)?.as_str())
                .unwrap_or_default()
                .to_string();
            Ok((base_url, key))
        }
        "codex" => {
            let config = text(&["config"]).unwrap_or_default();
            let base_url = config
                .lines()
                .find_map(|line| line.trim().strip_prefix("base_url = "))
                .map(|line| line.trim_matches('"').to_string())
                .unwrap_or_default();
            Ok((
                base_url,
                text(&["auth", "OPENAI_API_KEY"]).unwrap_or_default(),
            ))
        }
        "grokbuild" => {
            let config = text(&["config"]).unwrap_or_default();
            let parsed = config.parse::<toml::Value>().ok();
            let fallback_model = text(&["model"]);
            let selected_model = parsed
                .as_ref()
                .and_then(|value| value.get("models"))
                .and_then(|value| value.get("default"))
                .and_then(toml::Value::as_str)
                .or(fallback_model.as_deref())
                .unwrap_or("grok-4.5");
            let selected = parsed
                .as_ref()
                .and_then(|value| value.get("model"))
                .and_then(|value| value.get(selected_model));
            let legacy = parsed.as_ref().and_then(|value| {
                let provider = value.get("model_provider")?.as_str()?;
                value.get("model_providers")?.get(provider)
            });
            let fallback_base_url = text(&["baseUrl"]);
            let base_url = selected
                .and_then(|value| value.get("base_url"))
                .and_then(toml::Value::as_str)
                .or_else(|| {
                    legacy
                        .and_then(|value| value.get("base_url"))
                        .and_then(toml::Value::as_str)
                })
                .or(fallback_base_url.as_deref())
                .unwrap_or_default()
                .to_string();
            let fallback_api_key = text(&["apiKey"]);
            let fallback_auth_key = text(&["auth", "OPENAI_API_KEY"]);
            let key = selected
                .and_then(|value| value.get("api_key"))
                .and_then(toml::Value::as_str)
                .or_else(|| {
                    legacy
                        .and_then(|value| value.get("api_key"))
                        .and_then(toml::Value::as_str)
                })
                .or(fallback_api_key.as_deref())
                .or(fallback_auth_key.as_deref())
                .unwrap_or_default()
                .to_string();
            Ok((base_url, key))
        }
        _ => Err(format!("Unsupported app: {tool_id}")),
    }
}

#[tauri::command(rename_all = "camelCase")]
#[allow(non_snake_case)]
pub async fn queryProviderUsage(
    provider_id: String,
    app: String,
    db: State<'_, DbState>,
) -> Result<Value, String> {
    let (base_url, api_key, profile_snapshot) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == provider_id && profile.tool_id == app)
            .ok_or_else(|| format!("Provider not found: {provider_id}"))?;
        let (base_url, api_key) = config_credentials(&app, &profile.config_snapshot)?;
        (base_url, api_key, profile.config_snapshot)
    };
    if let Some(script) = configured_usage_script(&profile_snapshot)? {
        let result = crate::commands::extended_compat::testUsageScript(
            provider_id,
            app,
            script.code,
            script.timeout,
            script.api_key.or_else(|| Some(api_key.clone())),
            script.base_url.or_else(|| Some(base_url.clone())),
            script.access_token,
            script.user_id,
            script.template_type,
        )
        .await?;
        return serde_json::to_value(result).map_err(|error| error.to_string());
    };
    if base_url.trim().is_empty() {
        return Ok(json!({
            "success": false,
            "provider": provider_id,
            "data": [],
            "error": "Provider does not declare a usage base URL"
        }));
    }
    if let Some(result) = crate::commands::balance::query(&base_url, &api_key).await? {
        return Ok(result);
    }
    query_usage(&base_url, &api_key, &["usage", "quota", "balance"]).await
}

#[cfg(test)]
mod tests {
    use super::{config_credentials, configured_usage_script, normalize_usage};
    use serde_json::json;

    #[test]
    fn normalizes_common_balance_fields() {
        let value = normalize_usage("example", &json!({"data": {"balance": 12.5}}));
        assert_eq!(value["success"], true);
        assert_eq!(value["data"][0]["remaining"], 12.5);
    }

    #[test]
    fn extracts_claude_credentials_without_leaking_other_fields() {
        let (base, key) = config_credentials(
            "claude",
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://example.test","ANTHROPIC_API_KEY":"secret"}}"#,
        )
        .expect("credentials should parse");
        assert_eq!(base, "https://example.test");
        assert_eq!(key, "secret");
    }

    #[test]
    fn only_enabled_non_empty_usage_scripts_are_selected() {
        let disabled = r#"{"metadata":{"usageScript":{"enabled":false,"code":"return {};"}}}"#;
        assert!(configured_usage_script(disabled)
            .expect("disabled script should parse")
            .is_none());

        let enabled = r#"{"metadata":{"usageScript":{"enabled":true,"code":"return {remaining: 1};","timeout":1200,"apiKey":"script-key"}}}"#;
        let script = configured_usage_script(enabled)
            .expect("enabled script should parse")
            .expect("script should be selected");
        assert_eq!(script.code, "return {remaining: 1};");
        assert_eq!(script.timeout, Some(1200));
        assert_eq!(script.api_key.as_deref(), Some("script-key"));
    }
}
