use crate::openclaw_config;

#[tauri::command]
pub fn get_openclaw_env() -> Result<openclaw_config::OpenClawEnvConfig, String> {
    openclaw_config::get_env_config()
}

#[tauri::command]
pub fn set_openclaw_env(env: openclaw_config::OpenClawEnvConfig) -> Result<(), String> {
    openclaw_config::set_env_config(&env)
}

#[tauri::command]
pub fn get_openclaw_tools() -> Result<openclaw_config::OpenClawToolsConfig, String> {
    openclaw_config::get_tools_config()
}

#[tauri::command]
pub fn set_openclaw_tools(tools: openclaw_config::OpenClawToolsConfig) -> Result<(), String> {
    openclaw_config::set_tools_config(&tools)
}

#[tauri::command]
pub fn get_openclaw_agents_defaults() -> Result<openclaw_config::OpenClawAgentsDefaults, String> {
    openclaw_config::get_agents_defaults()
}

#[tauri::command]
pub fn set_openclaw_agents_defaults(
    defaults: openclaw_config::OpenClawAgentsDefaults,
) -> Result<(), String> {
    openclaw_config::set_agents_defaults(&defaults)
}

#[tauri::command]
pub fn scan_openclaw_health() -> Result<Vec<openclaw_config::OpenClawHealthWarning>, String> {
    openclaw_config::scan_health()
}

#[tauri::command]
pub fn get_openclaw_status() -> Result<OpenClawStatus, String> {
    Ok(OpenClawStatus {
        installed: openclaw_config::get_config_exists(),
        config_path: openclaw_config::get_config_path(),
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawStatus {
    pub installed: bool,
    pub config_path: String,
}
