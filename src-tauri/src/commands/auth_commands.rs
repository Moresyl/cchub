use serde::Serialize;
use tauri::State;

use crate::codex_oauth::{CodexAccount, CodexAuthStatus, CodexDeviceCodeResponse, CodexOAuthState};
use crate::copilot_auth::{
    CopilotAuthState, CopilotAuthStatus, GitHubAccount, GitHubDeviceCodeResponse,
};
use crate::xai_oauth::{XaiAccount, XaiAuthStatus, XaiDeviceCodeResponse, XaiOAuthState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthAccount {
    pub id: String,
    pub login: String,
    pub avatar_url: Option<String>,
    pub authenticated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthDeviceCode {
    pub provider: String,
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub provider: String,
    pub authenticated: bool,
    pub accounts: Vec<AuthAccount>,
    pub default_account_id: Option<String>,
    pub username: Option<String>,
}

fn provider_name(value: Option<&str>) -> Result<&str, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("copilot") | Some("github_copilot") => Ok("github_copilot"),
        Some("codex") | Some("codex_oauth") => Ok("codex_oauth"),
        Some("xai") | Some("xai_oauth") => Ok("xai_oauth"),
        Some(value) => Err(format!("Unsupported auth provider: {value}")),
        None => Err("Auth provider is required".to_string()),
    }
}

fn map_github_account(account: GitHubAccount) -> AuthAccount {
    AuthAccount {
        id: account.id,
        login: account.login,
        avatar_url: account.avatar_url,
        authenticated_at: account.authenticated_at,
    }
}

fn map_codex_account(account: CodexAccount) -> AuthAccount {
    AuthAccount {
        id: account.id,
        login: account.login,
        avatar_url: None,
        authenticated_at: account.authenticated_at,
    }
}

fn map_xai_account(account: XaiAccount) -> AuthAccount {
    AuthAccount {
        id: account.id,
        login: account.login,
        avatar_url: None,
        authenticated_at: account.authenticated_at,
    }
}

#[tauri::command]
pub async fn auth_start_login(
    provider: Option<String>,
    copilot: State<'_, CopilotAuthState>,
    codex: State<'_, CodexOAuthState>,
    xai: State<'_, XaiOAuthState>,
) -> Result<AuthDeviceCode, String> {
    match provider_name(provider.as_deref())? {
        "github_copilot" => {
            let flow: GitHubDeviceCodeResponse = copilot
                .0
                .start_device_flow()
                .await
                .map_err(|e| e.to_string())?;
            Ok(AuthDeviceCode {
                provider: "github_copilot".to_string(),
                device_code: flow.device_code,
                user_code: flow.user_code,
                verification_uri: flow.verification_uri,
                expires_in: flow.expires_in,
                interval: flow.interval,
            })
        }
        "codex_oauth" => {
            let flow: CodexDeviceCodeResponse = codex
                .0
                .start_device_flow()
                .await
                .map_err(|e| e.to_string())?;
            Ok(AuthDeviceCode {
                provider: "codex_oauth".to_string(),
                device_code: flow.device_code,
                user_code: flow.user_code,
                verification_uri: flow.verification_uri,
                expires_in: flow.expires_in,
                interval: flow.interval,
            })
        }
        "xai_oauth" => {
            let flow: XaiDeviceCodeResponse =
                xai.0.start_device_flow().await.map_err(|e| e.to_string())?;
            Ok(AuthDeviceCode {
                provider: "xai_oauth".to_string(),
                device_code: flow.device_code,
                user_code: flow.user_code,
                verification_uri: flow.verification_uri,
                expires_in: flow.expires_in,
                interval: flow.interval,
            })
        }
        _ => unreachable!(),
    }
}

#[tauri::command]
pub async fn auth_poll_for_account(
    provider: String,
    device_code: String,
    copilot: State<'_, CopilotAuthState>,
    codex: State<'_, CodexOAuthState>,
    xai: State<'_, XaiOAuthState>,
) -> Result<Option<AuthAccount>, String> {
    match provider_name(Some(&provider))? {
        "github_copilot" => copilot
            .0
            .poll_for_token(&device_code)
            .await
            .map(|v| v.map(map_github_account))
            .map_err(|e| e.to_string()),
        "codex_oauth" => codex
            .0
            .poll_for_account(&device_code)
            .await
            .map(|v| v.map(map_codex_account))
            .map_err(|e| e.to_string()),
        "xai_oauth" => xai
            .0
            .poll_for_account(&device_code)
            .await
            .map(|v| v.map(map_xai_account))
            .map_err(|e| e.to_string()),
        _ => unreachable!(),
    }
}

