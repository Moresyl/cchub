use super::*;
use serde_json::json;
use std::path::Path;

fn connection() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn
}

fn binding(tool: &str, file: &Path, role: SourceRole) -> SourceBinding {
    SourceBinding {
        tool: tool.into(),
        path: file.into(),
        role,
    }
}

fn configure(conn: &Connection, tool: &str, file: &Path, dir: &Path) {
    conn.execute(
        "INSERT INTO custom_paths(tool_id, mcp_config_path, config_dir) VALUES(?1, ?2, ?3)",
        rusqlite::params![tool, file.to_str().unwrap(), dir.to_str().unwrap()],
    )
    .unwrap();
}

fn configured_fixture() -> (tempfile::TempDir, Connection) {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    for tool in [
        "claude",
        "claude-desktop",
        "codex",
        "gemini",
        "grokbuild",
        "opencode",
        "hermes",
        "mcode",
    ] {
        let file = root.path().join(format!("{tool}-exact.native"));
        configure(&conn, tool, &file, &root.path().join(tool));
        let text = match tool {
            "codex" | "grokbuild" => format!("[mcp_servers.shared]\ncommand='{tool}'\nargs=['one']\nenabled=false\nstartup_timeout_sec=91\nunknown_extension='keep'\n[mcp_servers.shared.env]\nTOKEN='private-fixture'\n"),
            "hermes" => format!("mcp_servers:\n  shared:\n    command: {tool}\n    args: [one]\n    env: {{TOKEN: private-fixture}}\n    enabled: false\n    timeout: 91\n    unknown_extension: keep\n"),
            "opencode" => json!({"mcp":{"shared":{"type":"local","command":[tool,"one"],"environment":{"TOKEN":"private-fixture"},"enabled":false,"timeout":91,"unknown_extension":"keep"}}}).to_string(),
            _ => json!({"mcpServers":{"shared":{"command":tool,"args":["one"],"env":{"TOKEN":"private-fixture"},"enabled":false,"timeout":91,"unknown_extension":"keep"}}}).to_string(),
        };
        std::fs::write(file, text).unwrap();
    }
    (root, conn)
}

#[test]
fn configured_complete_scopes_keep_same_name_sources_disabled_and_native_options() {
    let (root, conn) = configured_fixture();
    // Same display/native name exists in the primary, settings and plugin file.
    let settings = root.path().join("claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(
        &settings,
        r#"{"mcpServers":{"shared":{"command":"secondary","disabled":true}}}"#,
    )
    .unwrap();
    let plugin = root.path().join("claude/plugins/.hidden/package/.mcp.json");
    std::fs::create_dir_all(plugin.parent().unwrap()).unwrap();
    std::fs::write(&plugin, "\u{feff}{// comment\n\"$schema\":\"private\",\"shared\":{\"command\":\"plugin\",\"args\":[\"--mcp\"]}}\r\n").unwrap();
    let first = SourceSnapshot::read_configured(&conn).unwrap();
    assert_eq!(first.origins.len(), 10);
    assert_eq!(first.documents.len(), 10);
    assert_eq!(first.plugin_roots, [root.path().join("claude/plugins")]);
    let ids: std::collections::HashSet<_> = first.origins.iter().map(|origin| &origin.id).collect();
    assert_eq!(ids.len(), 10);
    for origin in &first.origins {
        assert_eq!(origin.native_name, "shared");
        assert_eq!(origin.connection.transport, "stdio");
        if origin.bindings[0].role == SourceRole::Primary {
            assert!(origin.disabled);
            assert_eq!(origin.connection.args, ["one"]);
            assert_eq!(origin.connection.env["TOKEN"], "private-fixture");
            let encoding = match &origin.spec {
                NativeSpec::Json(value) | NativeSpec::Toml(value) | NativeSpec::Yaml(value) => {
                    value
                }
            };
            assert!(encoding.contains("unknown_extension"));
        }
    }
    let second = SourceSnapshot::read_configured(&conn).unwrap();
    assert_eq!(
        first
            .origins
            .iter()
            .map(|origin| &origin.id)
            .collect::<Vec<_>>(),
        second
            .origins
            .iter()
            .map(|origin| &origin.id)
            .collect::<Vec<_>>()
    );
    for document in first.documents {
        assert_eq!(
            crate::config_write::read(&document.canonical_path).unwrap(),
            document.original
        );
    }
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM mcp_servers", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn canonical_aliases_join_one_origin_and_incompatible_aliases_fail() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("aliases.json");
    std::fs::write(
        &file,
        r#"{"mcpServers":{"same":{"command":"node","env":{"X":"y"},"enabled":false}}}"#,
    )
    .unwrap();
    let aliases = [
        binding("claude", &file, SourceRole::Primary),
        binding(
            "mcode",
            &root.path().join("./aliases.json"),
            SourceRole::Primary,
        ),
        binding("claude", &file, SourceRole::Secondary),
    ];
    let snapshot = SourceSnapshot::read_bindings(&aliases).unwrap();
    assert_eq!(snapshot.documents.len(), 1);
    assert_eq!(snapshot.origins.len(), 1);
    assert_eq!(snapshot.origins[0].bindings.len(), 3);
    let id = snapshot.origins[0].id.clone();
    let reversed: Vec<_> = aliases.into_iter().rev().collect();
    assert_eq!(
        SourceSnapshot::read_bindings(&reversed).unwrap().origins[0].id,
        id
    );
    // A Gemini url defaults to SSE while Standard defaults to HTTP.
    std::fs::write(
        &file,
        r#"{"mcpServers":{"same":{"url":"https://fixture.test/mcp"}}}"#,
    )
    .unwrap();
    let error = SourceSnapshot::read_bindings(&[
        binding("claude", &file, SourceRole::Primary),
        binding("gemini", &file, SourceRole::Primary),
    ])
    .unwrap_err();
    assert!(error.contains("interpret"));
    assert!(!error.contains("fixture.test"));
}

