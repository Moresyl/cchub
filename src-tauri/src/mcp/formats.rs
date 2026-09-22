use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

use super::config::{McpServerConfig, ScannedMcpServer};

#[derive(Clone, Copy)]
pub enum JsonMcpFormat {
    Standard,
    Gemini,
    OpenCode,
}

impl JsonMcpFormat {
    fn container_key(self) -> &'static str {
        match self {
            Self::Standard => "mcpServers",
            Self::Gemini => "mcpServers",
            Self::OpenCode => "mcp",
        }
    }
}

pub fn is_remote(config: &McpServerConfig) -> bool {
    matches!(
        config.transport_type.as_deref(),
        Some("http" | "sse" | "streamable-http" | "remote")
    ) || config.command.starts_with("http://")
        || config.command.starts_with("https://")
}

fn string_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

pub fn parse_json_server_entry(
    name: &str,
    config: &Value,
    source: &str,
    config_path: &str,
) -> Option<ScannedMcpServer> {
    if config.get("enabled").and_then(Value::as_bool) == Some(false) {
        return None;
    }

    let http_url = config.get("httpUrl").and_then(Value::as_str);
    let url = config.get("url").and_then(Value::as_str).or(http_url);
    let command_array = config.get("command").and_then(Value::as_array);
    let command = config
        .get("command")
        .and_then(Value::as_str)
        .or_else(|| command_array?.first()?.as_str())
        .or(url)?
        .to_string();
    let args = config
        .get("args")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .or_else(|| {
            command_array.map(|items| {
                items
                    .iter()
                    .skip(1)
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
        })
        .unwrap_or_default();
    let mut env = string_map(config.get("env").or_else(|| config.get("environment")));
    for key in ["headers", "http_headers"] {
        env.extend(string_map(config.get(key)));
    }
    let raw_transport = config.get("type").and_then(Value::as_str);
    let transport = match raw_transport {
        Some("local" | "stdio") => "stdio",
        Some("sse") => "sse",
        Some("remote" | "streamable-http" | "http") => "http",
        Some(other) => other,
        None if http_url.is_some() => "http",
        None if url.is_some() && source == "gemini" => "sse",
        None if url.is_some() => "http",
        None => "stdio",
    };

    Some(ScannedMcpServer {
        name: name.to_string(),
        command,
        args,
        env,
        transport: transport.to_string(),
        source: source.to_string(),
        config_path: config_path.to_string(),
    })
}

fn standard_spec(config: &McpServerConfig) -> Value {
    if is_remote(config) {
        let transport = match config.transport_type.as_deref() {
            Some("sse") => "sse",
            _ => "http",
        };
        json!({
            "type": transport,
            "url": config.command,
            "headers": config.env,
        })
    } else {
        json!({
            "type": "stdio",
            "command": config.command,
            "args": config.args,
            "env": config.env,
        })
    }
}

fn opencode_spec(config: &McpServerConfig) -> Value {
    if is_remote(config) {
        json!({
            "type": "remote",
            "url": config.command,
            "headers": config.env,
            "enabled": true,
        })
    } else {
        let mut command = vec![config.command.clone()];
        command.extend(config.args.iter().cloned());
        json!({
            "type": "local",
            "command": command,
            "environment": config.env,
            "enabled": true,
        })
    }
}

fn gemini_spec(config: &McpServerConfig) -> Value {
    if is_remote(config) {
        let mut spec = serde_json::Map::new();
        let url_key = if config.transport_type.as_deref() == Some("sse") {
            "url"
        } else {
            "httpUrl"
        };
        spec.insert(url_key.to_string(), json!(config.command));
        if !config.env.is_empty() {
            spec.insert("headers".to_string(), json!(config.env));
        }
        Value::Object(spec)
    } else {
        json!({
            "command": config.command,
            "args": config.args,
            "env": config.env,
        })
    }
}

pub fn write_json_server(
    path: &Path,
    name: &str,
    config: &McpServerConfig,
    format: JsonMcpFormat,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut document = if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        serde_json::from_str::<Value>(&content)
            .map_err(|error| format!("Invalid MCP JSON at {}: {error}", path.display()))?
    } else {
        json!({})
    };
    let root = document
        .as_object_mut()
        .ok_or_else(|| format!("MCP config must be a JSON object: {}", path.display()))?;
    let container_key = format.container_key();
    let servers = root.entry(container_key).or_insert_with(|| json!({}));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| format!("{container_key} must be a JSON object: {}", path.display()))?;
    let spec = match format {
        JsonMcpFormat::Standard => standard_spec(config),
        JsonMcpFormat::Gemini => gemini_spec(config),
        JsonMcpFormat::OpenCode => opencode_spec(config),
    };
    servers.insert(name.to_string(), spec);
    let content = serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?;
    crate::utils::atomic_write_string(path, &content).map_err(|error| error.to_string())
}

