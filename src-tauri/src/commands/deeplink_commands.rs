use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::db::{record_activity, DbState};
use crate::deeplink::{
    decode_text_payload, merge_deeplink_request as merge_request_impl, parse_deeplink_url,
    DeepLinkErrorPayload, DeepLinkImportRequest, DeepLinkState,
};
use crate::mcp::config::{self, McpServerConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkImportFailure {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkMcpImportResult {
    pub imported_count: usize,
    pub imported_ids: Vec<String>,
    pub failed: Vec<DeepLinkImportFailure>,
}

#[tauri::command]
pub fn parse_deeplink(url: String) -> Result<DeepLinkImportRequest, String> {
    parse_deeplink_url(&url).map_err(String::from)
}

#[tauri::command]
pub async fn merge_deeplink_request(
    request: DeepLinkImportRequest,
) -> Result<DeepLinkImportRequest, String> {
    merge_request_impl(request).await.map_err(String::from)
}

#[tauri::command]
pub fn take_pending_deeplink_imports(
    state: State<'_, DeepLinkState>,
) -> Result<Vec<DeepLinkImportRequest>, String> {
    state.take_imports().map_err(String::from)
}

#[tauri::command]
pub fn take_pending_deeplink_errors(
    state: State<'_, DeepLinkState>,
) -> Result<Vec<DeepLinkErrorPayload>, String> {
    state.take_errors().map_err(String::from)
}

#[tauri::command]
pub fn import_mcp_servers_from_deeplink(
    request: DeepLinkImportRequest,
    db: State<'_, DbState>,
) -> Result<DeepLinkMcpImportResult, String> {
    if request.resource != "mcp" {
        return Err("Deep link resource is not MCP".to_string());
    }

    let apps = parse_target_apps(
        request
            .apps
            .as_deref()
            .ok_or_else(|| "Missing apps field in MCP deep link".to_string())?,
    )?;
    let servers = parse_mcp_servers(&request)?;

    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let mut imported_ids = Vec::new();
    let mut failed = Vec::new();

    for (server_name, server_config) in servers {
        let mut sync_error: Option<String> = None;
        for app in &apps {
            if let Err(error) = config::sync_mcp_to_tool(&server_name, &server_config, app) {
                sync_error = Some(match sync_error {
                    Some(current) => format!("{current}; {app}: {error}"),
                    None => format!("{app}: {error}"),
                });
            }
        }

        if let Some(error) = sync_error {
            failed.push(DeepLinkImportFailure {
                id: server_name,
                error,
            });
            continue;
        }

        let args_json =
            serde_json::to_string(&server_config.args).unwrap_or_else(|_| "[]".to_string());
        let env_json =
            serde_json::to_string(&server_config.env).unwrap_or_else(|_| "{}".to_string());
        let transport = server_config
            .transport_type
            .clone()
            .unwrap_or_else(|| "stdio".to_string());

        conn.execute(
            "INSERT OR REPLACE INTO mcp_servers (id, name, command, args, env, transport, source, status, installed_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'deeplink', 'active', COALESCE((SELECT installed_at FROM mcp_servers WHERE id = ?1), ?7), ?7)",
            rusqlite::params![
                &server_name,
                &server_name,
                &server_config.command,
                &args_json,
                &env_json,
                &transport,
                &now,
            ],
        )
        .map_err(|error| error.to_string())?;

        record_activity(&conn, &server_name, "deeplink_import", "success", None);
        imported_ids.push(server_name);
    }

    Ok(DeepLinkMcpImportResult {
        imported_count: imported_ids.len(),
        imported_ids,
        failed,
    })
}

fn parse_target_apps(raw: &str) -> Result<Vec<String>, String> {
    let mut apps = Vec::new();
    for value in raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        match value {
            "claude" | "codex" | "gemini" | "opencode" => {
                if !apps.iter().any(|current| current == value) {
                    apps.push(value.to_string());
                }
            }
            "openclaw" => {}
            other => return Err(format!("Unsupported MCP target app: {other}")),
        }
    }

    if apps.is_empty() {
        return Err("MCP deep link must target at least one supported app".to_string());
    }

    Ok(apps)
}

fn parse_mcp_servers(
    request: &DeepLinkImportRequest,
) -> Result<Vec<(String, McpServerConfig)>, String> {
    let config_value = request
        .config
        .as_deref()
        .ok_or_else(|| "Missing config field in MCP deep link".to_string())?;
    let config_text = decode_text_payload(config_value).map_err(String::from)?;
    let parsed: Value =
        serde_json::from_str(&config_text).map_err(|error| format!("Invalid MCP JSON: {error}"))?;

    let mut servers = Vec::new();
    if let Some(object) = parsed.get("mcpServers").and_then(Value::as_object) {
        for (name, value) in object {
            servers.push((name.clone(), parse_mcp_server_config(value)?));
        }
        return Ok(servers);
    }

    if parsed.get("command").is_some() {
        let name = request
            .name
            .clone()
            .ok_or_else(|| "Single MCP config deep link requires a name field".to_string())?;
        servers.push((name, parse_mcp_server_config(&parsed)?));
        return Ok(servers);
    }

    if let Some(object) = parsed.as_object() {
        for (name, value) in object {
            if value.get("command").is_none() {
                continue;
            }
            servers.push((name.clone(), parse_mcp_server_config(value)?));
        }
    }

    if servers.is_empty() {
        return Err("No MCP servers found in deep link config".to_string());
    }

    Ok(servers)
}

fn parse_mcp_server_config(value: &Value) -> Result<McpServerConfig, String> {
    let command = value
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "MCP server config missing command".to_string())?
        .to_string();
    let args = value
        .get("args")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let env = value
        .get("env")
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|text| (key.clone(), text.to_string()))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let transport_type = value
        .get("type")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    Ok(McpServerConfig {
        command,
        args,
        env,
        transport_type,
    })
}
