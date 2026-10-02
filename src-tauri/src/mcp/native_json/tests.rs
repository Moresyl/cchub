use super::*;
use serde_json::Value;
use std::collections::HashMap;

fn local() -> McpServerConfig {
    McpServerConfig {
        command: "node".into(),
        args: vec!["server.js".into()],
        env: HashMap::from([("TOKEN".into(), "secret".into())]),
        transport_type: Some("stdio".into()),
    }
}

fn remote() -> McpServerConfig {
    McpServerConfig {
        command: "https://example.com/mcp".into(),
        args: Vec::new(),
        env: HashMap::from([("Authorization".into(), "Bearer secret".into())]),
        transport_type: Some("http".into()),
    }
}

fn edit<'a>(name: &'a str, config: Option<&'a McpServerConfig>, format: JsonMcpFormat) -> Edit<'a> {
    Edit {
        name,
        config,
        format,
    }
}

fn parse(source: &str) -> Value {
    crate::json_config::parse_json_object(source).unwrap()
}

#[test]
fn connection_edits_preserve_disabled_policy_and_extensions_in_each_json_format() {
    let config = remote();
    for (format, key, connection) in [
        (JsonMcpFormat::Standard, "mcpServers", r#""command":"old""#),
        (JsonMcpFormat::Gemini, "mcpServers", r#""command":"old""#),
        (JsonMcpFormat::MiniMax, "mcpServers", r#""command":"old""#),
        (
            JsonMcpFormat::OpenCode,
            "mcp",
            r#""type":"local","command":["old"]"#,
        ),
    ] {
        for enabled in [false, true] {
            let source = format!(
                r#"{{"{key}":{{"service":{{{connection},"enabled":{enabled},"disabled":true,"timeout":99,"oauth":{{"scopes":["read"]}},"tools":{{"allowed":["one"]}}}}}}}}"#
            );
            let output = edit_text(&source, &[edit("service", Some(&config), format)]).unwrap();
            let value = parse(&output);
            let entry = &value[key]["service"];
            assert_eq!(entry["enabled"], enabled);
            assert_eq!(entry["disabled"], true);
            assert_eq!(entry["timeout"], 99);
            assert_eq!(entry["oauth"]["scopes"][0], "read");
            assert_eq!(entry["tools"]["allowed"][0], "one");
            assert!(entry.get("command").is_none());
            assert_eq!(entry["headers"]["Authorization"], "Bearer secret");
            let url_key = if matches!(format, JsonMcpFormat::Gemini) {
                "httpUrl"
            } else {
                "url"
            };
            assert_eq!(entry[url_key], "https://example.com/mcp");
            assert_eq!(
                edit_text(&output, &[edit("service", Some(&config), format)]).unwrap(),
                output
            );
        }
    }
}

#[test]
fn batch_edits_compose_multiple_entries_and_containers_without_writing() {
    let source = "\u{feff}{\r\n  // unrelated settings\r\n  \"theme\": \"dark\",\r\n  \"mcpServers\": {\"remove\": {\"command\": \"old\"}},\r\n  \"mcp\": {\"keep\": {\"type\": \"local\", \"command\": [\"keep\"], \"enabled\": false}},\r\n}\r\n";
    let local = local();
    let remote = remote();
    let output = edit_text(
        source,
        &[
            edit("remove", None, JsonMcpFormat::Standard),
            edit("same", Some(&local), JsonMcpFormat::Standard),
            edit("same", Some(&remote), JsonMcpFormat::OpenCode),
        ],
    )
    .unwrap();
    assert!(output.starts_with('\u{feff}'));
    assert!(output.contains("// unrelated settings\r\n  \"theme\": \"dark\""));
    assert!(output
        .contains("\"keep\": {\"type\": \"local\", \"command\": [\"keep\"], \"enabled\": false}"));
    let value = parse(&output);
    assert!(value["mcpServers"].get("remove").is_none());
    assert_eq!(value["mcpServers"]["same"]["command"], "node");
    assert_eq!(value["mcp"]["same"]["url"], remote.command);
}

#[test]
fn noop_edits_are_byte_exact_and_missing_deletion_does_not_create_directories() {
    let config = local();
    let source = "\u{feff}{\r\n  // keep formatting\r\n  \"mcpServers\": {\"service\": {\"type\": \"stdio\", \"command\": \"node\", \"args\": [\"server.js\"], \"env\": {\"TOKEN\": \"secret\"},}},\r\n}\r\n";
    assert_eq!(
        edit_text(
            source,
            &[edit("service", Some(&config), JsonMcpFormat::Standard)]
        )
        .unwrap(),
        source
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing/settings.json");
    let _guard = crate::json_config::write_lock().unwrap();
    let plan = prepare(&path, &[edit("missing", None, JsonMcpFormat::Standard)]).unwrap();
    assert!(plan.updates.is_empty());
    assert_eq!(plan.guards.len(), 1);
    plan.commit().unwrap();
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn invalid_batch_input_never_writes_the_valid_first_edit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing/settings.json");
    let valid = local();
    let invalid = McpServerConfig {
        command: "file:///private-secret".into(),
        ..remote()
    };
    for name in ["", " ", "bad\nname", "service"] {
        let result = prepare(
            &path,
            &[
                edit("valid", Some(&valid), JsonMcpFormat::Standard),
                edit(name, Some(&invalid), JsonMcpFormat::Standard),
            ],
        );
        let error = result.err().unwrap();
        assert!(!error.contains("private-secret"));
        assert!(!path.parent().unwrap().exists());
    }
    for config in [
        McpServerConfig {
            command: " ".into(),
            ..local()
        },
        McpServerConfig {
            transport_type: Some("unsupported".into()),
            ..local()
        },
        McpServerConfig {
            args: vec!["bad\0arg".into()],
            ..local()
        },
    ] {
        assert!(prepare(
            &path,
            &[edit("service", Some(&config), JsonMcpFormat::OpenCode)]
        )
        .is_err());
    }
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn corrupt_files_and_invalid_existing_entries_remain_untouched_and_errors_are_safe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    for source in [
        b"{ private-secret".as_slice(),
        b"{\"mcpServers\": 3}",
        b"{\"mcpServers\": {\"service\": \"private-secret\"}}",
        b"{\"mcpServers\": {}, \"mcpServers\": {}}",
        &[255, 254, 0],
    ] {
        std::fs::write(&path, source).unwrap();
        for config in [Some(local()), None] {
            let error =
                update_at(&path, "service", config.as_ref(), JsonMcpFormat::Standard).unwrap_err();
            assert!(!error.contains("private-secret"));
            assert!(!error.contains(path.to_str().unwrap()));
            assert_eq!(std::fs::read(&path).unwrap(), source);
        }
    }
    for policy in ["enabled", "disabled"] {
        let source = format!(
            r#"{{"mcpServers":{{"service":{{"command":"old","{policy}":"private-secret"}}}}}}"#
        );
        std::fs::write(&path, &source).unwrap();
        let error =
            update_at(&path, "service", Some(&local()), JsonMcpFormat::Standard).unwrap_err();
        assert!(!error.contains("private-secret"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(update_at(&path, "service", Some(&local()), JsonMcpFormat::Standard).is_err());
    assert!(path.is_dir());
}

#[test]
fn preparation_guards_both_changed_and_unchanged_files_against_external_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let _guard = crate::json_config::write_lock().unwrap();
    for config in [Some(local()), None] {
        std::fs::write(&path, "{}\n").unwrap();
        let plan = prepare(
            &path,
            &[edit("service", config.as_ref(), JsonMcpFormat::Standard)],
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}\n");
        std::fs::write(&path, "{\"newer\":true}\n").unwrap();
        assert!(plan.commit().is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"newer\":true}\n"
        );
    }
}

#[test]
fn mixed_native_group_restores_exact_bytes_and_catalog_on_sqlite_finalization_failure() {
    let dir = tempfile::tempdir().unwrap();
    let json = dir.path().join("settings.jsonc");
    let toml = dir.path().join("config.toml");
    let created = dir.path().join("new/opencode.json");
    let original_json = "\u{feff}{\r\n // preserve\r\n \"mcpServers\": {\"service\": {\"command\": \"old\", \"enabled\": false}}\r\n}\r\n";
    let original_toml = "# preserve\n[mcp_servers.service]\ncommand='old'\nenabled=false\n";
    std::fs::write(&json, original_json).unwrap();
    std::fs::write(&toml, original_toml).unwrap();
    let config = local();
    let _guard = crate::json_config::write_lock().unwrap();
    let mut plan = prepare(
        &json,
        &[edit("service", Some(&config), JsonMcpFormat::Standard)],
    )
    .unwrap();
    plan.extend(
        super::super::native_toml::prepare(
            &toml,
            "service",
            Some(&config),
            super::super::native_toml::Format::Codex,
        )
        .unwrap(),
    );
    plan.extend(
        prepare(
            &created,
            &[edit("service", Some(&config), JsonMcpFormat::OpenCode)],
        )
        .unwrap(),
    );
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE selected (name TEXT CHECK (name = 'original')); INSERT INTO selected VALUES ('original');").unwrap();
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let error = plan
        .commit_then(|| {
            assert_eq!(
                parse(&std::fs::read_to_string(&json).unwrap())["mcpServers"]["service"]["command"],
                "node"
            );
            assert!(created.exists());
            tx.execute("UPDATE selected SET name = 'invalid'", [])
                .map_err(|_| "database rejected")?;
            tx.commit().map_err(|_| "database commit rejected".into())
        })
        .unwrap_err();
    assert_eq!(error, "database rejected");
    assert_eq!(
        conn.query_row("SELECT name FROM selected", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "original"
    );
    assert_eq!(std::fs::read_to_string(&json).unwrap(), original_json);
    assert_eq!(std::fs::read_to_string(&toml).unwrap(), original_toml);
    assert!(!created.parent().unwrap().exists());
}
