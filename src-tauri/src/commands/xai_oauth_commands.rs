use tauri::State;

use crate::commands::oauth_commands::{CodexCliModel, CodexCliQuota};
use crate::xai_oauth::{XaiAccount, XaiAuthStatus, XaiDeviceCodeResponse, XaiOAuthState};

fn error_message(error: crate::xai_oauth::XaiOAuthError) -> String {
    error.to_string()
}

#[tauri::command]
pub async fn xai_oauth_start_device_flow(
    state: State<'_, XaiOAuthState>,
) -> Result<XaiDeviceCodeResponse, String> {
    state.0.start_device_flow().await.map_err(error_message)
}

#[tauri::command]
pub async fn xai_oauth_poll_for_account(
    device_code: String,
    state: State<'_, XaiOAuthState>,
) -> Result<Option<XaiAccount>, String> {
    state
        .0
        .poll_for_account(&device_code)
        .await
        .map_err(error_message)
}

#[tauri::command]
pub async fn xai_oauth_list_accounts(
    state: State<'_, XaiOAuthState>,
) -> Result<Vec<XaiAccount>, String> {
    Ok(state.0.list_accounts().await)
}

#[tauri::command]
pub async fn xai_oauth_get_status(
    state: State<'_, XaiOAuthState>,
) -> Result<XaiAuthStatus, String> {
    Ok(state.0.get_status().await)
}

#[tauri::command]
pub async fn xai_oauth_remove_account(
    account_id: String,
    state: State<'_, XaiOAuthState>,
) -> Result<(), String> {
    state
        .0
        .remove_account(&account_id)
        .await
        .map_err(error_message)
}

#[tauri::command]
pub async fn xai_oauth_set_default_account(
    account_id: String,
    state: State<'_, XaiOAuthState>,
) -> Result<(), String> {
    state
        .0
        .set_default_account(&account_id)
        .await
        .map_err(error_message)
}

#[tauri::command]
pub async fn xai_oauth_logout(state: State<'_, XaiOAuthState>) -> Result<(), String> {
    state.0.clear_auth().await.map_err(error_message)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_xai_oauth_models(
    account_id: Option<String>,
    state: State<'_, XaiOAuthState>,
) -> Result<Vec<CodexCliModel>, String> {
    let models = state
        .0
        .fetch_models(account_id.as_deref())
        .await
        .map_err(error_message)?
        .into_iter()
        .map(|model| CodexCliModel {
            id: model.id,
            display_name: None,
            owned_by: model.owned_by,
        })
        .collect::<Vec<_>>();
    Ok(models)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_xai_oauth_quota(
    account_id: Option<String>,
    state: State<'_, XaiOAuthState>,
) -> Result<CodexCliQuota, String> {
    let result = state.0.get_valid_token(account_id.as_deref()).await;
    match result {
        Ok(_) => Ok(CodexCliQuota {
            tool: "xai_oauth".to_string(),
            credential_status: "valid".to_string(),
            credential_message: Some(
                "xAI authentication is valid; this account does not expose subscription windows through the public API."
                    .to_string(),
            ),
            success: false,
            tiers: Vec::new(),
            error: Some("xAI quota endpoint is not available".to_string()),
            queried_at: Some(chrono::Utc::now().timestamp_millis()),
        }),
        Err(error) => Ok(CodexCliQuota {
            tool: "xai_oauth".to_string(),
            credential_status: "expired".to_string(),
            credential_message: Some(error.to_string()),
            success: false,
            tiers: Vec::new(),
            error: Some(error.to_string()),
            queried_at: Some(chrono::Utc::now().timestamp_millis()),
        }),
    }
}
