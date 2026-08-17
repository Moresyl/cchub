use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tauri::State;

use crate::codex_oauth::{CodexAccount, CodexAuthStatus, CodexDeviceCodeResponse, CodexOAuthState};
use crate::db::DbState;

const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const CODEX_MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";
const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_ERROR_BODY_CHARS: usize = 512;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuotaTier {
    pub name: String,
    pub utilization: f64,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCliQuota {
    pub tool: String,
    pub credential_status: String,
    pub credential_message: Option<String>,
    pub success: bool,
    pub tiers: Vec<CodexQuotaTier>,
    pub error: Option<String>,
    pub queried_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCliModel {
    pub id: String,
    pub display_name: Option<String>,
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone)]
struct CodexCredentials {
    access_token: Option<String>,
    account_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ClaudeCredentials {
    access_token: Option<String>,
    expires_at: Option<i64>,
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn quota_not_found(message: Option<String>) -> CodexCliQuota {
    CodexCliQuota {
        tool: "codex".to_string(),
        credential_status: "not_found".to_string(),
        credential_message: message,
        success: false,
        tiers: Vec::new(),
        error: None,
        queried_at: None,
    }
}

fn quota_error(status: &str, message: String) -> CodexCliQuota {
    CodexCliQuota {
        tool: "codex".to_string(),
        credential_status: status.to_string(),
        credential_message: Some(message.clone()),
        success: false,
        tiers: Vec::new(),
        error: Some(message),
        queried_at: Some(now_millis()),
    }
}

fn auth_path(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    Ok(crate::commands::extra_commands::resolve_tool_config_dir(conn, "codex")?.join("auth.json"))
}

fn read_credentials(conn: &rusqlite::Connection) -> Result<CodexCredentials, CodexCliQuota> {
    let path = auth_path(conn).map_err(|error| quota_error("parse_error", error))?;
    if !path.is_file() {
        return Err(quota_not_found(Some(format!(
            "No Codex auth file found at {}",
            path.display()
        ))));
    }

    let content = std::fs::read_to_string(&path).map_err(|error| {
        quota_error(
            "parse_error",
            format!("Failed to read Codex auth file: {error}"),
        )
    })?;
    parse_credentials(&content).map_err(|message| quota_error("parse_error", message))
}

fn claude_auth_path(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    Ok(
        crate::commands::extra_commands::resolve_tool_config_dir(conn, "claude")?
            .join(".credentials.json"),
    )
}

fn parse_timestamp(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number.as_i64().map(|value| {
            if value > 10_000_000_000 {
                value / 1_000
            } else {
                value
            }
        }),
        Some(Value::String(text)) => text.parse::<i64>().ok().or_else(|| {
            chrono::DateTime::parse_from_rfc3339(text)
                .ok()
                .map(|date| date.timestamp())
        }),
        _ => None,
    }
}

fn read_claude_credentials(
    conn: &rusqlite::Connection,
) -> Result<ClaudeCredentials, CodexCliQuota> {
    let path = claude_auth_path(conn).map_err(|error| quota_error("parse_error", error))?;
    if !path.is_file() {
        return Err(quota_not_found(Some(format!(
            "No Claude OAuth file found at {}",
            path.display()
        ))));
    }
    let content = std::fs::read_to_string(&path).map_err(|error| {
        quota_error(
            "parse_error",
            format!("Failed to read Claude OAuth file: {error}"),
        )
    })?;
    let value: Value = serde_json::from_str(&content).map_err(|error| {
        quota_error(
            "parse_error",
            format!("Failed to parse Claude OAuth JSON: {error}"),
        )
    })?;
    let oauth = value
        .get("claudeAiOauth")
        .or_else(|| value.get("claude.ai_oauth"))
        .and_then(Value::as_object)
        .ok_or_else(|| quota_error("not_found", "Claude OAuth entry is missing".to_string()))?;
    let token = oauth
        .get("accessToken")
        .or_else(|| oauth.get("access_token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            quota_error(
                "parse_error",
                "Claude OAuth access token is missing".to_string(),
            )
        })?
        .to_string();
    Ok(ClaudeCredentials {
        access_token: Some(token),
        expires_at: parse_timestamp(oauth.get("expiresAt").or_else(|| oauth.get("expires_at"))),
    })
}

fn parse_credentials(content: &str) -> Result<CodexCredentials, String> {
    let value: Value = serde_json::from_str(content)
        .map_err(|error| format!("Failed to parse Codex auth JSON: {error}"))?;
    let auth_mode = value
        .get("auth_mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !auth_mode.eq_ignore_ascii_case("chatgpt") {
        return Err("Codex is not using browser OAuth mode".to_string());
    }

    let tokens = value
        .get("tokens")
        .and_then(Value::as_object)
        .ok_or_else(|| "Codex auth JSON does not contain tokens".to_string())?;
    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "Codex OAuth access token is missing".to_string())?
        .to_string();
    let account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string);

    Ok(CodexCredentials {
        access_token: Some(access_token),
        account_id,
    })
}

