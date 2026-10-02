use rusqlite::Connection;
use serde_yaml::Value;
use std::collections::HashMap;

use crate::mcp::config::{McpServerConfig, ScannedMcpServer};

fn with_default_root_conn<T>(
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let conn =
        Connection::open_in_memory().map_err(|_| "Cannot initialize Hermes path settings")?;
    conn.execute_batch("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT); CREATE TABLE custom_paths (tool_id TEXT PRIMARY KEY, config_dir TEXT, mcp_config_path TEXT);")
        .map_err(|_| "Cannot initialize default Hermes path settings")?;
    f(&conn)
}

pub fn scan_servers(conn: &Connection) -> Result<Vec<ScannedMcpServer>, String> {
    // Keep the legacy global catalog's scan semantics until source-complete
    // reconciliation is migrated with identity and mutation routing. Its caller
    // currently swallows errors then deletes missing rows. Strict scoped reads
    // live in native_read; enabling them here alone could erase catalog records.
    let document = super::config::read_value(conn)?;
    let config_path = super::config_path(conn)?.to_string_lossy().into_owned();
    let Some(servers) = document
        .as_mapping()
        .and_then(|root| root.get(yaml_key("mcp_servers")))
        .and_then(Value::as_mapping)
    else {
        return Ok(Vec::new());
    };
    let mut scanned = Vec::new();
    for (name, entry) in servers {
        let (Some(name), Some(entry)) = (name.as_str(), entry.as_mapping()) else {
            continue;
        };
        if let Some(url) = entry.get(yaml_key("url")).and_then(Value::as_str) {
            let transport =
                if entry.get(yaml_key("transport")).and_then(Value::as_str) == Some("sse") {
                    "sse"
                } else {
                    "http"
                };
            scanned.push(ScannedMcpServer {
                name: name.into(),
                command: url.into(),
                args: Vec::new(),
                env: extract_string_map(entry.get(yaml_key("headers"))),
                transport: transport.into(),
                source: "hermes".into(),
                config_path: config_path.clone(),
            });
        } else if let Some(command) = entry.get(yaml_key("command")).and_then(Value::as_str) {
            scanned.push(ScannedMcpServer {
                name: name.into(),
                command: command.into(),
                args: extract_string_array(entry.get(yaml_key("args"))),
                env: extract_string_map(entry.get(yaml_key("env"))),
                transport: "stdio".into(),
                source: "hermes".into(),
                config_path: config_path.clone(),
            });
        }
    }
    Ok(scanned)
}

fn yaml_key(key: &str) -> Value {
    Value::String(key.to_owned())
}

fn extract_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn extract_string_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_mapping)
        .map(|fields| {
            fields
                .iter()
                .filter_map(|(key, value)| {
                    Some((key.as_str()?.to_owned(), value.as_str()?.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn scan_servers_from_default_root() -> Result<Vec<ScannedMcpServer>, String> {
    with_default_root_conn(scan_servers)
}

pub fn write_server(conn: &Connection, name: &str, server: &McpServerConfig) -> Result<(), String> {
    let path = crate::commands::extra_commands::resolve_tool_mcp_path(conn, "hermes")?;
    crate::mcp::native_yaml::update_at(&path, name, Some(server))
}

pub fn write_server_to_default_root(name: &str, server: &McpServerConfig) -> Result<(), String> {
    with_default_root_conn(|conn| write_server(conn, name, server))
}

pub fn remove_server(conn: &Connection, name: &str) -> Result<(), String> {
    let path = crate::commands::extra_commands::resolve_tool_mcp_path(conn, "hermes")?;
    crate::mcp::native_yaml::update_at(&path, name, None)
}

pub fn remove_server_from_default_root(name: &str) -> Result<(), String> {
    with_default_root_conn(|conn| remove_server(conn, name))
}

pub fn has_server(conn: &Connection, name: &str) -> Result<bool, String> {
    let value = super::config::read_value(conn)?;
    Ok(value
        .as_mapping()
        .and_then(|root| root.get(yaml_key("mcp_servers")))
        .and_then(Value::as_mapping)
        .is_some_and(|servers| servers.contains_key(yaml_key(name))))
}

pub fn has_server_in_default_root(name: &str) -> Result<bool, String> {
    with_default_root_conn(|conn| has_server(conn, name))
}
