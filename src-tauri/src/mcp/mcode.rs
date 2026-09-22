use super::config::{parse_server_entry, McpServerConfig, ScannedMcpServer};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub fn path() -> Result<PathBuf, String> {
    Ok(crate::commands::mcode_commands::config_path()?.with_file_name("mcp.json"))
}

fn read(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({"mcpServers": {}}));
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read MiniMax Code MCP config: {error}"))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|_| "Invalid MiniMax Code MCP JSON".to_string())?;
    if !value.is_object()
        || value
            .get("mcpServers")
            .is_some_and(|servers| !servers.is_object())
    {
        return Err("Invalid MiniMax Code mcpServers mapping".to_string());
    }
    Ok(value)
}

fn scan_at(path: &Path) -> Vec<ScannedMcpServer> {
    let Ok(document) = read(path) else {
        return Vec::new();
    };
    let location = path.to_string_lossy();
    document
        .get("mcpServers")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|servers| servers.iter())
        .filter(|(_, spec)| spec.get("enabled") != Some(&Value::Bool(false)))
        .filter_map(|(name, spec)| parse_server_entry(name, spec, "mcode", &location))
        .collect()
}

pub fn scan() -> Vec<ScannedMcpServer> {
    path().map(|path| scan_at(&path)).unwrap_or_default()
}

fn spec_for(config: &McpServerConfig) -> Value {
    let remote = matches!(
        config.transport_type.as_deref(),
        Some("http" | "sse" | "streamable-http" | "remote")
    ) || config.command.starts_with("https://")
        || config.command.starts_with("http://");
    if remote {
        json!({"type": config.transport_type.as_deref().unwrap_or("http"), "url": config.command, "headers": config.env, "enabled": true})
    } else {
        json!({"type": "stdio", "command": config.command, "args": config.args, "env": config.env, "enabled": true})
    }
}

struct ConfigLock(PathBuf);
impl Drop for ConfigLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

fn sync_at(path: &Path, name: &str, config: Option<&McpServerConfig>) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("MCP server name cannot be empty".to_string());
    }
    if config.is_none() && !path.exists() {
        return Ok(());
    }
    let parent = path.parent().ok_or("Invalid MiniMax Code MCP path")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Cannot create MiniMax Code directory: {error}"))?;
    let lock_path = PathBuf::from(format!("{}.lock", path.display()));
    fs::create_dir(&lock_path).map_err(|_| "MiniMax Code MCP configuration is busy".to_string())?;
    let _lock = ConfigLock(lock_path);
    let mut document = read(path)?;
    let root = document
        .as_object_mut()
        .ok_or("Invalid MiniMax Code MCP document")?;
    let entry = root.entry("mcpServers").or_insert_with(|| json!({}));
    let servers = entry
        .as_object_mut()
        .ok_or("Invalid MiniMax Code mcpServers mapping")?;
    if let Some(config) = config {
        if config.command.trim().is_empty() {
            return Err("MCP server command or URL cannot be empty".to_string());
        }
        let next = spec_for(config);
        let mut existing = servers.get(name).cloned().unwrap_or_else(|| json!({}));
        let fields = existing
            .as_object_mut()
            .ok_or("Invalid MiniMax Code MCP entry")?;
        for key in ["command", "args", "env", "url", "headers", "type"] {
            fields.remove(key);
        }
        fields.extend(next.as_object().expect("MCP spec is an object").clone());
        servers.insert(name.to_string(), existing);
    } else {
        servers.remove(name);
    }
    let text = serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?;
    crate::commands::mcode_commands::write_config(path, &text)
        .map_err(|error| format!("Cannot save MiniMax Code MCP config: {error}"))
}

pub fn sync(name: &str, config: Option<&McpServerConfig>) -> Result<(), String> {
    sync_at(&path()?, name, config)
}

pub fn has_server(name: &str) -> bool {
    path()
        .ok()
        .and_then(|path| read(&path).ok())
        .and_then(|document| {
            document
                .get("mcpServers")
                .and_then(|servers| servers.get(name))
                .cloned()
        })
        .is_some_and(|entry| entry.get("enabled") != Some(&Value::Bool(false)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn preserves_unmanaged_fields_and_remote_transport() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("mcp.json");
        fs::write(&file, r#"{"other":"kept","mcpServers":{"service":{"command":"old","note":"kept"},"disabled":{"command":"no","enabled":false}}}"#).unwrap();
        let config = McpServerConfig {
            command: "https://example.com/mcp".into(),
            args: vec![],
            env: HashMap::from([("Authorization".into(), "Bearer token".into())]),
            transport_type: Some("http".into()),
        };
        sync_at(&file, "service", Some(&config)).unwrap();
        let result = read(&file).unwrap();
        assert_eq!(result["other"], "kept");
        assert_eq!(result["mcpServers"]["service"]["note"], "kept");
        assert!(result["mcpServers"]["service"].get("command").is_none());
        assert_eq!(
            result["mcpServers"]["service"]["url"],
            "https://example.com/mcp"
        );
        assert_eq!(scan_at(&file).len(), 1);
        sync_at(&file, "service", None).unwrap();
        assert!(result["mcpServers"]["disabled"].is_object());
        assert!(read(&file).unwrap()["mcpServers"]["disabled"].is_object());
        assert!(!file.with_extension("json.lock").exists());
    }

    #[test]
    fn rejects_invalid_file_without_replacing_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("mcp.json");
        fs::write(&file, "not JSON").unwrap();
        let config = McpServerConfig {
            command: "tool".into(),
            args: vec![],
            env: HashMap::new(),
            transport_type: None,
        };
        assert!(sync_at(&file, "service", Some(&config)).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "not JSON");
    }
}
