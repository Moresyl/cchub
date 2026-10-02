use super::config::{parse_server_entry, McpServerConfig, ScannedMcpServer};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn path() -> Result<PathBuf, String> {
    Ok(crate::commands::mcode_commands::config_path()?.with_file_name("mcp.json"))
}

fn read(path: &Path) -> Result<Value, String> {
    let Some(bytes) = crate::config_write::read(path)? else {
        return Ok(json!({"mcpServers": {}}));
    };
    let text = std::str::from_utf8(&bytes).map_err(|_| "MiniMax Code MCP config must use UTF-8")?;
    let value = crate::json_config::parse_json_object(text)?;
    if value
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

fn sync_at(path: &Path, name: &str, config: Option<&McpServerConfig>) -> Result<(), String> {
    super::native_json::update_at(path, name, config, super::formats::JsonMcpFormat::MiniMax)
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
    use std::fs;

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

    #[test]
    fn connection_changes_keep_disabled_policy_comments_and_exact_noop_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("mcp.json");
        let source = "\u{feff}{\r\n // unrelated\r\n \"version\": 7,\r\n \"mcpServers\": {\"service\": {\"type\": \"stdio\", \"command\": \"old\", \"enabled\": false, \"timeout\": 95, \"oauth\": {\"scopes\": [\"read\"]}}, \"other\": {\"command\": \"keep\"}}\r\n}\r\n";
        fs::write(&file, source).unwrap();
        let config = McpServerConfig {
            command: "https://example.com/mcp".into(),
            args: vec![],
            env: HashMap::from([("Authorization".into(), "Bearer token".into())]),
            transport_type: Some("streamable-http".into()),
        };
        sync_at(&file, "service", Some(&config)).unwrap();
        let output = fs::read_to_string(&file).unwrap();
        assert!(output.starts_with('\u{feff}'));
        assert!(output.contains("// unrelated\r\n \"version\": 7"));
        assert!(output.contains("\"other\": {\"command\": \"keep\"}"));
        let value = crate::json_config::parse_json_object(&output).unwrap();
        let entry = &value["mcpServers"]["service"];
        assert_eq!(entry["enabled"], false);
        assert_eq!(entry["type"], "streamable-http");
        assert_eq!(entry["timeout"], 95);
        assert_eq!(entry["oauth"]["scopes"][0], "read");
        assert!(entry.get("command").is_none());
        assert_eq!(entry["headers"]["Authorization"], "Bearer token");
        assert_eq!(read(&file).unwrap(), value);
        let scanned = scan_at(&file);
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].name, "other");
        sync_at(&file, "service", Some(&config)).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), output);
        sync_at(&file, "service", None).unwrap();
        let value =
            crate::json_config::parse_json_object(&fs::read_to_string(&file).unwrap()).unwrap();
        assert!(value["mcpServers"].get("service").is_none());
        assert_eq!(value["mcpServers"]["other"]["command"], "keep");
    }

    #[test]
    fn invalid_inputs_and_missing_removals_create_neither_file_nor_parent() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("missing/mcp.json");
        sync_at(&file, "missing", None).unwrap();
        let config = McpServerConfig {
            command: "file:///private-secret".into(),
            args: vec![],
            env: HashMap::new(),
            transport_type: Some("http".into()),
        };
        for name in ["service", "", "bad\nname"] {
            let error = sync_at(&file, name, Some(&config)).unwrap_err();
            assert!(!error.contains("private-secret"));
            assert!(!file.parent().unwrap().exists());
        }
    }

    #[test]
    fn reads_reject_invalid_file_shapes_without_disclosing_private_values() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("mcp.json");
        assert!(read(&file).unwrap()["mcpServers"]
            .as_object()
            .unwrap()
            .is_empty());
        assert!(!file.exists());
        for bytes in [
            b"{\"private-secret\":".as_slice(),
            b"{\"mcpServers\":[],\"token\":\"private-secret\"}",
            b"{\"mcpServers\":{},\"mcpServers\":{\"secret\":\"private-secret\"}}",
            &[255, 254, 0],
        ] {
            fs::write(&file, bytes).unwrap();
            let error = read(&file).unwrap_err();
            assert!(!error.contains("private-secret"));
            assert!(!error.contains(file.to_str().unwrap()));
            assert_eq!(fs::read(&file).unwrap(), bytes);
        }
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();
        assert!(read(&file).is_err());
        assert!(file.is_dir());
    }
}
