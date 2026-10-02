use super::*;
use crate::yaml_config::same;
use std::collections::HashMap;

fn local() -> McpServerConfig {
    McpServerConfig {
        command: "node".into(),
        args: vec!["server.js".into()],
        env: HashMap::from([("TOKEN".into(), "secret".into())]),
        transport_type: Some("stdio".into()),
    }
}

fn remote(transport: &str) -> McpServerConfig {
    McpServerConfig {
        command: "https://example.com/mcp".into(),
        args: Vec::new(),
        env: HashMap::from([("Authorization".into(), "Bearer secret".into())]),
        transport_type: Some(transport.into()),
    }
}

fn edit<'a>(name: &'a str, config: Option<&'a McpServerConfig>) -> Edit<'a> {
    Edit { name, config }
}

fn parse(text: &str) -> Value {
    serde_yaml::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)).unwrap()
}

#[test]
fn changes_preserve_yaml_stream_comments_native_policies_and_unrelated_values() {
    let source = "\u{feff}# header\r\n%YAML 1.2\r\n---\r\nmodel: 'keep' # model\r\ncustom: !option {number: .nan, yes: true}\r\nmcp_servers:\r\n  'service.name': # connection\r\n    command: 'old' # command\r\n    args: [before]\r\n    env:\r\n      TOKEN: 'secret' # same token\r\n    enabled: false # policy\r\n    timeout: 99\r\n    connect_timeout: 12\r\n    auth: oauth\r\n    tools: {allow: ['read'], deny: ['write']}\r\n    lifecycle: lazy\r\n    custom: !extension {score: .nan, 7: retained}\r\n  sibling:\r\n    command: 'untouched' # sibling\r\n...\r\n# footer\r\n";
    let config = local();
    let output = edit_text(source, &[edit("service.name", Some(&config))]).unwrap();
    let before = parse(source);
    let next = parse(&output);
    let entry = &next["mcp_servers"]["service.name"];
    assert_eq!(entry["command"], "node");
    assert_eq!(entry["args"][0], "server.js");
    for field in [
        "enabled",
        "timeout",
        "connect_timeout",
        "auth",
        "tools",
        "lifecycle",
        "custom",
    ] {
        assert!(same(
            &entry[field],
            &before["mcp_servers"]["service.name"][field]
        ));
    }
    assert!(same(&next["custom"], &before["custom"]));
    for trivia in [
        "# header",
        "%YAML 1.2",
        "# model",
        "# connection",
        "# command",
        "TOKEN: 'secret' # same token",
        "# policy",
        "command: 'untouched' # sibling",
        "...\r\n# footer",
    ] {
        assert!(output.contains(trivia), "missing {trivia}");
    }
    assert!(output.starts_with('\u{feff}'));
    assert!(!output.replace("\r\n", "").contains('\n'));
    assert_eq!(
        edit_text(&output, &[edit("service.name", Some(&config))]).unwrap(),
        output
    );
}

#[test]
fn hermes_transport_round_trips_sse_http_aliases_and_stdio_without_forcing_options() {
    let source = "mcp_servers:\n  service:\n    command: old\n    args: [old]\n    env: {OLD: value}\n    enabled: false\n    timeout: 91\n    auth: oauth\n    tools: {deny: [write]}\n";
    let mut text = source.to_owned();
    for transport in ["sse", "http", "remote", "streamable-http", "sse"] {
        let config = remote(transport);
        text = edit_text(&text, &[edit("service", Some(&config))]).unwrap();
        let entry = parse(&text)["mcp_servers"]["service"].clone();
        assert_eq!(entry["url"], "https://example.com/mcp");
        assert_eq!(entry["headers"]["Authorization"], "Bearer secret");
        assert_eq!(
            entry.get("transport").and_then(Value::as_str),
            (transport == "sse").then_some("sse")
        );
        for stale in ["command", "args", "env", "type"] {
            assert!(entry.get(stale).is_none());
        }
        assert_eq!(entry["enabled"], false);
        assert_eq!(entry["timeout"], 91);
        assert_eq!(entry["auth"], "oauth");
    }
    text = edit_text(&text, &[edit("service", Some(&local()))]).unwrap();
    let entry = parse(&text)["mcp_servers"]["service"].clone();
    for stale in ["url", "headers", "transport", "type"] {
        assert!(entry.get(stale).is_none());
    }
    assert_eq!(entry["command"], "node");
    assert_eq!(entry["enabled"], false);
    let created = parse(&edit_text("{}\n", &[edit("new", Some(&remote("sse")))]).unwrap());
    assert_eq!(created["mcp_servers"]["new"]["transport"], "sse");
    for generated in ["timeout", "enabled", "auth"] {
        assert!(created["mcp_servers"]["new"].get(generated).is_none());
    }
}