#[test]
fn identity_includes_container_and_unambiguous_native_name_but_not_connection_revision() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("multi.json");
    std::fs::write(&file, r#"{"mcpServers":{"a/b":{"command":"one"}},"mcp":{"a/b":{"type":"local","command":["two"]}}}"#).unwrap();
    let aliases = [
        binding("claude", &file, SourceRole::Primary),
        binding("opencode", &file, SourceRole::Primary),
    ];
    let before = SourceSnapshot::read_bindings(&aliases).unwrap();
    assert_eq!(before.documents.len(), 1);
    assert_eq!(before.origins.len(), 2);
    assert_ne!(before.origins[0].id, before.origins[1].id);
    std::fs::write(&file, r#"{"mcpServers":{"a/b":{"command":"edited"}},"mcp":{"a/b":{"type":"local","command":["two"]}}}"#).unwrap();
    let after = SourceSnapshot::read_bindings(&aliases).unwrap();
    assert_eq!(
        before
            .origins
            .iter()
            .map(|origin| &origin.id)
            .collect::<Vec<_>>(),
        after
            .origins
            .iter()
            .map(|origin| &origin.id)
            .collect::<Vec<_>>()
    );
    assert_ne!(
        logical_id(root.path(), "a/b", "c").unwrap(),
        logical_id(root.path(), "a", "b/c").unwrap()
    );
}

#[test]
fn missing_scopes_have_absent_revisions_and_create_no_directories() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    for tool in [
        "claude",
        "claude-desktop",
        "codex",
        "gemini",
        "grokbuild",
        "opencode",
        "hermes",
        "mcode",
    ] {
        configure(
            &conn,
            tool,
            &root.path().join(tool).join("missing.native"),
            &root.path().join(tool),
        );
    }
    let snapshot = SourceSnapshot::read_configured(&conn).unwrap();
    assert_eq!(snapshot.documents.len(), 9);
    assert!(snapshot
        .documents
        .iter()
        .all(|document| document.original.is_none()));
    assert!(snapshot.origins.is_empty());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn malformed_siblings_and_native_types_fail_the_complete_read_without_any_writes() {
    let (root, conn) = configured_fixture();
    for broken in [
        r#"{"mcpServers":{"good":{"command":"node"},"bad":{"command":"private-fixture","args":[1]}}}"#,
        r#"{"mcpServers":{"bad":{"command":"private-fixture","env":{"X":1}}}}"#,
        r#"{"mcpServers":{"bad":{"url":"private-fixture"}}}"#,
        r#"{"mcpServers":{ "bad":{"command":"node","enabled":"private-fixture"}}}"#,
        r#"{"mcpServers":{},"mcpServers":{"private-fixture":{}}}"#,
    ] {
        let file = root.path().join("gemini-exact.native");
        std::fs::write(&file, broken).unwrap();
        let error = SourceSnapshot::read_configured(&conn).unwrap_err();
        assert!(!error.contains("private-fixture"));
        assert_eq!(std::fs::read_to_string(file).unwrap(), broken);
    }
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM mcp_servers", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn settings_plugin_and_path_failures_cannot_be_silently_omitted_or_fall_back() {
    let (root, conn) = configured_fixture();
    let settings = root.path().join("claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(&settings, r#"{"mcpServers":{"bad":{"args":[]}}}"#).unwrap();
    assert!(SourceSnapshot::read_configured(&conn).is_err());
    std::fs::write(&settings, "{}").unwrap();
    let plugins = root.path().join("claude/plugins");
    std::fs::write(&plugins, "not a directory").unwrap();
    assert!(SourceSnapshot::read_configured(&conn).is_err());
    std::fs::remove_file(&plugins).unwrap();
    std::fs::create_dir_all(&plugins).unwrap();
    let file = plugins.join(".mcp.json");
    std::fs::write(
        &file,
        r#"{"good":{"command":"node"},"bad":"private-fixture"}"#,
    )
    .unwrap();
    assert!(SourceSnapshot::read_configured(&conn).is_err());
    std::fs::write(&file, "{}").unwrap();
    conn.execute(
        "UPDATE custom_paths SET mcp_config_path='relative.native' WHERE tool_id='codex'",
        [],
    )
    .unwrap();
    assert!(SourceSnapshot::read_configured(&conn).is_err());
    // A real settings schema failure is not a missing override.
    conn.execute("DROP TABLE custom_paths", []).unwrap();
    assert!(SourceSnapshot::read_configured(&conn).is_err());
}

#[test]
fn typed_native_extensions_and_exact_document_bytes_survive_without_json_coercion() {
    let root = tempfile::tempdir().unwrap();
    let toml = root.path().join("native.toml");
    let yaml = root.path().join("native.yaml");
    let toml_source = "\u{feff}# keep\r\n[mcp_servers.shared]\r\ncommand='node'\r\nweight=nan\r\nexpiry=2026-10-03T00:00:00Z\r\n";
    let yaml_source = "\u{feff}# keep\r\nmcp_servers:\r\n  shared:\r\n    command: node\r\n    tagged: !custom keep\r\n    weight: .nan\r\n    extra: {1: native}\r\n# tail\r\n";
    std::fs::write(&toml, toml_source).unwrap();
    std::fs::write(&yaml, yaml_source).unwrap();
    let snapshot = SourceSnapshot::read_bindings(&[
        binding("codex", &toml, SourceRole::Primary),
        binding("hermes", &yaml, SourceRole::Primary),
    ])
    .unwrap();
    for origin in snapshot.origins {
        match origin.spec {
            NativeSpec::Toml(source) => {
                let entry: toml::Table = toml::from_str(&source).unwrap();
                assert!(entry["weight"].as_float().unwrap().is_nan());
                assert!(entry["expiry"].is_datetime());
            }
            NativeSpec::Yaml(source) => {
                let entry: serde_yaml::Value = serde_yaml::from_str(&source).unwrap();
                assert!(matches!(&entry["tagged"], serde_yaml::Value::Tagged(_)));
                assert!(entry["weight"].as_f64().unwrap().is_nan());
                assert!(entry["extra"]
                    .as_mapping()
                    .unwrap()
                    .contains_key(serde_yaml::Value::Number(1.into())));
            }
            NativeSpec::Json(_) => panic!("native values became JSON"),
        }
    }
    assert_eq!(
        snapshot
            .documents
            .iter()
            .find(|doc| doc.canonical_path == crate::config_write::target_key(&toml).unwrap())
            .unwrap()
            .original
            .as_deref(),
        Some(toml_source.as_bytes())
    );
    assert_eq!(std::fs::read_to_string(yaml).unwrap(), yaml_source);
}

#[test]
fn yaml_root_and_entry_merges_have_effective_connection_and_retained_source() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("merged.yaml");
    let source = "connection: &connection\n  command: node\n  args: [--mcp]\n  env: {TOKEN: private-fixture}\n  enabled: false\n  timeout: 91\nbase: &base\n  mcp_servers:\n    shared:\n      <<: *connection\n      extension: keep\n<<: *base\n";
    std::fs::write(&file, source).unwrap();
    let snapshot =
        SourceSnapshot::read_bindings(&[binding("hermes", &file, SourceRole::Primary)]).unwrap();
    assert_eq!(snapshot.origins.len(), 1);
    let origin = &snapshot.origins[0];
    assert!(origin.disabled);
    assert_eq!(origin.connection.command, "node");
    assert_eq!(origin.connection.args, ["--mcp"]);
    assert_eq!(origin.connection.env["TOKEN"], "private-fixture");
    assert_eq!(
        snapshot.documents[0].original.as_deref(),
        Some(source.as_bytes())
    );
    let NativeSpec::Yaml(spec) = &origin.spec else {
        panic!("wrong native format")
    };
    let entry: serde_yaml::Value = serde_yaml::from_str(spec).unwrap();
    assert_eq!(entry["timeout"], 91);
    assert_eq!(entry["extension"], "keep");
    assert!(entry.get("<<").is_none());
}

#[test]
fn transports_keep_native_headers_separate_from_stdio_environment() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("remote.json");
    std::fs::write(&file, r#"{"mcpServers":{"sse":{"url":"https://fixture.test/sse","headers":{"Authorization":"private-fixture"},"env":{"UNRELATED":"keep"}},"http":{"httpUrl":"https://fixture.test/http","headers":{"X":"y"}}}}"#).unwrap();
    let snapshot =
        SourceSnapshot::read_bindings(&[binding("gemini", &file, SourceRole::Primary)]).unwrap();
    let sse = snapshot
        .origins
        .iter()
        .find(|origin| origin.native_name == "sse")
        .unwrap();
    assert_eq!(sse.connection.transport, "sse");
    assert_eq!(sse.connection.headers["Authorization"], "private-fixture");
    assert!(sse.connection.env.is_empty());
    let http = snapshot
        .origins
        .iter()
        .find(|origin| origin.native_name == "http")
        .unwrap();
    assert_eq!(http.connection.transport, "http");
    let NativeSpec::Json(spec) = &sse.spec else {
        panic!("wrong format")
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(spec).unwrap()["env"]["UNRELATED"],
        "keep"
    );
}

#[test]
fn non_file_invalid_utf8_and_unsupported_sources_fail_safely() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("invalid.json");
    std::fs::write(&file, [0xff, 0xfe]).unwrap();
    assert!(
        SourceSnapshot::read_bindings(&[binding("claude", &file, SourceRole::Primary)]).is_err()
    );
    assert!(
        SourceSnapshot::read_bindings(&[binding("claude", root.path(), SourceRole::Primary)])
            .is_err()
    );
    let missing = root.path().join("missing.native");
    assert!(
        SourceSnapshot::read_bindings(&[binding("openclaw", &missing, SourceRole::Primary)])
            .is_err()
    );
    assert!(!missing.exists());
}

