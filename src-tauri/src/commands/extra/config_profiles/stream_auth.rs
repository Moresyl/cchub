use tauri::{AppHandle, Manager};

use crate::codex_oauth::CodexOAuthState;
use crate::copilot_auth::{self, CopilotAuthState};
use crate::xai_oauth::XaiOAuthState;

use super::codex::join_api_endpoint;

fn extract_profile_metadata(
    parsed: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    parsed
        .get("metadata")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default()
}

pub(super) fn extract_provider_type(parsed: &serde_json::Value) -> Option<String> {
    extract_profile_metadata(parsed)
        .get("providerType")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn extract_use_full_url(parsed: &serde_json::Value) -> bool {
    extract_profile_metadata(parsed)
        .get("useFullUrl")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn extract_bound_account_id(parsed: &serde_json::Value, provider_name: &str) -> Option<String> {
    extract_profile_metadata(parsed)
        .get("authBinding")
        .and_then(|value| {
            let provider = value.get("authProvider").and_then(|item| item.as_str())?;
            (provider == provider_name)
                .then(|| value.get("accountId").and_then(|item| item.as_str()))
                .flatten()
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn extract_copilot_account_id(parsed: &serde_json::Value) -> Option<String> {
    extract_bound_account_id(parsed, "github_copilot").or_else(|| {
        extract_profile_metadata(parsed)
            .get("githubAccountId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(super) fn build_openai_chat_endpoint(
    base_url: &str,
    provider_type: Option<&str>,
    use_full_url: bool,
) -> String {
    let path = if provider_type == Some("github_copilot") {
        "chat/completions"
    } else {
        "v1/chat/completions"
    };
    join_api_endpoint(base_url, path, use_full_url)
}

pub(super) fn build_openai_responses_endpoint(
    base_url: &str,
    provider_type: Option<&str>,
    use_full_url: bool,
) -> String {
    let path = if provider_type == Some("codex_oauth") {
        "responses"
    } else {
        "v1/responses"
    };
    join_api_endpoint(base_url, path, use_full_url)
}

pub(super) async fn resolve_copilot_headers(
    app_handle: &AppHandle,
    parsed: &serde_json::Value,
) -> Result<Vec<(String, String)>, String> {
    let account_id = extract_copilot_account_id(parsed);
    let manager = app_handle.state::<CopilotAuthState>().0.clone();
    let token = manager
        .get_valid_token_for_account(account_id.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    Ok(copilot_auth::copilot_request_headers(&token))
}

pub(super) async fn resolve_codex_headers(
    app_handle: &AppHandle,
    parsed: &serde_json::Value,
) -> Result<Vec<(String, String)>, String> {
    let account_id = extract_bound_account_id(parsed, "codex_oauth");
    let manager = app_handle.state::<CodexOAuthState>().0.clone();
    let token = manager
        .get_valid_token(account_id.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    let mut headers = vec![("authorization".to_string(), format!("Bearer {token}"))];
    if let Some(account_id) = account_id {
        headers.push(("chatgpt-account-id".to_string(), account_id));
    }
    headers.push(("originator".to_string(), "cchub".to_string()));
    Ok(headers)
}

pub(super) async fn resolve_xai_headers(
    app_handle: &AppHandle,
    parsed: &serde_json::Value,
) -> Result<Vec<(String, String)>, String> {
    let account_id = extract_bound_account_id(parsed, "xai_oauth");
    let manager = app_handle.state::<XaiOAuthState>().0.clone();
    let token = manager
        .get_valid_token(account_id.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    Ok(vec![(
        "authorization".to_string(),
        format!("Bearer {token}"),
    )])
}