#[test]
fn batches_compose_quoted_names_flow_maps_and_nested_key_replacement() {
    let source = "# header\nmcp_servers: {'service.name': {command: old, env: {OLD: old}, enabled: false}, remove: {command: gone}}\nmodel: keep\n";
    let output = edit_text(
        source,
        &[
            edit("service.name", Some(&local())),
            edit("remove", None),
            edit("second name", Some(&remote("sse"))),
        ],
    )
    .unwrap();
    let value = parse(&output);
    assert_eq!(value["model"], "keep");
    assert_eq!(
        value["mcp_servers"]["service.name"]["env"]["TOKEN"],
        "secret"
    );
    assert!(value["mcp_servers"]["service.name"]["env"]
        .get("OLD")
        .is_none());
    assert!(value["mcp_servers"].get("remove").is_none());
    assert_eq!(value["mcp_servers"]["second name"]["transport"], "sse");
    assert!(output.starts_with("# header\n"));
    assert_eq!(
        edit_text(&output, &[edit("missing", None)]).unwrap(),
        output
    );
}

#[test]
fn invalid_input_fails_before_creating_files_and_safe_errors_do_not_leak_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not-created/config.yaml");
    let mut bad = local();
    for name in ["", "  ", "bad\nname"] {
        assert!(prepare(&path, &[edit(name, Some(&bad))]).is_err());
    }
    for command in ["", "\0secret"] {
        bad.command = command.into();
        assert!(prepare(&path, &[edit("service", Some(&bad))]).is_err());
    }
    bad = local();
    bad.args.push("bad\0arg".into());
    assert!(prepare(&path, &[edit("service", Some(&bad))]).is_err());
    bad = remote("sse");
    bad.command = "file:///private-secret".into();
    assert!(prepare(&path, &[edit("service", Some(&bad))]).is_err());
    bad = local();
    bad.transport_type = Some("udp".into());
    assert!(prepare(&path, &[edit("service", Some(&bad))]).is_err());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    let path = dir.path().join("existing.yaml");
    for source in [
        "[private-secret]",
        "mcp_servers: [private-secret]",
        "mcp_servers: {service: private-secret}",
        "mcp_servers: {service: {command: old, enabled: private-secret}}",
        "mcp_servers: {service: {command: old, disabled: private-secret}}",
        "mcp_servers: {service: {command: a, command: private-secret}}",
        "bad: [private-secret",
        "---\nmodel: keep\n---\nmodel: private-secret\n",
    ] {
        std::fs::write(&path, source).unwrap();
        let error = update_at(&path, "service", Some(&local())).unwrap_err();
        assert!(!error.contains("private-secret"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    assert!(prepare(&path, &[edit("service", None)]).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), [0xff, 0xfe]);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(prepare(&path, &[edit("service", None)]).is_err());
}

#[test]
fn missing_removal_and_semantic_noops_are_byte_identical_and_guard_external_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not-created/config.yaml");
    let _guard = crate::json_config::write_lock().unwrap();
    prepare(&path, &[edit("missing", None)])
        .unwrap()
        .commit()
        .unwrap();
    assert!(!path.parent().unwrap().exists());
    let path = dir.path().join("config.yaml");
    let source = "# keep\nmcp_servers:\n  service:\n    command: 'node' # command\n    args: [server.js]\n    env: {TOKEN: secret}\n    extra: .nan\n";
    let local = local();
    let remote = remote("sse");
    for edit in [
        edit("service", Some(&local)),
        edit("service", Some(&remote)),
        edit("missing", None),
    ] {
        std::fs::write(&path, source).unwrap();
        let plan = prepare(&path, &[edit]).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        std::fs::write(&path, "model: newer\n").unwrap();
        assert!(plan.commit().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "model: newer\n");
    }
    assert_eq!(
        edit_text(source, &[edit("service", Some(&local))]).unwrap(),
        source
    );
}