fn proxy_url(conn: &rusqlite::Connection) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'proxy_url'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .filter(|value| !value.trim().is_empty())
}

fn build_client(conn: &rusqlite::Connection) -> Result<reqwest::Client, String> {
    crate::shared::http_client::build_http_client(
        proxy_url(conn).as_deref(),
        Some("CCHub Codex OAuth"),
        REQUEST_TIMEOUT,
    )
}

fn truncate_body(body: String) -> String {
    if body.chars().count() <= MAX_ERROR_BODY_CHARS {
        body
    } else {
        let mut truncated = body.chars().take(MAX_ERROR_BODY_CHARS).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(|value| match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    })
}

fn integer(value: Option<&Value>) -> Option<i64> {
    number(value).map(|value| value as i64)
}

fn tier_name(window_seconds: Option<i64>) -> String {
    match window_seconds {
        Some(18_000) => "five_hour".to_string(),
        Some(604_800) => "seven_day".to_string(),
        Some(2_592_000) => "thirty_day".to_string(),
        Some(seconds) if seconds >= 86_400 => format!("{}_day", seconds / 86_400),
        Some(seconds) if seconds >= 3_600 => format!("{}_hour", seconds / 3_600),
        Some(seconds) => format!("{}_second", seconds),
        None => "unknown".to_string(),
    }
}

fn timestamp_to_rfc3339(value: Option<&Value>) -> Option<String> {
    let timestamp = integer(value)?;
    let seconds = if timestamp > 10_000_000_000 {
        timestamp / 1_000
    } else {
        timestamp
    };
    chrono::DateTime::from_timestamp(seconds, 0).map(|date| date.to_rfc3339())
}

fn parse_quota(value: &Value) -> Vec<CodexQuotaTier> {
    let Some(rate_limit) = value.get("rate_limit").and_then(Value::as_object) else {
        return Vec::new();
    };
    ["primary_window", "secondary_window"]
        .into_iter()
        .filter_map(|key| rate_limit.get(key).and_then(Value::as_object))
        .filter_map(|window| {
            let utilization = number(window.get("used_percent"))?;
            Some(CodexQuotaTier {
                name: tier_name(integer(window.get("limit_window_seconds"))),
                utilization: utilization.clamp(0.0, 100.0),
                resets_at: timestamp_to_rfc3339(window.get("reset_at")),
            })
        })
        .collect()
}

