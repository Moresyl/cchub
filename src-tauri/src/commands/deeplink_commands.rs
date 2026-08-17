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

fn provider_snapshot(request: &DeepLinkImportRequest) -> Result<String, String> {
    let app = request
        .app
        .as_deref()
        .ok_or_else(|| "Provider deep link is missing app".to_string())?;
    let endpoint = request.endpoint.clone().unwrap_or_default();
    let api_key = request.api_key.clone().unwrap_or_default();
    let model = request.model.clone().unwrap_or_default();
    let metadata = serde_json::json!({
        "websiteUrl": request.homepage,
        "category": "custom",
        "endpointCandidates": if endpoint.is_empty() { Vec::<String>::new() } else { vec![endpoint.clone()] },
        "iconUrl": request.icon,
    });
    let mut value = match app {
        "claude" => serde_json::json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": api_key,
                "ANTHROPIC_BASE_URL": endpoint,
                "ANTHROPIC_MODEL": model,
                "ANTHROPIC_DEFAULT_HAIKU_MODEL": request.haiku_model,
                "ANTHROPIC_DEFAULT_SONNET_MODEL": request.sonnet_model,
                "ANTHROPIC_DEFAULT_OPUS_MODEL": request.opus_model,
                "ANTHROPIC_API_FORMAT": request.api_format,
            }, "metadata": metadata
        }),
        "codex" => serde_json::json!({
            "auth": {"OPENAI_API_KEY": api_key},
            "config": format!("model = \"{}\"\n\n[model_providers.custom]\nname = \"custom\"\nbase_url = \"{}\"\nwire_api = \"{}\"\nrequires_openai_auth = true\n", if model.is_empty() { "gpt-5.6-sol" } else { &model }, endpoint, request.codex_wire_api.as_deref().unwrap_or("responses")),
            "metadata": metadata
        }),
        "gemini" => serde_json::json!({
            "env": {"GEMINI_API_KEY": api_key, "GEMINI_BASE_URL": endpoint, "GEMINI_MODEL": model},
            "metadata": metadata
        }),
        "openclaw" => serde_json::json!({
            "baseUrl": endpoint, "apiKey": api_key, "api": request.api_protocol.as_deref().unwrap_or("openai-completions"),
            "models": if model.is_empty() { Vec::<Value>::new() } else { vec![serde_json::json!({"id": model, "name": model})] },
            "metadata": metadata
        }),
        "grokbuild" => serde_json::json!({
            "config": format!("[models]\ndefault = \"{}\"\n\n[model.\"{}\"]\nmodel = \"{}\"\nbase_url = \"{}\"\napi_backend = \"responses\"\n{}", if model.is_empty() { "grok-4.5" } else { &model }, if model.is_empty() { "grok-4.5" } else { &model }, if model.is_empty() { "grok-4.5" } else { &model }, endpoint, if api_key.is_empty() { String::new() } else { format!("api_key = \"{}\"\\n", api_key) }),
            "metadata": metadata
        }),
        "opencode" => serde_json::json!({
            "name": "custom", "npm": request.npm.as_deref().unwrap_or("@ai-sdk/openai-compatible"),
            "options": {"baseURL": endpoint, "apiKey": api_key},
            "models": if model.is_empty() { serde_json::json!({}) } else { serde_json::json!({model.clone(): {"name": model}}) },
            "metadata": metadata
        }),
        "hermes" => serde_json::json!({
            "config": {"model": {"provider": request.notes.as_deref().unwrap_or("custom"), "default": model, "base_url": endpoint}},
            "env": {"HERMES_API_KEY": api_key}, "metadata": metadata
        }),
        other => return Err(format!("Unsupported provider app: {other}")),
    };
    if request.usage_script.is_some()
        || request.usage_enabled.is_some()
        || request.usage_api_key.is_some()
        || request.usage_base_url.is_some()
        || request.usage_access_token.is_some()
        || request.usage_user_id.is_some()
        || request.usage_auto_interval.is_some()
    {
        let metadata = value
            .get_mut("metadata")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "Provider snapshot metadata must be an object".to_string())?;
        let code = request
            .usage_script
            .as_deref()
            .map(crate::deeplink::decode_text_payload)
            .transpose()
            .map_err(String::from)?
            .unwrap_or_default();
        metadata.insert(
            "usageScript".to_string(),
            serde_json::json!({
                "enabled": request.usage_enabled.unwrap_or(false),
                "language": "javascript",
                "code": code,
                "apiKey": request.usage_api_key,
                "baseUrl": request.usage_base_url,
                "accessToken": request.usage_access_token,
                "userId": request.usage_user_id,
                "autoQueryInterval": request.usage_auto_interval,
            }),
        );
    }
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