#[test]
fn alias_side_effects_are_rejected_instead_of_changing_unrelated_settings() {
    let source = "mcp_servers:\n  service:\n    command: &shared old\nother: *shared\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, source).unwrap();
    assert!(update_at(&path, "service", Some(&local())).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(edit_text(source, &[edit("missing", None)]).unwrap(), source);
}

#[test]
fn argument_edits_keep_unchanged_items_and_their_comments() {
    let source = "mcp_servers:\n  service:\n    command: node\n    args:\n      - before # changed\n      - same # keep this item\n    env: {TOKEN: secret}\n";
    let mut config = local();
    config.args = vec!["after".into(), "same".into(), "new".into()];
    let output = edit_text(source, &[edit("service", Some(&config))]).unwrap();
    assert!(output.contains("same # keep this item"));
    assert_eq!(
        parse(&output)["mcp_servers"]["service"]["args"],
        serde_yaml::to_value(&config.args).unwrap()
    );
    config.args = vec!["after".into()];
    let removed = edit_text(&output, &[edit("service", Some(&config))]).unwrap();
    assert_eq!(
        parse(&removed)["mcp_servers"]["service"]["args"],
        serde_yaml::to_value(&config.args).unwrap()
    );
}

#[test]
fn argument_layouts_and_yaml_sensitive_strings_round_trip_without_comment_drift() {
    for args in [
        "    args:\n      - before # last\n",
        "    args:\n      - before # last",
        "    args:\n      - before\n\n    # following field\n",
        "    args: [before] # list\n",
        "    args:\n      - before # first\n      - same # last\n",
    ] {
        let source = format!("# header\nmcp_servers:\n  service:\n    command: node\n{args}");
        for crlf in [false, true] {
            let source = if crlf {
                source.replace('\n', "\r\n")
            } else {
                source.clone()
            };
            let mut config = local();
            config.args = vec![
                "after".into(),
                "same".into(),
                "a, b: [c] # value\nsecond line".into(),
                "null".into(),
                "yes".into(),
            ];
            config.env = HashMap::from([("TOKEN: # [key]".into(), "null\nsecond line".into())]);
            let output = edit_text(&source, &[edit("service", Some(&config))]).unwrap();
            let value = parse(&output);
            assert_eq!(
                value["mcp_servers"]["service"]["args"],
                serde_yaml::to_value(&config.args).unwrap()
            );
            assert_eq!(
                value["mcp_servers"]["service"]["env"],
                serde_yaml::to_value(&config.env).unwrap()
            );
            for comment in [
                "# header",
                "# last",
                "# first",
                "# list",
                "# following field",
            ] {
                if source.contains(comment) {
                    assert!(output.contains(comment), "lost {comment}");
                }
            }
            if crlf {
                assert!(!output.replace("\r\n", "").contains('\n'));
            }
            assert_eq!(
                edit_text(&output, &[edit("service", Some(&config))]).unwrap(),
                output
            );
        }
    }
}

#[test]
fn hermes_native_transport_validation_rejects_ambiguous_or_unusable_sources() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO custom_paths(tool_id, mcp_config_path) VALUES ('hermes', ?1)",
        [path.to_str().unwrap()],
    )
    .unwrap();
    for fields in [
        "url: https://example.com, transport: 7",
        "url: https://example.com, transport: udp",
        "url: https://example.com, type: sse",
        "url: https://example.com, type: http, transport: sse",
        "url: https://example.com, transport: stdio",
        "command: node, transport: sse",
        "command: node, args: [private-secret, 7]",
        "command: node, env: {TOKEN: [private-secret]}",
    ] {
        let source = format!("mcp_servers: {{valid: {{command: keep}}, invalid: {{{fields}}}}}\n");
        std::fs::write(&path, &source).unwrap();
        let error = super::super::native_read::read_config(&conn, "hermes").unwrap_err();
        assert!(!error.contains("private-secret"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
    let schema_error = rusqlite::Connection::open_in_memory().unwrap();
    assert!(crate::hermes::mcp::write_server(&schema_error, "service", &local()).is_err());
    assert!(crate::hermes::mcp::remove_server(&schema_error, "service").is_err());
}

#[test]
fn exact_custom_path_scan_write_and_remove_use_the_same_native_transport() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("custom-native.yaml");
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO custom_paths(tool_id, config_dir, mcp_config_path) VALUES ('hermes', ?1, ?2)",
        rusqlite::params![
            dir.path().join("unused-root").to_str().unwrap(),
            path.to_str().unwrap()
        ],
    )
    .unwrap();
    std::fs::write(
        &path,
        "# keep\nmcp_servers:\n  service:\n    command: old\n    enabled: false\n    timeout: 92\n",
    )
    .unwrap();
    crate::hermes::mcp::write_server(&conn, "service", &remote("sse")).unwrap();
    let services = crate::hermes::mcp::scan_servers(&conn).unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].transport, "sse");
    assert_eq!(services[0].config_path, path.to_str().unwrap());
    assert_eq!(services[0].env["Authorization"], "Bearer secret");
    assert!(crate::hermes::mcp::has_server(&conn, "service").unwrap());
    let next = std::fs::read_to_string(&path).unwrap();
    assert!(next.starts_with("# keep\n"));
    assert_eq!(parse(&next)["mcp_servers"]["service"]["enabled"], false);
    assert_eq!(parse(&next)["mcp_servers"]["service"]["timeout"], 92);
    crate::hermes::mcp::write_server(&conn, "service", &remote("http")).unwrap();
    assert_eq!(
        crate::hermes::mcp::scan_servers(&conn).unwrap()[0].transport,
        "http"
    );
    crate::hermes::mcp::remove_server(&conn, "service").unwrap();
    assert!(!crate::hermes::mcp::has_server(&conn, "service").unwrap());
    assert!(crate::hermes::mcp::scan_servers(&conn).unwrap().is_empty());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let bad = "mcp_servers: {broken: {url: https://example.com, transport: udp}}\n";
    std::fs::write(&path, bad).unwrap();
    assert!(super::super::native_read::read_config(&conn, "hermes").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), bad);
}

