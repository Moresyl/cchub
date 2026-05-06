use rusqlite::Connection;
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;

use crate::mcp::config::{McpServerConfig, ScannedMcpServer};

use super::config;

fn with_default_root_conn<T>(
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
    f(&conn)
}

fn yaml_key(key: &str) -> Value {
    Value::String(key.to_string())
}

fn extract_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn extract_string_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_mapping)
        .map(|mapping| {
            mapping
                .iter()
                .filter_map(|(key, value)| {
                    Some((key.as_str()?.to_string(), value.as_str()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn scan_servers(conn: &Connection) -> Result<Vec<ScannedMcpServer>, String> {
    let config_value = config::read_value(conn)?;
    let config_path = super::config_path(conn)?.to_string_lossy().to_string();
    let Some(root) = config_value.as_mapping() else {
        return Ok(Vec::new());
    };
    let Some(servers) = root
        .get(yaml_key("mcp_servers"))
        .and_then(Value::as_mapping)
    else {
        return Ok(Vec::new());
    };

    let mut scanned = Vec::new();
    for (name_value, entry_value) in servers {
        let Some(name) = name_value.as_str() else {
            continue;
        };
        let Some(entry) = entry_value.as_mapping() else {
            continue;
        };

        if let Some(url) = entry.get(yaml_key("url")).and_then(Value::as_str) {
            scanned.push(ScannedMcpServer {
                name: name.to_string(),
                command: url.to_string(),
                args: Vec::new(),
                env: extract_string_map(entry.get(yaml_key("headers"))),
                transport: "http".to_string(),
                source: "hermes".to_string(),
                config_path: config_path.clone(),
            });
            continue;
        }

        let Some(command) = entry.get(yaml_key("command")).and_then(Value::as_str) else {
            continue;
        };

        scanned.push(ScannedMcpServer {
            name: name.to_string(),
            command: command.to_string(),
            args: extract_string_array(entry.get(yaml_key("args"))),
            env: extract_string_map(entry.get(yaml_key("env"))),
            transport: "stdio".to_string(),
            source: "hermes".to_string(),
            config_path: config_path.clone(),
        });
    }

    Ok(scanned)
}

pub fn scan_servers_from_default_root() -> Result<Vec<ScannedMcpServer>, String> {
    with_default_root_conn(scan_servers)
}

pub fn write_server(conn: &Connection, name: &str, server: &McpServerConfig) -> Result<(), String> {
    let mut config_value = config::read_value(conn)?;
    let root = config::top_level_mapping_mut(&mut config_value);
    let servers_value = root
        .entry(yaml_key("mcp_servers"))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let servers = config::top_level_mapping_mut(servers_value);
    let transport = server.transport_type.as_deref().unwrap_or("stdio");

    let mut entry = Mapping::new();
    if transport == "http" {
        entry.insert(yaml_key("url"), Value::String(server.command.clone()));
        if !server.env.is_empty() {
            let mut headers = Mapping::new();
            for (key, value) in &server.env {
                headers.insert(yaml_key(key), Value::String(value.clone()));
            }
            entry.insert(yaml_key("headers"), Value::Mapping(headers));
        }
        entry.insert(
            yaml_key("timeout"),
            Value::Number(serde_yaml::Number::from(60)),
        );
    } else {
        entry.insert(yaml_key("command"), Value::String(server.command.clone()));
        if !server.args.is_empty() {
            entry.insert(
                yaml_key("args"),
                Value::Sequence(
                    server
                        .args
                        .iter()
                        .map(|item| Value::String(item.clone()))
                        .collect(),
                ),
            );
        }
        if !server.env.is_empty() {
            let mut env = Mapping::new();
            for (key, value) in &server.env {
                env.insert(yaml_key(key), Value::String(value.clone()));
            }
            entry.insert(yaml_key("env"), Value::Mapping(env));
        }
        entry.insert(
            yaml_key("timeout"),
            Value::Number(serde_yaml::Number::from(60)),
        );
    }

    servers.insert(yaml_key(name), Value::Mapping(entry));
    config::write_value(conn, &config_value).map(|_| ())
}

pub fn write_server_to_default_root(name: &str, server: &McpServerConfig) -> Result<(), String> {
    with_default_root_conn(|conn| write_server(conn, name, server))
}

pub fn remove_server(conn: &Connection, name: &str) -> Result<(), String> {
    let mut config_value = config::read_value(conn)?;
    if let Some(servers) = config_value
        .as_mapping_mut()
        .and_then(|root| root.get_mut(yaml_key("mcp_servers")))
        .and_then(Value::as_mapping_mut)
    {
        servers.remove(yaml_key(name));
    }
    config::write_value(conn, &config_value).map(|_| ())
}

pub fn remove_server_from_default_root(name: &str) -> Result<(), String> {
    with_default_root_conn(|conn| remove_server(conn, name))
}

pub fn has_server(conn: &Connection, name: &str) -> Result<bool, String> {
    let config_value = config::read_value(conn)?;
    Ok(config_value
        .as_mapping()
        .and_then(|root| root.get(yaml_key("mcp_servers")))
        .and_then(Value::as_mapping)
        .map(|servers| servers.contains_key(yaml_key(name)))
        .unwrap_or(false))
}

pub fn has_server_in_default_root(name: &str) -> Result<bool, String> {
    with_default_root_conn(|conn| has_server(conn, name))
}
