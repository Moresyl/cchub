use tauri::State;

use crate::copilot_auth::{
    CopilotAuthState, CopilotAuthStatus, CopilotModel, CopilotUsage, GitHubAccount,
    GitHubDeviceCodeResponse,
};

#[tauri::command]
pub async fn copilot_start_device_flow(
    state: State<'_, CopilotAuthState>,
) -> Result<GitHubDeviceCodeResponse, String> {
    let manager = state.0.clone();
    manager
        .start_device_flow()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_poll_for_account(
    device_code: String,
    state: State<'_, CopilotAuthState>,
) -> Result<Option<GitHubAccount>, String> {
    let manager = state.0.clone();
    manager
        .poll_for_token(&device_code)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn copilot_list_accounts(
    state: State<'_, CopilotAuthState>,
) -> Result<Vec<GitHubAccount>, String> {
    let manager = state.0.clone();
    Ok(manager.list_accounts().await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_remove_account(
    account_id: String,
    state: State<'_, CopilotAuthState>,
) -> Result<(), String> {
    let manager = state.0.clone();
    manager
        .remove_account(&account_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_set_default_account(
    account_id: String,
    state: State<'_, CopilotAuthState>,
) -> Result<(), String> {
    let manager = state.0.clone();
    manager
        .set_default_account(&account_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn copilot_logout(state: State<'_, CopilotAuthState>) -> Result<(), String> {
    state
        .0
        .clear_auth()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn copilot_get_auth_status(
    state: State<'_, CopilotAuthState>,
) -> Result<CopilotAuthStatus, String> {
    let manager = state.0.clone();
    Ok(manager.get_status().await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_get_token(
    account_id: Option<String>,
    state: State<'_, CopilotAuthState>,
) -> Result<String, String> {
    let manager = state.0.clone();
    manager
        .get_valid_token_for_account(account_id.as_deref())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_get_usage(
    account_id: Option<String>,
    state: State<'_, CopilotAuthState>,
) -> Result<CopilotUsage, String> {
    state
        .0
        .fetch_usage(account_id.as_deref())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_get_models(
    account_id: Option<String>,
    state: State<'_, CopilotAuthState>,
) -> Result<Vec<CopilotModel>, String> {
    state
        .0
        .fetch_models(account_id.as_deref())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_get_token_for_account(
    account_id: String,
    state: State<'_, CopilotAuthState>,
) -> Result<String, String> {
    state
        .0
        .get_valid_token_for_account(Some(account_id.as_str()))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_get_usage_for_account(
    account_id: String,
    state: State<'_, CopilotAuthState>,
) -> Result<CopilotUsage, String> {
    state
        .0
        .fetch_usage(Some(account_id.as_str()))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_get_models_for_account(
    account_id: String,
    state: State<'_, CopilotAuthState>,
) -> Result<Vec<CopilotModel>, String> {
    state
        .0
        .fetch_models(Some(account_id.as_str()))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn copilot_is_authenticated(state: State<'_, CopilotAuthState>) -> Result<bool, String> {
    Ok(state.0.get_status().await.authenticated)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copilot_poll_for_auth(
    device_code: String,
    state: State<'_, CopilotAuthState>,
) -> Result<Option<GitHubAccount>, String> {
    state
        .0
        .poll_for_token(&device_code)
        .await
        .map_err(|error| error.to_string())
}
