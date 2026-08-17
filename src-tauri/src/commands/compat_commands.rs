use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::extra_commands::{
    apply_config_profile, delete_config_profile, get_active_config_profile_ids_from_conn,
    get_config_profiles, get_environment_conflicts, get_local_auth_status, get_proxy,
    get_tool_environment_report, read_all_config_profiles_from_conn, save_config_profile,
    set_proxy, stream_check_all_config_profiles, stream_check_config_profile,
    update_config_profile, ConfigProfile,
};
use crate::commands::usage_commands::get_recent_proxy_request_logs;
use crate::db::DbState;
use crate::provider_proxy::{
    get_local_provider_proxy_settings, get_local_provider_proxy_status,
    read_local_provider_proxy_settings_from_conn, set_local_provider_proxy_settings,
    LocalProviderProxySettings, LocalProviderProxyStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalProxyConfig {
    pub url: String,
    pub configured: bool,
}

#[tauri::command]
pub fn list_profiles(db: State<'_, DbState>) -> Result<Vec<ConfigProfile>, String> {
    get_config_profiles(db)
}

#[tauri::command]
pub fn create_profile(
    name: String,
    tool_id: String,
    config_snapshot: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    save_config_profile(name, tool_id, config_snapshot, db)
}

#[tauri::command]
pub fn update_profile(
    id: String,
    name: String,
    config_snapshot: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    update_config_profile(id, name, config_snapshot, db)
}

#[tauri::command]
pub fn delete_profile(id: String, db: State<'_, DbState>) -> Result<(), String> {
    delete_config_profile(id, db)
}

#[tauri::command]
pub fn apply_profile(id: String, db: State<'_, DbState>) -> Result<serde_json::Value, String> {
    let result = apply_config_profile(id, db)?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_global_proxy_url(db: State<'_, DbState>) -> String {
    get_proxy(db)
}

#[tauri::command]
pub fn set_global_proxy_url(proxy_url: String, db: State<'_, DbState>) -> Result<(), String> {
    set_proxy(proxy_url, db)
}

#[tauri::command]
pub fn get_proxy_config(db: State<'_, DbState>) -> Result<LocalProviderProxySettings, String> {
    get_local_provider_proxy_settings(db)
}

#[tauri::command]
pub fn get_settings(db: State<'_, DbState>) -> Result<GlobalProxyConfig, String> {
    let url = get_proxy(db);
    Ok(GlobalProxyConfig {
        configured: !url.trim().is_empty(),
        url,
    })
}

#[tauri::command]
pub fn save_settings(
    settings: GlobalProxyConfig,
    db: State<'_, DbState>,
) -> Result<GlobalProxyConfig, String> {
    set_proxy(settings.url.clone(), db)?;
    Ok(GlobalProxyConfig {
        configured: !settings.url.trim().is_empty(),
        url: settings.url,
    })
}

#[tauri::command]
pub fn get_global_proxy_config(db: State<'_, DbState>) -> Result<GlobalProxyConfig, String> {
    get_settings(db)
}

#[tauri::command]
pub fn update_global_proxy_config(
    settings: GlobalProxyConfig,
    db: State<'_, DbState>,
) -> Result<GlobalProxyConfig, String> {
    save_settings(settings, db)
}

#[tauri::command]
pub fn update_proxy_config(
    settings: LocalProviderProxySettings,
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    set_local_provider_proxy_settings(settings, app_handle, db)
}

#[tauri::command]
pub fn get_proxy_config_for_app(
    _app_id: String,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxySettings, String> {
    get_local_provider_proxy_settings(db)
}

#[tauri::command]
pub fn update_proxy_config_for_app(
    app_id: String,
    enabled: bool,
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    let mut settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_local_provider_proxy_settings_from_conn(&conn)
    };
    settings.enabled_apps.retain(|value| value != &app_id);
    if enabled {
        settings.enabled_apps.push(app_id);
    }
    set_local_provider_proxy_settings(settings, app_handle, db)
}

#[tauri::command]
pub fn switch_provider(id: String, db: State<'_, DbState>) -> Result<serde_json::Value, String> {
    apply_profile(id, db)
}

#[tauri::command]
pub fn switch_proxy_provider(
    id: String,
    db: State<'_, DbState>,
) -> Result<serde_json::Value, String> {
    apply_profile(id, db)
}

#[tauri::command]
pub fn update_tray_menu(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::tray::refresh_menu(&app_handle).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn check_env_conflicts(
) -> Result<Vec<crate::commands::extra_commands::EnvironmentConflict>, String> {
    get_environment_conflicts()
}

#[tauri::command]
pub fn get_config_status(
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::extra_commands::LocalAuthStatus>, String> {
    get_local_auth_status(db)
}

#[tauri::command]
pub fn get_tool_versions(
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::extra_commands::ToolEnvironmentReport>, String> {
    get_tool_environment_report(db)
}

#[tauri::command]
pub fn get_request_logs(
    limit: Option<u32>,
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::usage_commands::ProxyRequestLogRow>, String> {
    get_recent_proxy_request_logs(limit, db)
}

#[tauri::command]
pub async fn stream_check_provider(
    id: String,
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<crate::commands::extra_commands::ProviderStreamCheckResult, String> {
    stream_check_config_profile(id, app_handle, db).await
}

#[tauri::command]
pub async fn stream_check_all_providers(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::extra_commands::ProviderStreamCheckResult>, String> {
    stream_check_all_config_profiles(app_handle, db).await
}

#[tauri::command]
pub fn get_proxy_status(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    get_local_provider_proxy_status(app_handle, db)
}

#[tauri::command]
pub fn get_upstream_proxy_status(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    get_local_provider_proxy_status(app_handle, db)
}

#[tauri::command]
pub fn is_proxy_running(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    Ok(get_local_provider_proxy_status(app_handle, db)?.running)
}

#[tauri::command]
pub fn start_proxy_server(
    apps: Option<Vec<String>>,
    port: Option<u16>,
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    let mut settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_local_provider_proxy_settings_from_conn(&conn)
    };
    if let Some(port) = port.filter(|value| *value > 0) {
        settings.port = port;
    }
    settings.enabled_apps = apps.unwrap_or_else(|| {
        if settings.enabled_apps.is_empty() {
            vec![
                "claude", "codex", "gemini", "opencode", "openclaw", "hermes",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        } else {
            settings.enabled_apps.clone()
        }
    });
    set_local_provider_proxy_settings(settings, app_handle, db)
}

#[tauri::command]
pub fn stop_proxy_server(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    let mut settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_local_provider_proxy_settings_from_conn(&conn)
    };
    settings.enabled_apps.clear();
    set_local_provider_proxy_settings(settings, app_handle, db)
}

#[tauri::command]
pub fn get_current_provider(
    tool_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<Option<ConfigProfile>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let active_ids = get_active_config_profile_ids_from_conn(&conn)?;
    let profiles = read_all_config_profiles_from_conn(&conn)?;
    Ok(active_ids.into_iter().find_map(|id| {
        profiles
            .iter()
            .find(|profile| {
                profile.id == id
                    && tool_id
                        .as_deref()
                        .map(|tool| profile.tool_id == tool)
                        .unwrap_or(true)
            })
            .cloned()
    }))
}

#[tauri::command]
pub fn list_sessions(
    tool_id: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::extra_commands::SessionSummary>, String> {
    crate::commands::extra_commands::get_sessions(tool_id, query, limit, db)
}

#[tauri::command]
pub fn get_model_pricing(
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::usage_commands::ModelPricingRow>, String> {
    crate::commands::usage_commands::list_model_pricing(db)
}

#[tauri::command]
pub fn get_provider_limits(
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::provider_health_commands::ProviderStatsItem>, String> {
    crate::commands::provider_health_commands::get_provider_stats(db)
}

/// Compatibility entry point for clients that request limits for one provider/app pair.
/// The local request log is the source of truth, so the returned rows are filtered when
/// either selector is supplied and otherwise expose the complete aggregate.
#[tauri::command]
pub fn check_provider_limits(
    provider_id: Option<String>,
    app_type: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::provider_health_commands::ProviderStatsItem>, String> {
    let rows = get_provider_limits(db)?;
    let provider = provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let app = app_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if provider.is_none() && app.is_none() {
        return Ok(rows);
    }
    Ok(rows
        .into_iter()
        .filter(|row| {
            provider.is_none_or(|value| row.provider_name == value)
                && app.is_none_or(|value| row.tool_id == value)
        })
        .collect())
}

#[tauri::command]
pub fn get_claude_config_status(
    db: State<'_, DbState>,
) -> Result<Option<crate::commands::extra_commands::LocalAuthStatus>, String> {
    Ok(crate::commands::extra_commands::get_local_auth_status(db)?
        .into_iter()
        .find(|status| status.tool_id == "claude"))
}

#[tauri::command]
pub fn get_config_dir(app: String, db: State<'_, DbState>) -> Result<String, String> {
    if app.trim().eq_ignore_ascii_case("claude-desktop") {
        let path = crate::mcp::config::claude_desktop_config_path()
            .ok_or("Claude Desktop path is unavailable")?;
        return path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .ok_or_else(|| "Invalid Claude Desktop path".to_string());
    }
    let tool_id = normalize_tool_id(&app)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    crate::commands::extra_commands::resolve_tool_config_dir(&conn, &tool_id)
        .map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn get_claude_code_config_path(db: State<'_, DbState>) -> Result<String, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    crate::commands::extra_commands::resolve_tool_config_path(&conn, "claude")
        .map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn get_app_config_path() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(home
        .join(".cchub")
        .join("config.json")
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub fn open_config_folder(app: String, db: State<'_, DbState>) -> Result<bool, String> {
    let path = get_config_dir(app, db)?;
    crate::commands::extra_commands::open_in_system(path)?;
    Ok(true)
}

#[tauri::command]
pub fn open_app_config_folder() -> Result<bool, String> {
    let path = get_app_config_path()?;
    let directory = std::path::Path::new(&path)
        .parent()
        .ok_or("Invalid app config path")?
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    crate::commands::extra_commands::open_in_system(directory)?;
    Ok(true)
}

#[tauri::command]
pub fn list_db_backups(
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::extra_commands::ManagedBackupFile>, String> {
    crate::commands::extra_commands::list_managed_backups(db)
}

#[tauri::command]
pub fn create_db_backup(db: State<'_, DbState>) -> Result<String, String> {
    crate::commands::extra_commands::create_managed_backup(Some("manual".to_string()), db)
}

#[tauri::command]
pub fn delete_db_backup(filename: String, db: State<'_, DbState>) -> Result<(), String> {
    let path = normalize_backup_path(&filename)?;
    crate::commands::extra_commands::delete_managed_backup(path, db)
}

#[tauri::command]
pub fn restore_db_backup(filename: String, db: State<'_, DbState>) -> Result<String, String> {
    let path = normalize_backup_path(&filename)?;
    crate::commands::extra_commands::restore_managed_backup(path, db)
}

#[tauri::command]
pub fn list_daily_memory_files(
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::extra_commands::OpenClawDailyMemoryEntry>, String> {
    crate::commands::extra_commands::search_openclaw_daily_memory(None, None, db)
}

#[tauri::command]
pub fn search_daily_memory_files(
    query: Option<String>,
    limit: Option<usize>,
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::extra_commands::OpenClawDailyMemoryEntry>, String> {
    crate::commands::extra_commands::search_openclaw_daily_memory(query, limit, db)
}

#[tauri::command]
pub fn read_daily_memory_file(path: String, db: State<'_, DbState>) -> Result<String, String> {
    crate::commands::extra_commands::read_openclaw_daily_memory_content(path, db)
}

#[tauri::command]
pub fn write_daily_memory_file(
    path: String,
    content: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let path_buf = std::path::PathBuf::from(&path);
    if !crate::commands::extra_commands::is_valid_openclaw_daily_memory_path(&path_buf, &conn) {
        return Err("Invalid daily memory path".to_string());
    }
    crate::utils::atomic_write_string(&path_buf, &content).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_daily_memory_file(path: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let path_buf = std::path::PathBuf::from(&path);
    if !crate::commands::extra_commands::is_valid_openclaw_daily_memory_path(&path_buf, &conn) {
        return Err("Invalid daily memory path".to_string());
    }
    std::fs::remove_file(path_buf).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_hermes_live_provider_ids(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    Ok(crate::commands::hermes_commands::list_hermes_providers(db)?
        .into_iter()
        .map(|provider| provider.name)
        .collect())
}

#[tauri::command]
pub fn get_hermes_live_provider(
    provider_id: String,
    db: State<'_, DbState>,
) -> Result<crate::commands::hermes_commands::HermesProvider, String> {
    crate::commands::hermes_commands::get_hermes_provider(db, provider_id)
}

#[tauri::command]
pub fn get_hermes_model_config(
    db: State<'_, DbState>,
) -> Result<Option<serde_yaml::Value>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let config = crate::hermes::config::read_value(&conn)?;
    Ok(config.get("model").cloned())
}

#[tauri::command]
pub fn get_hermes_memory(kind: String, db: State<'_, DbState>) -> Result<String, String> {
    crate::commands::hermes_commands::get_hermes_memory_content(db, kind)
}

#[tauri::command]
pub fn set_hermes_memory(
    kind: String,
    content: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    crate::commands::hermes_commands::save_hermes_memory_content(db, kind, content)
}

#[tauri::command]
pub fn set_hermes_memory_enabled(
    kind: String,
    enabled: bool,
    db: State<'_, DbState>,
) -> Result<(), String> {
    crate::commands::hermes_commands::toggle_hermes_memory_enabled(db, kind, enabled)
}

#[tauri::command]
pub fn get_claude_common_config_snippet(
    db: State<'_, DbState>,
) -> Result<crate::commands::extra_commands::CommonConfigSnippet, String> {
    crate::commands::extra_commands::get_common_config_snippet("claude".to_string(), db)
}

#[tauri::command]
pub fn set_claude_common_config_snippet(
    snippet: crate::commands::extra_commands::CommonConfigSnippet,
    db: State<'_, DbState>,
) -> Result<crate::commands::extra_commands::CommonConfigSnippet, String> {
    crate::commands::extra_commands::set_common_config_snippet("claude".to_string(), snippet, db)
}

#[tauri::command]
pub fn get_claude_plugin_status() -> Result<crate::commands::extra_commands::Hello2ccStatus, String>
{
    crate::commands::extra_commands::get_hello2cc_status()
}

#[tauri::command]
pub fn is_claude_plugin_applied() -> Result<bool, String> {
    Ok(crate::commands::extra_commands::get_hello2cc_status()?.enabled)
}

#[tauri::command]
pub fn read_claude_plugin_config() -> Result<crate::commands::extra_commands::Hello2ccConfig, String>
{
    crate::commands::extra_commands::get_hello2cc_config()
}

#[tauri::command]
pub fn apply_claude_plugin_config(
    config: crate::commands::extra_commands::Hello2ccConfig,
) -> Result<crate::commands::extra_commands::Hello2ccStatus, String> {
    crate::commands::extra_commands::set_hello2cc_config(config)
}

#[tauri::command]
pub fn apply_claude_onboarding_skip(db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('claude_onboarding_skipped', 'true')",
        [],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clear_claude_onboarding_skip(db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM app_settings WHERE key = 'claude_onboarding_skipped'",
        [],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_proxy_takeover_status(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    get_local_provider_proxy_status(app_handle, db)
}

#[tauri::command]
pub fn is_live_takeover_active(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    Ok(get_local_provider_proxy_status(app_handle, db)?.running)
}

#[tauri::command]
pub fn stop_proxy_with_restore(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    stop_proxy_server(app_handle, db)
}

#[tauri::command]
pub async fn pick_directory() -> Result<Option<String>, String> {
    crate::commands::extra_commands::pick_folder().await
}

#[tauri::command]
pub async fn open_file_dialog() -> Result<Option<String>, String> {
    crate::commands::extra_commands::pick_file().await
}

#[tauri::command]
pub async fn open_zip_file_dialog() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Select ZIP archive")
        .add_filter("ZIP", &["zip"])
        .pick_file()
        .await;
    Ok(file.map(|item| item.path().to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn save_file_dialog() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Save file")
        .save_file()
        .await;
    Ok(file.map(|item| item.path().to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url.trim()).map_err(|error| error.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only HTTP(S) URLs can be opened externally".to_string());
    }
    crate::commands::extra_commands::open_in_system(parsed.to_string())
}

#[tauri::command]
pub fn ensure_codex_official_provider(db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    crate::commands::extra_commands::ensure_official_config_profiles_seeded(&conn)
}

#[tauri::command]
pub fn clear_current_profile(tool_id: String, db: State<'_, DbState>) -> Result<(), String> {
    let tool = tool_id.trim();
    if tool.is_empty() {
        return Err("Tool id is required".to_string());
    }
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM app_settings WHERE key = ?1",
        rusqlite::params![crate::commands::extra_commands::current_profile_setting_key(tool)],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_mcp_server(id: String, db: State<'_, DbState>) -> Result<(), String> {
    crate::commands::mcp_commands::uninstall_mcp_server(id, db)
}

#[tauri::command]
pub fn delete_mcp_server_in_config(
    _app: String,
    id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    crate::commands::mcp_commands::uninstall_mcp_server(id, db)
}

#[tauri::command]
pub fn upsert_mcp_server(
    id: String,
    spec: serde_json::Value,
    db: State<'_, DbState>,
) -> Result<crate::db::models::McpServer, String> {
    let config: crate::mcp::config::McpServerConfig = serde_json::from_value(spec)
        .map_err(|error| format!("Invalid MCP server config: {error}"))?;
    crate::commands::mcp_commands::install_mcp_server(
        id,
        config.command,
        config.args,
        config.env,
        db,
    )
}

#[tauri::command]
pub fn upsert_mcp_server_in_config(
    _app: String,
    id: String,
    spec: serde_json::Value,
    _sync_other_side: Option<bool>,
    db: State<'_, DbState>,
) -> Result<crate::db::models::McpServer, String> {
    upsert_mcp_server(id, spec, db)
}

#[tauri::command]
pub fn toggle_mcp_app(
    server_id: String,
    app: String,
    enabled: bool,
    db: State<'_, DbState>,
) -> Result<(), String> {
    if enabled {
        crate::commands::mcp_commands::sync_mcp_server_to_tool(server_id, app, db)
    } else {
        crate::commands::mcp_commands::unsync_mcp_server_from_tool(server_id, app)
    }
}

#[tauri::command]
pub fn scan_openclaw_config_health(
) -> Result<Vec<crate::openclaw_config::OpenClawHealthWarning>, String> {
    crate::openclaw_config::scan_health()
}

#[tauri::command]
pub fn uninstall_skill_for_app(
    _app: String,
    directory: String,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    crate::commands::skill_repository_commands::uninstall_skill(directory, db)
}

#[tauri::command]
pub fn uninstall_skill_unified(
    id: String,
    db: State<'_, DbState>,
) -> Result<serde_json::Value, String> {
    crate::commands::skill_compat::uninstall_skill_unified(id, db)
}

#[tauri::command]
pub fn get_log_config(
    db: State<'_, DbState>,
) -> Result<crate::commands::extra_commands::LogPreferences, String> {
    crate::commands::extra_commands::get_log_preferences(db)
}

#[tauri::command]
pub fn set_log_config(
    preferences: crate::commands::extra_commands::LogPreferences,
    db: State<'_, DbState>,
) -> Result<crate::commands::extra_commands::LogPreferences, String> {
    crate::commands::extra_commands::set_log_preferences(preferences, db)
}

#[tauri::command]
pub fn update_circuit_breaker_config(
    app_handle: tauri::AppHandle,
    config: crate::proxy_optimizer::config::OptimizerConfig,
    db: State<'_, DbState>,
) -> Result<(), String> {
    crate::commands::optimizer_commands::set_optimizer_config(app_handle, db, config)
}

#[tauri::command]
pub fn update_model_pricing(
    entry: crate::commands::usage_commands::ModelPricingInput,
    db: State<'_, DbState>,
) -> Result<crate::commands::usage_commands::ModelPricingRow, String> {
    crate::commands::usage_commands::save_model_pricing(entry, db)
}

#[tauri::command]
pub fn update_model_pricing_batch(
    entries: Vec<crate::commands::usage_commands::ModelPricingInput>,
    db: State<'_, DbState>,
) -> Result<Vec<crate::commands::usage_commands::ModelPricingRow>, String> {
    entries
        .into_iter()
        .map(|entry| crate::commands::usage_commands::save_model_pricing(entry, db.clone()))
        .collect()
}

fn normalize_tool_id(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    let tool_id = match normalized.as_str() {
        "claude" | "claude-code" => "claude",
        "codex" | "gemini" | "opencode" | "openclaw" | "hermes" | "pi" => normalized.as_str(),
        _ => return Err(format!("Unsupported application: {value}")),
    };
    Ok(tool_id.to_string())
}

fn normalize_backup_path(value: &str) -> Result<String, String> {
    let raw = std::path::PathBuf::from(value);
    if raw.components().count() == 1 {
        let dir = crate::commands::extra_commands::ensure_managed_backups_dir()?;
        return Ok(dir.join(raw).to_string_lossy().into_owned());
    }
    Ok(raw.to_string_lossy().into_owned())
}