#[test]
fn retained_snapshot_detects_native_edits_missing_creation_and_plugin_membership_changes() {
    let (root, conn) = configured_fixture();
    let snapshot = SourceSnapshot::read_configured(&conn).unwrap();
    snapshot.verify().unwrap();
    let file = root.path().join("codex-exact.native");
    let old = std::fs::read(&file).unwrap();
    std::fs::write(&file, "# changed externally\n").unwrap();
    assert!(snapshot.verify().is_err());
    std::fs::write(&file, old).unwrap();
    let missing = root.path().join("claude/settings.json");
    std::fs::create_dir_all(missing.parent().unwrap()).unwrap();
    std::fs::write(&missing, "{}").unwrap();
    assert!(snapshot.verify().is_err());
    std::fs::remove_file(&missing).unwrap();
    snapshot.verify().unwrap();
    let plugin = root.path().join("claude/plugins/new/.mcp.json");
    std::fs::create_dir_all(plugin.parent().unwrap()).unwrap();
    std::fs::write(&plugin, r#"{"added":{"command":"node"}}"#).unwrap();
    assert!(snapshot.verify().is_err());
    let snapshot = SourceSnapshot::read_configured(&conn).unwrap();
    std::fs::remove_file(&plugin).unwrap();
    assert!(snapshot.verify().is_err());
}

#[test]
fn export_checks_native_types_instead_of_coercing_them() {
    let spec = NativeSpec::Toml("command='node'\nweight=nan\n".into());
    assert!(spec.to_json().is_err());
    let spec = NativeSpec::Yaml("command: node\ntagged: !custom private-fixture\n".into());
    let error = spec.to_json().unwrap_err();
    assert!(!error.contains("private-fixture"));
    let spec = NativeSpec::Json(
        r#"{"url":"https://fixture.test/mcp","headers":{"X":"y"},"type":"http"}"#.into(),
    );
    assert_eq!(spec.to_json().unwrap()["type"], "http");
}

#[test]
fn same_byte_file_replacement_invalidates_an_entire_source_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("native.json");
    let bytes = br#"{"mcpServers":{"same":{"command":"node"}}}"#;
    std::fs::write(&file, bytes).unwrap();
    let snapshot =
        SourceSnapshot::read_bindings(&[binding("claude", &file, SourceRole::Primary)]).unwrap();
    crate::utils::atomic_write(&file, bytes).unwrap();
    assert!(snapshot.verify().is_err());
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}