fn import_provider_request(
    request: &DeepLinkImportRequest,
    db: &DbState,
) -> Result<String, String> {
    let app = request.app.as_deref().ok_or("Provider app is required")?;
    let name = request
        .name
        .as_deref()
        .unwrap_or("Imported provider")
        .trim();
    if name.is_empty() {
        return Err("Provider name is required".to_string());
    }
    let snapshot = provider_snapshot(request)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(sort_order), -1) + 1 FROM config_profiles WHERE tool_id = ?3), 'deeplink', NULL, ?5, ?5)",
        rusqlite::params![&id, name, app, snapshot, now],
    )
    .map_err(|error| error.to_string())?;
    Ok(id)
}

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
pub async fn merge_deeplink_config(
    request: DeepLinkImportRequest,
) -> Result<DeepLinkImportRequest, String> {
    merge_request_impl(request).await.map_err(String::from)
}

#[tauri::command]
pub fn import_from_deeplink(
    request: DeepLinkImportRequest,
    db: State<'_, DbState>,
) -> Result<String, String> {
    if request.resource != "provider" {
        return Err("Deep link resource is not a provider".to_string());
    }
    import_provider_request(&request, db.inner())
}

#[tauri::command]
pub async fn import_from_deeplink_unified(
    request: DeepLinkImportRequest,
    db: State<'_, DbState>,
) -> Result<Value, String> {
    let request = merge_request_impl(request).await.map_err(String::from)?;
    match request.resource.as_str() {
        "provider" => Ok(serde_json::json!({
            "type": "provider",
            "id": import_provider_request(&request, db.inner())?
        })),
        "prompt" => {
            let name = request
                .name
                .clone()
                .unwrap_or_else(|| "Imported prompt".to_string());
            let content =
                crate::deeplink::decode_text_payload(request.content.as_deref().unwrap_or(""))
                    .map_err(String::from)?;
            let conn = db.0.lock().map_err(|error| error.to_string())?;
            let preset = crate::claude_md::manager::save_prompt_preset(&conn, None, name, content)?;
            Ok(serde_json::json!({"type": "prompt", "id": preset.id}))
        }
        "mcp" => {
            let result = import_mcp_servers_from_deeplink(request, db)?;
            Ok(serde_json::json!({
                "type": "mcp",
                "importedCount": result.imported_count,
                "importedIds": result.imported_ids,
                "failed": result.failed
            }))
        }
        "skill" => {
            Err("Skill deep links require resolving repository content before import".to_string())
        }
        other => Err(format!("Unsupported deep link resource: {other}")),
    }
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
            "claude" | "claude-desktop" | "codex" | "gemini" | "grokbuild" | "opencode"
            | "hermes" => {
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

    if parsed.get("command").is_some() || parsed.get("url").is_some() {
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
        .or_else(|| value.get("url"))
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
        .or_else(|| value.get("headers"))
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
        .map(ToString::to_string)
        .or_else(|| value.get("url").map(|_| "http".to_string()));

    Ok(McpServerConfig {
        command,
        args,
        env,
        transport_type,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_mcp_server_config, provider_snapshot};
    use crate::deeplink::parse_deeplink_url;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use serde_json::json;

    #[test]
    fn parses_remote_mcp_url_and_headers() {
        let config = parse_mcp_server_config(&json!({
            "url": "https://example.com/mcp",
            "headers": {"Authorization": "Bearer secret"}
        }))
        .expect("remote MCP should parse");
        assert_eq!(config.command, "https://example.com/mcp");
        assert_eq!(config.transport_type.as_deref(), Some("http"));
        assert_eq!(
            config.env.get("Authorization").map(String::as_str),
            Some("Bearer secret")
        );
    }

    #[test]
    fn deep_link_usage_script_is_stored_disabled_by_default() {
        let encoded = STANDARD.encode("return { remaining: 1 };");
        let url = format!(
            "cchub://v1/import?resource=provider&app=claude&name=Usage&endpoint=https%3A%2F%2Fexample.com&usageScript={encoded}"
        );
        let request = parse_deeplink_url(&url).expect("provider deep link should parse");
        let snapshot: serde_json::Value =
            serde_json::from_str(&provider_snapshot(&request).unwrap()).unwrap();
        let usage = snapshot
            .pointer("/metadata/usageScript")
            .expect("usage metadata");
        assert_eq!(
            usage.get("enabled").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            usage.get("code").and_then(|value| value.as_str()),
            Some("return { remaining: 1 };")
        );
    }
}
