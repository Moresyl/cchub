use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::db::DbState;
use crate::deeplink::{
    decode_text_payload, merge_deeplink_request as merge_request_impl, parse_deeplink_url,
    DeepLinkErrorPayload, DeepLinkImportRequest, DeepLinkState,
};
#[cfg(test)]
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

fn mcode_provider_from_request(
    request: &DeepLinkImportRequest,
    name: &str,
) -> Result<(String, Value), String> {
    let safe_name = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(48)
        .collect::<String>();
    let prefix = if safe_name.is_empty() {
        "imported"
    } else {
        &safe_name
    };
    let id = format!(
        "{}-{}",
        prefix,
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    let api = match request
        .api_protocol
        .as_deref()
        .or(request.api_format.as_deref())
    {
        None | Some("anthropic" | "anthropic-messages") => "anthropic-messages",
        Some("openai_chat" | "openai-completions") => "openai-completions",
        Some("openai_responses" | "openai-responses") => "openai-responses",
        Some(_) => return Err("Unsupported MiniMax Code API format".to_string()),
    };
    let model = request.model.as_deref().unwrap_or_default().trim();
    let provider = serde_json::json!({
        "kind": "custom",
        "enabled": request.enabled.unwrap_or(true),
        "api": api,
        "options": {"baseURL": request.endpoint.as_deref().unwrap_or_default(), "apiKey": request.api_key.as_deref().unwrap_or_default()},
        "models": if model.is_empty() { serde_json::json!({}) } else { serde_json::json!({model: {"name": model}}) }
    });
    Ok((id, provider))
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
    if app == "mcode" {
        let (id, provider) = mcode_provider_from_request(request, name)?;
        crate::commands::mcode_commands::save_mcode_provider(id.clone(), provider)?;
        return Ok(id);
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
pub fn has_pending_deeplinks(state: State<'_, DeepLinkState>) -> Result<bool, String> {
    state.has_pending().map_err(String::from)
}

#[tauri::command]
pub fn import_mcp_servers_from_deeplink(
    request: DeepLinkImportRequest,
    db: State<'_, DbState>,
) -> Result<DeepLinkMcpImportResult, String> {
    if request.resource != "mcp" {
        return Err("Deep link resource is not MCP".into());
    }
    let apps = parse_target_apps(
        request
            .apps
            .as_deref()
            .ok_or("Missing apps field in MCP deep link")?,
    )?;
    let (format, text) = decode_mcp_document(&request)?;
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    let imported = crate::mcp::operations::import_targets(&conn, format, &text, apps)?;
    let imported_ids: Vec<_> = imported.into_iter().map(|row| row.server.id).collect();
    Ok(DeepLinkMcpImportResult {
        imported_count: imported_ids.len(),
        imported_ids,
        failed: vec![],
    })
}

fn decode_mcp_document(request: &DeepLinkImportRequest) -> Result<(&'static str, String), String> {
    let text = decode_text_payload(
        request
            .config
            .as_deref()
            .ok_or("Missing MCP configuration")?,
    )
    .map_err(String::from)?;
    let value = crate::json_config::parse_json_object(&text)?;
    if value.get("command").is_some()
        || value.get("url").is_some()
        || value.get("httpUrl").is_some()
    {
        let name = request
            .name
            .as_deref()
            .ok_or("Single MCP config requires a name")?;
        let tool = if value.get("command").is_some_and(Value::is_array)
            || value.get("type").and_then(Value::as_str) == Some("remote")
        {
            "opencode"
        } else {
            "claude"
        };
        return Ok((tool, serde_json::json!({name: value}).to_string()));
    }
    Ok((super::mcp_commands::import_format(&text)?, text))
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
            | "hermes" | "mcode" => {
                if !apps.iter().any(|current| current == value) {
                    apps.push(value.to_string());
                }
            }
            other => return Err(format!("Unsupported MCP target app: {other}")),
        }
    }

    if apps.is_empty() {
        return Err("MCP deep link must target at least one supported app".to_string());
    }

    Ok(apps)
}

#[cfg(test)]
fn parse_mcp_server_config(value: &Value) -> Result<McpServerConfig, String> {
    let server = config::parse_server_entry("deeplink", value, "deeplink", "")
        .ok_or_else(|| "MCP server config is disabled or missing a command/URL".to_string())?;
    if !matches!(server.transport.as_str(), "stdio" | "http" | "sse") {
        return Err(format!("Unsupported MCP transport: {}", server.transport));
    }
    Ok(McpServerConfig {
        command: server.command,
        args: server.args,
        env: server.env,
        transport_type: Some(server.transport),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        mcode_provider_from_request, parse_mcp_server_config, parse_target_apps, provider_snapshot,
    };
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
    fn parses_opencode_and_gemini_native_mcp_shapes() {
        let local = parse_mcp_server_config(&json!({
            "type": "local",
            "command": ["node", "server.js"],
            "environment": {"TOKEN": "secret"},
            "enabled": true
        }))
        .unwrap();
        assert_eq!(local.command, "node");
        assert_eq!(local.args, vec!["server.js"]);
        assert_eq!(local.transport_type.as_deref(), Some("stdio"));

        let remote = parse_mcp_server_config(&json!({
            "httpUrl": "https://example.com/mcp",
            "headers": {"X-API-Key": "secret"}
        }))
        .unwrap();
        assert_eq!(remote.command, "https://example.com/mcp");
        assert_eq!(remote.transport_type.as_deref(), Some("http"));
    }

    #[test]
    fn rejects_clients_without_native_mcp_support() {
        assert!(parse_target_apps("openclaw").is_err());
        assert!(parse_target_apps("pi").is_err());
        assert_eq!(parse_target_apps("claude,gemini").unwrap().len(), 2);
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

    #[test]
    fn minimax_deep_link_builds_a_distinct_native_provider() {
        let request = parse_deeplink_url("cchub://v1/import?resource=provider&app=mcode&name=Demo&endpoint=https%3A%2F%2Fexample.com%2Fv1&apiKey=secret&model=model-a&apiProtocol=openai-responses").unwrap();
        let (first_id, provider) = mcode_provider_from_request(&request, "Demo").unwrap();
        let (second_id, _) = mcode_provider_from_request(&request, "Demo").unwrap();
        assert!(first_id.starts_with("Demo-"));
        assert_ne!(first_id, second_id);
        assert_eq!(provider["api"], "openai-responses");
        assert_eq!(provider["models"]["model-a"]["name"], "model-a");
    }
}