#[tauri::command]
pub async fn auth_get_status(
    provider: String,
    copilot: State<'_, CopilotAuthState>,
    codex: State<'_, CodexOAuthState>,
    xai: State<'_, XaiOAuthState>,
) -> Result<AuthStatus, String> {
    match provider_name(Some(&provider))? {
        "github_copilot" => {
            let status: CopilotAuthStatus = copilot.0.get_status().await;
            Ok(AuthStatus {
                provider: "github_copilot".to_string(),
                authenticated: status.authenticated,
                accounts: status
                    .accounts
                    .into_iter()
                    .map(map_github_account)
                    .collect(),
                default_account_id: status.default_account_id,
                username: status.username,
            })
        }
        "codex_oauth" => {
            let status: CodexAuthStatus = codex.0.get_status().await;
            Ok(AuthStatus {
                provider: "codex_oauth".to_string(),
                authenticated: status.authenticated,
                accounts: status.accounts.into_iter().map(map_codex_account).collect(),
                default_account_id: status.default_account_id,
                username: status.username,
            })
        }
        "xai_oauth" => {
            let status: XaiAuthStatus = xai.0.get_status().await;
            Ok(AuthStatus {
                provider: "xai_oauth".to_string(),
                authenticated: status.authenticated,
                accounts: status.accounts.into_iter().map(map_xai_account).collect(),
                default_account_id: status.default_account_id,
                username: status.username,
            })
        }
        _ => unreachable!(),
    }
}

#[tauri::command]
pub async fn auth_list_accounts(
    provider: String,
    copilot: State<'_, CopilotAuthState>,
    codex: State<'_, CodexOAuthState>,
    xai: State<'_, XaiOAuthState>,
) -> Result<Vec<AuthAccount>, String> {
    match provider_name(Some(&provider))? {
        "github_copilot" => Ok(copilot
            .0
            .list_accounts()
            .await
            .into_iter()
            .map(map_github_account)
            .collect()),
        "codex_oauth" => Ok(codex
            .0
            .list_accounts()
            .await
            .into_iter()
            .map(map_codex_account)
            .collect()),
        "xai_oauth" => Ok(xai
            .0
            .list_accounts()
            .await
            .into_iter()
            .map(map_xai_account)
            .collect()),
        _ => unreachable!(),
    }
}

#[tauri::command]
pub async fn auth_set_default_account(
    provider: String,
    account_id: String,
    copilot: State<'_, CopilotAuthState>,
    codex: State<'_, CodexOAuthState>,
    xai: State<'_, XaiOAuthState>,
) -> Result<(), String> {
    match provider_name(Some(&provider))? {
        "github_copilot" => copilot
            .0
            .set_default_account(&account_id)
            .await
            .map_err(|e| e.to_string()),
        "codex_oauth" => codex
            .0
            .set_default_account(&account_id)
            .await
            .map_err(|e| e.to_string()),
        "xai_oauth" => xai
            .0
            .set_default_account(&account_id)
            .await
            .map_err(|e| e.to_string()),
        _ => unreachable!(),
    }
}

#[tauri::command]
pub async fn auth_remove_account(
    provider: String,
    account_id: String,
    copilot: State<'_, CopilotAuthState>,
    codex: State<'_, CodexOAuthState>,
    xai: State<'_, XaiOAuthState>,
) -> Result<(), String> {
    match provider_name(Some(&provider))? {
        "github_copilot" => copilot
            .0
            .remove_account(&account_id)
            .await
            .map_err(|e| e.to_string()),
        "codex_oauth" => codex
            .0
            .remove_account(&account_id)
            .await
            .map_err(|e| e.to_string()),
        "xai_oauth" => xai
            .0
            .remove_account(&account_id)
            .await
            .map_err(|e| e.to_string()),
        _ => unreachable!(),
    }
}

#[tauri::command]
pub async fn auth_logout(
    provider: String,
    copilot: State<'_, CopilotAuthState>,
    codex: State<'_, CodexOAuthState>,
    xai: State<'_, XaiOAuthState>,
) -> Result<(), String> {
    match provider_name(Some(&provider))? {
        "github_copilot" => copilot.0.clear_auth().await.map_err(|e| e.to_string()),
        "codex_oauth" => codex.0.clear_auth().await.map_err(|e| e.to_string()),
        "xai_oauth" => xai.0.clear_auth().await.map_err(|e| e.to_string()),
        _ => unreachable!(),
    }
}