#[test]
fn grouped_yaml_and_json_writes_restore_exact_originals_on_actual_sqlite_failure() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = dir.path().join("config.yaml");
    let json = dir.path().join("settings.jsonc");
    let created = dir.path().join("new/config.yaml");
    let original_yaml = "\u{feff}# keep\r\nmcp_servers:\r\n  service:\r\n    command: 'old' # connection\r\n    enabled: false\r\n";
    let original_json = "{\n // keep\n \"mcpServers\": {\"service\": {\"command\": \"old\"}}\n}\n";
    std::fs::write(&yaml, original_yaml).unwrap();
    std::fs::write(&json, original_json).unwrap();
    let config = remote("sse");
    let _guard = crate::json_config::write_lock().unwrap();
    let mut plan = prepare(&yaml, &[edit("service", Some(&config))]).unwrap();
    plan.extend(prepare(&created, &[edit("service", Some(&config))]).unwrap());
    plan.extend(
        super::super::native_json::prepare(
            &json,
            &[super::super::native_json::Edit {
                name: "service",
                config: Some(&config),
                format: super::super::formats::JsonMcpFormat::Standard,
            }],
        )
        .unwrap(),
    );
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE catalog (name TEXT CHECK (name = 'original')); INSERT INTO catalog VALUES ('original');").unwrap();
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let error = plan
        .commit_then(|| {
            assert_eq!(
                parse(&std::fs::read_to_string(&yaml).unwrap())["mcp_servers"]["service"]
                    ["transport"],
                "sse"
            );
            assert!(created.exists());
            tx.execute("UPDATE catalog SET name = 'invalid'", [])
                .map_err(|_| "database rejected")?;
            tx.commit().map_err(|_| "database commit rejected".into())
        })
        .unwrap_err();
    assert_eq!(error, "database rejected");
    assert_eq!(
        conn.query_row("SELECT name FROM catalog", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "original"
    );
    assert_eq!(std::fs::read_to_string(&yaml).unwrap(), original_yaml);
    assert_eq!(std::fs::read_to_string(&json).unwrap(), original_json);
    assert!(!created.parent().unwrap().exists());
}
