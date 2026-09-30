//! Provider usage compatibility commands.
//!
//! These endpoints deliberately return normalized JSON instead of pretending
//! that every provider exposes the same billing schema.  A provider response is
//! kept private to the caller and converted to a small, stable result shape.

use std::time::Duration;

use serde_json::{json, Value};
use tauri::State;

use crate::commands::extra_commands::{read_all_config_profiles_from_conn, ConfigProfile};
use crate::db::DbState;
use crate::shared::usage_http::FailureKind;

fn validate_base_url(raw: &str) -> Result<url::Url, String> {
    let parsed =
        url::Url::parse(raw.trim()).map_err(|error| format!("Invalid base URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(
            "Base URL must use HTTP(S) with a host and no embedded credentials".to_string(),
        );
    }
    Ok(parsed)
}

fn endpoint_candidates(base: &url::Url, paths: &[&str]) -> Vec<url::Url> {
    let mut candidates = Vec::new();
    let base_path = base.path().trim_end_matches('/');
    for path in paths {
        let mut candidate = base.clone();
        candidate.set_path(&format!("{base_path}/{}", path.trim_start_matches('/')));
        candidate.set_query(None);
        candidate.set_fragment(None);
        if !candidates.iter().any(|item: &url::Url| item == &candidate) {
            candidates.push(candidate);
        }
    }
    candidates
}

mod normalize;
use normalize::{normalize_usage, quota_from_usage};

#[derive(Debug, Clone, serde::Serialize)]
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
    let provider = base.host_str().unwrap_or("provider").to_string();
    let failure = |message: String| json!({"success": false, "provider": provider, "data": [], "error": message});
    if key.is_empty() {
        return Ok(failure("API key is required".into()));
    }
    let client = crate::shared::usage_http::client_for_url(&base)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut last_error = None;
    let mut transient_error = None;
    for endpoint in endpoint_candidates(&base, paths) {
        match crate::shared::usage_http::request_json(
            &client,
            endpoint.as_str(),
            key,
            true,
            deadline,
        )
        .await
        {
            Ok(Ok(payload)) => {
                let result = normalize_usage(&provider, &payload);
                if result["success"] == true {
                    return Ok(result);
                }
                last_error = Some("Provider returned no recognized usage fields".to_string());
            }
            Ok(Err(error)) => {
                if matches!(
                    error.kind,
                    FailureKind::Http(
                        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
                    )
                ) {
                    return Ok(failure(error.message));
                }
                if error.kind == FailureKind::InvalidRequest {
                    return Ok(failure(error.message));
                }
                last_error = Some(error.message);
            }
            Err(error) => {
                transient_error = Some(error);
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
            }
        }
    }
    if let Some(error) = transient_error {
        return Err(error);
    }
    Ok(failure(last_error.unwrap_or_else(|| {
        "Provider does not expose a supported usage endpoint".into()
    })))
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
    Ok(quota_from_usage(&provider, &result))
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
    let profile = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == provider_id && profile.tool_id == app)
            .ok_or_else(|| format!("Provider not found: {provider_id}"))?;
        profile
    };
    query_profile_usage(&profile).await
}

pub(crate) fn usage_identity(profile: &ConfigProfile) -> Result<String, String> {
    let (base_url, api_key) = config_credentials(&profile.tool_id, &profile.config_snapshot)?;
    let value = if let Some(mut script) = configured_usage_script(&profile.config_snapshot)? {
        script.api_key = Some(script.api_key.unwrap_or(api_key));
        script.base_url = Some(script.base_url.unwrap_or(base_url));
        json!([profile.tool_id, script])
    } else {
        json!([profile.tool_id, base_url.trim(), api_key.trim()])
    };
    Ok(crate::usage_alerts::engine::hash(&value.to_string()))
}

pub(crate) async fn query_profile_usage(profile: &ConfigProfile) -> Result<Value, String> {
    let provider_id = profile.id.clone();
    let (base_url, api_key) = config_credentials(&profile.tool_id, &profile.config_snapshot)?;
    if let Some(script) = configured_usage_script(&profile.config_snapshot)? {
        let result = crate::commands::extended_compat::testUsageScript(
            provider_id.clone(),
            profile.tool_id.clone(),
            script.code,
            script.timeout,
            script.api_key.or_else(|| Some(api_key.clone())),
            script.base_url.or_else(|| Some(base_url.clone())),
            script.access_token,
            script.user_id,
            script.template_type,
        )
        .await?;
        return if result.success {
            Ok(normalize_usage(
                &provider_id,
                result.data.as_ref().unwrap_or(&Value::Null),
            ))
        } else {
            serde_json::to_value(result).map_err(|error| error.to_string())
        };
    };
    if base_url.trim().is_empty() {
        return Ok(json!({
            "success": false,
            "provider": provider_id,
            "data": [],
            "error": "Provider does not declare a usage base URL"
        }));
    }
    if let Some(quota) = crate::commands::coding_plan::query(&base_url, &api_key, None).await? {
        return Ok(normalize_usage(&provider_id, &quota));
    }
    if let Some(result) = crate::commands::balance::query(&base_url, &api_key).await? {
        return Ok(result);
    }
    query_usage(&base_url, &api_key, &["usage", "quota", "balance"]).await
}

#[cfg(test)]
mod query_tests;

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
