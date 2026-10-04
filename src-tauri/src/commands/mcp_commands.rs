use crate::db::models::McpServer;
use crate::db::DbState;
use crate::mcp::{config::McpServerConfig, health, operations, service};
use std::collections::{BTreeMap, HashMap};
use tauri::State;

mod health_commands;
#[tauri::command]
pub fn check_all_mcp_health(
    db: State<'_, DbState>,
) -> Result<Vec<health::HealthCheckResult>, String> {
    health_commands::check_all_mcp_health(db)
}

#[tauri::command]
pub fn check_mcp_server_health(
    name: String,
    db: State<'_, DbState>,
) -> Result<health::HealthCheckResult, String> {
    health_commands::check_mcp_server_health(name, db)
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfigResponse {
    pub config_path: String,
    pub servers: HashMap<String, serde_json::Value>,
}

#[tauri::command]
pub fn get_mcp_config(app: String, db: State<'_, DbState>) -> Result<McpConfigResponse, String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    let _guard = crate::json_config::write_lock()?;
    let view = crate::mcp::native_read::read_config(&conn, &app)?;
    Ok(McpConfigResponse {
        config_path: view.config_path,
        servers: view.servers,
    })
}

#[tauri::command]
pub(crate) fn scan_mcp_servers(
    db: State<'_, DbState>,
) -> Result<Vec<service::CatalogServer>, String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    operations::refresh(&conn)
}

#[tauri::command]
pub fn import_mcp_from_apps(db: State<'_, DbState>) -> Result<usize, String> {
    Ok(scan_mcp_servers(db)?.len())
}

#[tauri::command]
pub(crate) fn get_mcp_servers(
    db: State<'_, DbState>,
) -> Result<Vec<service::CatalogServer>, String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    service::list(&conn)
}

#[tauri::command]
pub fn install_mcp_server(
    name: String,
    transport: Option<String>,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    targets: Option<Vec<String>>,
    db: State<'_, DbState>,
) -> Result<McpServer, String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    Ok(operations::install(
        &conn,
        name,
        McpServerConfig {
            command,
            args,
            env,
            transport_type: transport,
        },
        targets.unwrap_or_default(),
    )?
    .server)
}

#[tauri::command]
pub fn uninstall_mcp_server(
    name: String,
    revision: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    operations::remove(&conn, &name, revision.as_deref())
}

#[tauri::command]
pub fn update_mcp_server_config(
    name: String,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    revision: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    operations::update(&conn, &name, command, args, env, revision.as_deref())
}

#[tauri::command]
pub fn sync_mcp_server_to_tool(
    server_name: String,
    target_tool: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    operations::toggle(&conn, &server_name, &target_tool, true)
}

#[tauri::command]
pub fn unsync_mcp_server_from_tool(
    server_name: String,
    target_tool: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    operations::toggle(&conn, &server_name, &target_tool, false)
}

/// Compatibility for older clients. Rich states are available to the main UI.
#[tauri::command]
pub fn check_mcp_server_in_tools(
    server_name: String,
    db: State<'_, DbState>,
) -> Result<BTreeMap<String, bool>, String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    Ok(operations::status(&conn, &server_name)?
        .into_iter()
        .map(|(tool, status)| (tool, matches!(status.state.as_str(), "source" | "linked")))
        .collect())
}

#[tauri::command]
pub(crate) fn get_mcp_sync_statuses(
    server_ids: Vec<String>,
    db: State<'_, DbState>,
) -> Result<BTreeMap<String, BTreeMap<String, service::ToolStatus>>, String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    operations::statuses(&conn, &server_ids)
}

#[tauri::command]
pub fn export_mcp_server_config(
    server_id: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    operations::export(&conn, &server_id)
}

#[tauri::command]
pub fn check_runtime_dependencies() -> Vec<health::RuntimeDepStatus> {
    health::check_runtime_deps()
}

#[tauri::command]
pub async fn import_mcp_servers_from_file(db: State<'_, DbState>) -> Result<u32, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Import MCP Servers")
        .add_filter("JSON", &["json", "jsonc"])
        .pick_file()
        .await
        .ok_or("Cancelled")?;
    let content =
        std::fs::read_to_string(file.path()).map_err(|_| "Could not read MCP import file")?;
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    let tool = import_format(&content)?;
    Ok(operations::import_document(&conn, tool, &content, vec![])?.len() as u32)
}

pub(crate) fn import_format(content: &str) -> Result<&'static str, String> {
    let value = crate::json_config::parse_json_object(content)?;
    if value.get("mcp").is_some() && value.get("mcpServers").is_some() {
        return Err(
            "MCP import contains multiple native formats; import one document at a time".into(),
        );
    }
    Ok(if value.get("mcp").is_some() {
        "opencode"
    } else {
        "claude"
    })
}

#[tauri::command]
pub fn read_claude_mcp_config(db: State<'_, DbState>) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    let _guard = crate::json_config::write_lock()?;
    let path = crate::commands::extra_commands::resolve_tool_mcp_path(&conn, "claude")?;
    crate::config_write::read(&path)?
        .map(|bytes| String::from_utf8(bytes).map_err(|_| "MCP file is not UTF-8".into()))
        .transpose()
}

#[tauri::command]
pub fn upsert_claude_mcp_server(
    id: String,
    spec: serde_json::Value,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    super::compat_commands::upsert_mcp_server_in_config("claude".into(), id, spec, None, db)?;
    Ok(true)
}

#[tauri::command]
pub fn delete_claude_mcp_server(id: String, db: State<'_, DbState>) -> Result<bool, String> {
    super::compat_commands::delete_mcp_server_in_config("claude".into(), id, db)?;
    Ok(true)
}

#[tauri::command]
pub async fn validate_mcp_command(cmd: String) -> Result<bool, String> {
    let command = cmd.trim();
    if command.is_empty() {
        return Ok(false);
    }
    let path = std::path::Path::new(command);
    if path.components().count() > 1 {
        return Ok(path.is_file());
    }
    Ok(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).any(|directory| {
            directory.join(command).is_file()
                || (cfg!(windows)
                    && [".exe", ".cmd", ".bat"]
                        .iter()
                        .any(|suffix| directory.join(format!("{command}{suffix}")).is_file()))
        }),
    )
}
