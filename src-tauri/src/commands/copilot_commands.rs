use tauri::State;

use crate::copilot_auth::{
    CopilotAuthState, CopilotAuthStatus, GitHubAccount, GitHubDeviceCodeResponse,
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