fn parse_claude_quota(value: &Value) -> Vec<CodexQuotaTier> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    let mut tiers = object
        .iter()
        .filter_map(|(name, entry)| {
            let window = entry.as_object()?;
            let utilization = number(window.get("utilization"))?;
            Some(CodexQuotaTier {
                name: name.clone(),
                utilization: utilization.clamp(0.0, 100.0),
                resets_at: window
                    .get("resets_at")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect::<Vec<_>>();
    tiers.sort_by(|left, right| left.name.cmp(&right.name));
    tiers
}

fn string_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| object.get(*key))
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

fn parse_models(value: &Value) -> Vec<CodexCliModel> {
    let mut models = Vec::new();
    let entries = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.get("models").and_then(Value::as_array))
        .or_else(|| value.get("items").and_then(Value::as_array))
        .or_else(|| value.as_array());

    if let Some(entries) = entries {
        for entry in entries {
            push_model(&mut models, entry, None);
        }
    }
    if let Some(map) = value.get("models").and_then(Value::as_object) {
        for (key, entry) in map {
            push_model(&mut models, entry, Some(key));
        }
    }
    models.sort_by(|left, right| left.id.cmp(&right.id));
    models.dedup_by(|left, right| left.id == right.id);
    models
}

fn push_model(models: &mut Vec<CodexCliModel>, value: &Value, fallback_id: Option<&str>) {
    if let Some(id) = value.as_str().map(str::trim).filter(|id| !id.is_empty()) {
        models.push(CodexCliModel {
            id: id.to_string(),
            display_name: None,
            owned_by: Some("Codex".to_string()),
        });
        return;
    }
    let Some(object) = value.as_object() else {
        if let Some(id) = fallback_id.filter(|id| !id.trim().is_empty()) {
            models.push(CodexCliModel {
                id: id.trim().to_string(),
                display_name: None,
                owned_by: Some("Codex".to_string()),
            });
        }
        return;
    };
    let id = string_field(object, &["slug", "id", "model", "name"])
        .or_else(|| fallback_id.map(str::to_string));
    let Some(id) = id.filter(|id| !id.trim().is_empty()) else {
        return;
    };
    models.push(CodexCliModel {
        id,
        display_name: string_field(object, &["display_name", "displayName", "label"]),
        owned_by: string_field(object, &["owned_by", "ownedBy", "provider", "vendor"])
            .or_else(|| Some("Codex".to_string())),
    });
}

#[tauri::command]
pub async fn get_codex_cli_quota(db: State<'_, DbState>) -> Result<CodexCliQuota, String> {
    let (credentials, client) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let credentials = match read_credentials(&conn) {
            Ok(credentials) => credentials,
            Err(result) => return Ok(result),
        };
        let client = build_client(&conn)?;
        (credentials, client)
    };
    let Some(token) = credentials.access_token else {
        return Ok(quota_not_found(Some(
            "Codex OAuth token is missing".to_string(),
        )));
    };
    let mut request = client
        .get(CODEX_USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "codex-cli")
        .header("Accept", "application/json");
    if let Some(account_id) = credentials.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("Codex quota request failed: {error}"))?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Ok(quota_error(
            "expired",
            format!("Codex OAuth token was rejected (HTTP {status})"),
        ));
    }
    if !status.is_success() {
        return Ok(quota_error(
            "valid",
            format!(
                "Codex quota API returned HTTP {status}: {}",
                truncate_body(response.text().await.unwrap_or_default())
            ),
        ));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse Codex quota response: {error}"))?;
    Ok(CodexCliQuota {
        tool: "codex".to_string(),
        credential_status: "valid".to_string(),
        credential_message: None,
        success: true,
        tiers: parse_quota(&value),
        error: None,
        queried_at: Some(now_millis()),
    })
}

#[tauri::command]
pub async fn get_codex_cli_models(db: State<'_, DbState>) -> Result<Vec<CodexCliModel>, String> {
    let (credentials, client) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let credentials = read_credentials(&conn).map_err(|result| {
            result
                .error
                .unwrap_or_else(|| "Codex OAuth is unavailable".to_string())
        })?;
        let client = build_client(&conn)?;
        (credentials, client)
    };
    let token = credentials
        .access_token
        .ok_or_else(|| "Codex OAuth token is missing".to_string())?;
    let mut request = client
        .get(CODEX_MODELS_URL)
        .query(&[("client_version", env!("CARGO_PKG_VERSION"))])
        .header("Authorization", format!("Bearer {token}"))
        .header("originator", "cchub");
    if let Some(account_id) = credentials.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("Codex model request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Codex model API returned HTTP {status}: {}",
            truncate_body(response.text().await.unwrap_or_default())
        ));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse Codex model response: {error}"))?;
    Ok(parse_models(&value))
}

