use serde::Serialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

use crate::mcp::config::{claude_desktop_config_path, scan_all_mcp_servers, ScannedMcpServer};

const DESKTOP_FILE_NAME: &str = "claude_desktop_config.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopStatus {
    pub supported: bool,
    pub configured: bool,
    pub valid_json: bool,
    pub config_path: Option<String>,
    pub mcp_server_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopImportResult {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub config_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMcpStatus {
    pub configured: bool,
    pub server_count: usize,
    pub config_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopRoute {
    pub id: String,
    pub label: String,
    pub config_path: String,
    pub server_count: usize,
}

#[tauri::command]
pub fn get_claude_desktop_status() -> Result<ClaudeDesktopStatus, String> {
    let path = claude_desktop_config_path();
    let Some(path) = path else {
        return Ok(ClaudeDesktopStatus {
            supported: false,
            configured: false,
            valid_json: false,
            config_path: None,
            mcp_server_count: 0,
        });
    };
    if !path.exists() {
        return Ok(status_for_value(&path, false, Value::Null));
    }
    let content = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let value = serde_json::from_str(&content).unwrap_or(Value::Null);
    Ok(status_for_value(&path, true, value))
}

#[tauri::command]
pub fn get_claude_mcp_status() -> Result<ClaudeMcpStatus, String> {
    let servers = scan_all_mcp_servers()
        .into_iter()
        .filter(|server| server.source == "local")
        .collect::<Vec<_>>();
    let mut config_paths = servers
        .iter()
        .map(|server| server.config_path.clone())
        .collect::<Vec<_>>();
    config_paths.sort();
    config_paths.dedup();
    Ok(ClaudeMcpStatus {
        configured: !servers.is_empty(),
        server_count: servers.len(),
        config_paths,
    })
}

#[tauri::command]
pub fn get_claude_desktop_default_routes() -> Result<Vec<ClaudeDesktopRoute>, String> {
    let path = claude_desktop_config_path().ok_or("Claude Desktop path is unavailable")?;
    let server_count = read_json_value(&path)
        .ok()
        .map(|value| server_count(&value))
        .unwrap_or(0);
    Ok(vec![ClaudeDesktopRoute {
        id: "claude-desktop-mcp".to_string(),
        label: "Claude Desktop MCP".to_string(),
        config_path: path.to_string_lossy().into_owned(),
        server_count,
    }])
}

#[tauri::command]
pub fn ensure_claude_desktop_official_provider() -> Result<ClaudeDesktopStatus, String> {
    let path = claude_desktop_config_path().ok_or("Claude Desktop path is unavailable")?;
    let mut value = if path.exists() {
        read_json_value(&path)?
    } else {
        Value::Object(Map::new())
    };
    let object = value
        .as_object_mut()
        .ok_or("Claude Desktop configuration must be a JSON object")?;
    if !object.contains_key("mcpServers") {
        object.insert("mcpServers".to_string(), Value::Object(Map::new()));
        write_json_atomic(&path, &value)?;
    }
    Ok(status_for_value(&path, true, value))
}

#[tauri::command]
pub fn import_claude_desktop_providers_from_claude() -> Result<ClaudeDesktopImportResult, String> {
    let path = claude_desktop_config_path().ok_or("Claude Desktop path is unavailable")?;
    let mut value = if path.exists() {
        read_json_value(&path)?
    } else {
        Value::Object(Map::new())
    };
    let object = value
        .as_object_mut()
        .ok_or("Claude Desktop configuration must be a JSON object")?;
    let servers = object
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("Claude Desktop mcpServers must be a JSON object")?;
    let mut imported = 0;
    let mut updated = 0;
    let mut skipped = 0;
    for server in scan_all_mcp_servers()
        .into_iter()
        .filter(|server| server.source == "local")
    {
        let Some(name) = valid_server_name(&server.name) else {
            skipped += 1;
            continue;
        };
        let next = server_to_value(&server);
        match servers.insert(name, next) {
            Some(previous) if previous != server_to_value(&server) => updated += 1,
            Some(_) => skipped += 1,
            None => imported += 1,
        }
    }
    if imported > 0 || updated > 0 {
        write_json_atomic(&path, &value)?;
    }
    Ok(ClaudeDesktopImportResult {
        imported,
        updated,
        skipped,
        config_path: path.to_string_lossy().into_owned(),
    })
}

fn status_for_value(path: &Path, configured: bool, value: Value) -> ClaudeDesktopStatus {
    ClaudeDesktopStatus {
        supported: true,
        configured,
        valid_json: !value.is_null(),
        config_path: Some(path.to_string_lossy().into_owned()),
        mcp_server_count: server_count(&value),
    }
}

fn server_count(value: &Value) -> usize {
    value
        .get("mcpServers")
        .and_then(Value::as_object)
        .map(Map::len)
        .unwrap_or(0)
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| format!("Invalid {DESKTOP_FILE_NAME}: {error}"))
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid Claude Desktop path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let content = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, content).map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| error.to_string())
}

fn valid_server_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    (!trimmed.is_empty() && trimmed.len() <= 128).then(|| trimmed.to_string())
}

fn server_to_value(server: &ScannedMcpServer) -> Value {
    let mut value = serde_json::json!({
        "command": server.command,
        "args": server.args,
        "env": server.env,
    });
    if let Some(transport) = &server.transport.strip_prefix("type:") {
        value["type"] = Value::String((*transport).trim().to_string());
    }
    value
}

#[cfg(test)]
mod tests {
    use super::{server_count, valid_server_name};
    use serde_json::json;

    #[test]
    fn counts_only_object_mcp_servers() {
        assert_eq!(server_count(&json!({"mcpServers": {"one": {}}})), 1);
        assert_eq!(server_count(&json!({"mcpServers": []})), 0);
    }

    #[test]
    fn rejects_blank_or_oversized_server_names() {
        assert!(valid_server_name("  ").is_none());
        assert!(valid_server_name(&"x".repeat(129)).is_none());
        assert_eq!(valid_server_name("files").as_deref(), Some("files"));
    }
}