pub fn remove_json_server(path: &Path, name: &str, format: JsonMcpFormat) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut document: Value = serde_json::from_str(&content)
        .map_err(|error| format!("Invalid MCP JSON at {}: {error}", path.display()))?;
    if let Some(servers) = document
        .get_mut(format.container_key())
        .and_then(Value::as_object_mut)
    {
        servers.remove(name);
    }
    let content = serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?;
    crate::utils::atomic_write_string(path, &content).map_err(|error| error.to_string())
}

pub fn has_json_server(path: &Path, name: &str, format: JsonMcpFormat) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|document| {
            document
                .get(format.container_key())
                .and_then(Value::as_object)
                .map(|servers| servers.contains_key(name))
        })
        .unwrap_or(false)
}

pub fn codex_server_table(config: &McpServerConfig) -> toml_edit::Table {
    let mut server = toml_edit::Table::new();
    if is_remote(config) {
        let transport = if config.transport_type.as_deref() == Some("sse") {
            "sse"
        } else {
            "http"
        };
        server["type"] = toml_edit::value(transport);
        server["url"] = toml_edit::value(config.command.as_str());
        if !config.env.is_empty() {
            let mut headers = toml_edit::Table::new();
            for (key, value) in &config.env {
                headers[key.as_str()] = toml_edit::value(value.as_str());
            }
            server["http_headers"] = toml_edit::Item::Table(headers);
        }
    } else {
        server["type"] = toml_edit::value("stdio");
        server["command"] = toml_edit::value(config.command.as_str());
        let mut args = toml_edit::Array::new();
        for arg in &config.args {
            args.push(arg.as_str());
        }
        server["args"] = toml_edit::value(args);
        if !config.env.is_empty() {
            let mut env = toml_edit::Table::new();
            for (key, value) in &config.env {
                env[key.as_str()] = toml_edit::value(value.as_str());
            }
            server["env"] = toml_edit::Item::Table(env);
        }
    }
    server
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_config() -> McpServerConfig {
        McpServerConfig {
            command: "https://example.com/mcp".into(),
            args: Vec::new(),
            env: HashMap::from([("Authorization".into(), "Bearer secret".into())]),
            transport_type: Some("http".into()),
        }
    }

    #[test]
    fn parses_opencode_local_and_filters_disabled_entries() {
        let local = json!({
            "type": "local",
            "command": ["node", "server.js"],
            "environment": {"TOKEN": "secret"},
            "enabled": true,
        });
        let parsed = parse_json_server_entry("local", &local, "opencode", "opencode.json").unwrap();
        assert_eq!(parsed.command, "node");
        assert_eq!(parsed.args, vec!["server.js"]);
        assert_eq!(parsed.transport, "stdio");
        assert_eq!(parsed.env.get("TOKEN").map(String::as_str), Some("secret"));
        assert!(parse_json_server_entry(
            "disabled",
            &json!({"command": "node", "enabled": false}),
            "opencode",
            "opencode.json"
        )
        .is_none());
    }

    #[test]
    fn writes_remote_json_and_opencode_native_formats() {
        let directory = tempfile::tempdir().unwrap();
        let standard = directory.path().join("standard.json");
        std::fs::write(&standard, r#"{"other":"kept"}"#).unwrap();
        write_json_server(
            &standard,
            "remote",
            &remote_config(),
            JsonMcpFormat::Standard,
        )
        .unwrap();
        let standard: Value =
            serde_json::from_str(&std::fs::read_to_string(standard).unwrap()).unwrap();
        assert_eq!(standard["other"], "kept");
        assert_eq!(
            standard["mcpServers"]["remote"]["url"],
            "https://example.com/mcp"
        );
        assert!(standard["mcpServers"]["remote"].get("command").is_none());

        let opencode = directory.path().join("opencode.json");
        write_json_server(
            &opencode,
            "remote",
            &remote_config(),
            JsonMcpFormat::OpenCode,
        )
        .unwrap();
        let opencode: Value =
            serde_json::from_str(&std::fs::read_to_string(opencode).unwrap()).unwrap();
        assert_eq!(opencode["mcp"]["remote"]["type"], "remote");
        assert_eq!(opencode["mcp"]["remote"]["enabled"], true);
        assert!(opencode.get("mcpServers").is_none());
        assert!(has_json_server(
            &directory.path().join("opencode.json"),
            "remote",
            JsonMcpFormat::OpenCode
        ));
        remove_json_server(
            &directory.path().join("opencode.json"),
            "remote",
            JsonMcpFormat::OpenCode,
        )
        .unwrap();
        assert!(!has_json_server(
            &directory.path().join("opencode.json"),
            "remote",
            JsonMcpFormat::OpenCode
        ));
    }

    #[test]
    fn writes_codex_remote_headers_without_command_fields() {
        let table = codex_server_table(&remote_config());
        assert_eq!(
            table.get("type").and_then(toml_edit::Item::as_str),
            Some("http")
        );
        assert_eq!(
            table.get("url").and_then(toml_edit::Item::as_str),
            Some("https://example.com/mcp")
        );
        assert!(table.get("command").is_none());
        assert_eq!(
            table
                .get("http_headers")
                .and_then(toml_edit::Item::as_table)
                .and_then(|headers| headers.get("Authorization"))
                .and_then(toml_edit::Item::as_str),
            Some("Bearer secret")
        );
    }

    #[test]
    fn reads_and_writes_gemini_native_remote_transports() {
        let http = parse_json_server_entry(
            "http",
            &json!({"httpUrl": "https://example.com/mcp", "headers": {"X-API-Key": "secret"}}),
            "gemini",
            "settings.json",
        )
        .unwrap();
        assert_eq!(http.transport, "http");
        assert_eq!(http.command, "https://example.com/mcp");

        let sse = parse_json_server_entry(
            "sse",
            &json!({"url": "https://example.com/events"}),
            "gemini",
            "settings.json",
        )
        .unwrap();
        assert_eq!(sse.transport, "sse");

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        write_json_server(&path, "http", &remote_config(), JsonMcpFormat::Gemini).unwrap();
        let written: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let spec = &written["mcpServers"]["http"];
        assert_eq!(spec["httpUrl"], "https://example.com/mcp");
        assert!(spec.get("url").is_none());
        assert!(spec.get("type").is_none());
    }

    #[test]
    fn deserializes_remote_url_and_headers_into_the_canonical_config() {
        let config: McpServerConfig = serde_json::from_value(json!({
            "type": "sse",
            "url": "https://example.com/events",
            "headers": {"Authorization": "Bearer secret"}
        }))
        .unwrap();
        assert_eq!(config.command, "https://example.com/events");
        assert_eq!(config.transport_type.as_deref(), Some("sse"));
        assert_eq!(
            config.env.get("Authorization").map(String::as_str),
            Some("Bearer secret")
        );
    }
}