#[tauri::command]
pub async fn get_codex_oauth_quota(
    account_id: Option<String>,
    state: State<'_, CodexOAuthState>,
    db: State<'_, DbState>,
) -> Result<CodexCliQuota, String> {
    let resolved_account_id = match account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(value.to_string()),
        None => state.0.default_account_id().await,
    };
    let token = match state
        .0
        .get_valid_token(resolved_account_id.as_deref())
        .await
    {
        Ok(token) => token,
        Err(error) => {
            return Ok(CodexCliQuota {
                tool: "codex_oauth".to_string(),
                ..quota_not_found(Some(error.to_string()))
            })
        }
    };
    let client = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        build_client(&conn)?
    };
    let mut request = client
        .get(CODEX_USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "CCHub OAuth")
        .header("Accept", "application/json");
    if let Some(id) = resolved_account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", id);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("OAuth quota request failed: {error}"))?;
    let status = response.status();
    if matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        return Ok(CodexCliQuota {
            tool: "codex_oauth".to_string(),
            ..quota_error(
                "expired",
                format!("OAuth token was rejected (HTTP {status})"),
            )
        });
    }
    if !status.is_success() {
        return Ok(CodexCliQuota {
            tool: "codex_oauth".to_string(),
            ..quota_error(
                "valid",
                format!(
                    "OAuth quota API returned HTTP {status}: {}",
                    truncate_body(response.text().await.unwrap_or_default())
                ),
            )
        });
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse OAuth quota response: {error}"))?;
    Ok(CodexCliQuota {
        tool: "codex_oauth".to_string(),
        credential_status: "valid".to_string(),
        credential_message: None,
        success: true,
        tiers: parse_quota(&value),
        error: None,
        queried_at: Some(now_millis()),
    })
}

#[tauri::command]
pub async fn get_codex_oauth_models(
    account_id: Option<String>,
    state: State<'_, CodexOAuthState>,
    db: State<'_, DbState>,
) -> Result<Vec<CodexCliModel>, String> {
    let resolved_account_id = match account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(value.to_string()),
        None => state.0.default_account_id().await,
    };
    let token = state
        .0
        .get_valid_token(resolved_account_id.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    let client = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        build_client(&conn)?
    };
    let mut request = client
        .get(CODEX_MODELS_URL)
        .query(&[("client_version", env!("CARGO_PKG_VERSION"))])
        .header("Authorization", format!("Bearer {token}"))
        .header("originator", "cchub");
    if let Some(id) = resolved_account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", id);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("OAuth model request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "OAuth model API returned HTTP {status}: {}",
            truncate_body(response.text().await.unwrap_or_default())
        ));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse OAuth model response: {error}"))?;
    Ok(parse_models(&value))
}

#[tauri::command]
pub async fn codex_oauth_start_device_flow(
    state: State<'_, CodexOAuthState>,
) -> Result<CodexDeviceCodeResponse, String> {
    state
        .0
        .start_device_flow()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn codex_oauth_poll_for_account(
    device_code: String,
    state: State<'_, CodexOAuthState>,
) -> Result<Option<CodexAccount>, String> {
    state
        .0
        .poll_for_account(&device_code)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn codex_oauth_list_accounts(
    state: State<'_, CodexOAuthState>,
) -> Result<Vec<CodexAccount>, String> {
    Ok(state.0.list_accounts().await)
}

#[tauri::command]
pub async fn codex_oauth_get_status(
    state: State<'_, CodexOAuthState>,
) -> Result<CodexAuthStatus, String> {
    Ok(state.0.get_status().await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn codex_oauth_remove_account(
    account_id: String,
    state: State<'_, CodexOAuthState>,
) -> Result<(), String> {
    state
        .0
        .remove_account(&account_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn codex_oauth_set_default_account(
    account_id: String,
    state: State<'_, CodexOAuthState>,
) -> Result<(), String> {
    state
        .0
        .set_default_account(&account_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn codex_oauth_logout(state: State<'_, CodexOAuthState>) -> Result<(), String> {
    state
        .0
        .clear_auth()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_claude_cli_quota(db: State<'_, DbState>) -> Result<CodexCliQuota, String> {
    let (credentials, client) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let credentials = match read_claude_credentials(&conn) {
            Ok(credentials) => credentials,
            Err(result) => {
                return Ok(CodexCliQuota {
                    tool: "claude".to_string(),
                    ..result
                })
            }
        };
        let client = crate::shared::http_client::build_http_client(
            proxy_url(&conn).as_deref(),
            Some("CCHub Claude OAuth"),
            REQUEST_TIMEOUT,
        )?;
        (credentials, client)
    };
    if credentials
        .expires_at
        .is_some_and(|value| value <= chrono::Utc::now().timestamp())
    {
        return Ok(CodexCliQuota {
            tool: "claude".to_string(),
            ..quota_error(
                "expired",
                "Claude OAuth token has expired; re-login with the CLI".to_string(),
            )
        });
    }
    let Some(token) = credentials.access_token else {
        return Ok(CodexCliQuota {
            tool: "claude".to_string(),
            ..quota_not_found(None)
        });
    };
    let response = client
        .get(CLAUDE_USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| format!("Claude quota request failed: {error}"))?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Ok(CodexCliQuota {
            tool: "claude".to_string(),
            ..quota_error(
                "expired",
                format!("Claude OAuth token was rejected (HTTP {status})"),
            )
        });
    }
    if !status.is_success() {
        return Ok(CodexCliQuota {
            tool: "claude".to_string(),
            ..quota_error(
                "valid",
                format!(
                    "Claude quota API returned HTTP {status}: {}",
                    truncate_body(response.text().await.unwrap_or_default())
                ),
            )
        });
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse Claude quota response: {error}"))?;
    Ok(CodexCliQuota {
        tool: "claude".to_string(),
        credential_status: "valid".to_string(),
        credential_message: None,
        success: true,
        tiers: parse_claude_quota(&value),
        error: None,
        queried_at: Some(now_millis()),
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_subscription_quota(
    tool_id: String,
    db: State<'_, DbState>,
) -> Result<CodexCliQuota, String> {
    match tool_id.trim().to_ascii_lowercase().as_str() {
        "claude" => get_claude_cli_quota(db).await,
        "codex" => get_codex_cli_quota(db).await,
        other => Ok(CodexCliQuota {
            tool: other.to_string(),
            ..quota_not_found(Some(
                "Subscription quota is not available for this tool".to_string(),
            ))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_claude_quota, parse_credentials, parse_models, parse_quota, tier_name};
    use serde_json::json;

    #[test]
    fn parses_chatgpt_auth_without_exposing_token() {
        let credentials = parse_credentials(
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"secret","account_id":"acct"}}"#,
        )
        .expect("credentials should parse");
        assert_eq!(credentials.account_id.as_deref(), Some("acct"));
        assert_eq!(credentials.access_token.as_deref(), Some("secret"));
    }

    #[test]
    fn rejects_api_key_mode() {
        assert!(
            parse_credentials(r#"{"auth_mode":"apikey","tokens":{"access_token":"secret"}}"#,)
                .is_err()
        );
    }

    #[test]
    fn parses_quota_windows_and_normalizes_percent() {
        let tiers = parse_quota(&json!({
            "rate_limit": {
                "primary_window": {"used_percent": 105, "limit_window_seconds": 18000, "reset_at": 1700000000},
                "secondary_window": {"used_percent": "12.5", "limit_window_seconds": 604800}
            }
        }));
        assert_eq!(tiers[0].name, "five_hour");
        assert_eq!(tiers[0].utilization, 100.0);
        assert_eq!(tiers[1].name, "seven_day");
    }

    #[test]
    fn parses_and_deduplicates_model_shapes() {
        let models = parse_models(&json!({
            "data": [{"id": "gpt-5"}, {"slug": "gpt-5-mini", "displayName": "Mini"}, {"id": "gpt-5"}]
        }));
        assert_eq!(models.len(), 2);
        assert_eq!(models[1].display_name.as_deref(), Some("Mini"));
    }

    #[test]
    fn maps_known_window_names() {
        assert_eq!(tier_name(Some(2_592_000)), "thirty_day");
    }

    #[test]
    fn parses_claude_usage_windows() {
        let tiers = parse_claude_quota(&json!({
            "five_hour": {"utilization": 41.5, "resets_at": "2026-08-16T08:00:00Z"},
            "extra_usage": {"is_enabled": true}
        }));
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0].name, "five_hour");
        assert_eq!(tiers[0].utilization, 41.5);
    }
}
