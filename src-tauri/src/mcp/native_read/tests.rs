use super::*;
use serde_json::json;

fn connection() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn
}

#[test]
fn invalid_null_characters_in_native_connection_fields_are_rejected() {
    for spec in [
        json!({"command":"node","args":["bad\0argument"]}),
        json!({"command":"node","env":{"TOKEN":"bad\0value"}}),
        json!({"command":"node","env":{"bad\0key":"value"}}),
        json!({"type":"local","command":["node","bad\0argument"]}),
        json!({"url":"https://fixture.invalid/mcp","headers":{"Authorization":"bad\0value"}}),
    ] {
        let tool = if spec["type"] == "local" {
            "opencode"
        } else {
            "claude"
        };
        assert!(validate_json_entry("same", &spec, tool).is_err());
    }
}

fn configure(conn: &Connection, tool: &str, file: &Path) {
    conn.execute(
        "INSERT INTO custom_paths(tool_id, mcp_config_path) VALUES(?1, ?2)",
        rusqlite::params![tool, file.to_str().unwrap()],
    )
    .unwrap();
}

fn fixture(tool: &str) -> String {
    match tool {
        "codex" | "grokbuild" => "model='untouched'\n[mcp_servers.shared]\ncommand='native-tool'\nargs=['one','two']\nenabled=false\nstartup_timeout_sec=90\ncustom_option='retained'\n[mcp_servers.shared.env]\nTOKEN='secret'\n".into(),
        "hermes" => "other: retained\nmcp_servers:\n  shared:\n    command: native-tool\n    args: [one, two]\n    env: {TOKEN: secret}\n    enabled: false\n    timeout: 90\n    custom_option: retained\n".into(),
        "opencode" => json!({"other": "retained", "mcp": {"shared": {"type": "local", "command": ["native-tool", "one", "two"], "environment": {"TOKEN": "secret"}, "enabled": false, "timeout": 90, "custom_option": "retained"}}}).to_string(),
        _ => json!({"other": "retained", "mcpServers": {"shared": {"command": "native-tool", "args": ["one", "two"], "env": {"TOKEN": "secret"}, "enabled": false, "timeout": 90, "custom_option": "retained"}}}).to_string(),
    }
}

#[test]
fn each_tool_reads_its_exact_custom_file_and_preserves_disabled_native_fields() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    let mut files = Vec::new();
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
        let file = root.path().join(format!("{tool}-custom.native"));
        let text = fixture(tool).replace("native-tool", tool);
        std::fs::write(&file, &text).unwrap();
        configure(&conn, tool, &file);
        files.push((file, text));
    }
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
        let view = read_config(&conn, &tool.to_uppercase()).unwrap();
        assert_eq!(
            view.config_path,
            root.path()
                .join(format!("{tool}-custom.native"))
                .to_str()
                .unwrap()
        );
        assert_eq!(view.servers.len(), 1);
        let entry = &view.servers["shared"];
        assert_eq!(entry["enabled"], false);
        assert_eq!(entry["custom_option"], "retained");
        if tool == "opencode" {
            assert_eq!(entry["command"], json!([tool, "one", "two"]));
            assert_eq!(entry["environment"]["TOKEN"], "secret");
        } else {
            assert_eq!(entry["command"], tool);
            assert_eq!(entry["args"], json!(["one", "two"]));
            assert_eq!(entry["env"]["TOKEN"], "secret");
        }
    }
    for (file, original) in files {
        assert_eq!(std::fs::read_to_string(file).unwrap(), original);
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 8);
}

#[test]
fn missing_files_return_an_empty_scope_without_creating_parent_directories() {
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
        let file = root.path().join(tool).join("not-created.native");
        configure(&conn, tool, &file);
        assert!(read_config(&conn, tool).unwrap().servers.is_empty());
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn unrelated_invalid_tool_does_not_replace_or_block_a_requested_source() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    let claude = root.path().join("claude.json");
    let codex = root.path().join("codex.toml");
    std::fs::write(
        &claude,
        r#"{"mcpServers":{"shared":{"command":"claude-own"}}}"#,
    )
    .unwrap();
    std::fs::write(&codex, "credential-bearing-invalid-syntax").unwrap();
    configure(&conn, "claude", &claude);
    configure(&conn, "codex", &codex);
    assert_eq!(
        read_config(&conn, "claude").unwrap().servers["shared"]["command"],
        "claude-own"
    );
    assert!(read_config(&conn, "codex").is_err());
}

#[test]
fn database_schema_and_stored_path_errors_do_not_fall_back_to_home() {
    let conn = Connection::open_in_memory().unwrap();
    assert!(read_config(&conn, "claude")
        .unwrap_err()
        .contains("configured tool paths"));
    let conn = connection();
    conn.execute(
        "INSERT INTO custom_paths(tool_id,mcp_config_path) VALUES('claude',42)",
        [],
    )
    .unwrap();
    // TEXT affinity converts42 to text; it is still invalid as an absolute path.
    assert!(read_config(&conn, "claude").is_err());
    conn.execute("UPDATE custom_paths SET mcp_config_path = x'010203'", [])
        .unwrap();
    assert!(read_config(&conn, "claude")
        .unwrap_err()
        .contains("configured tool paths"));
    for tool in ["unknown", "openclaw", "pi", "claude' OR 1=1 --"] {
        assert!(read_config(&conn, tool)
            .unwrap_err()
            .contains("not supported"));
    }
}

#[test]
fn invalid_utf8_directory_targets_and_malformed_native_documents_are_errors() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.native");
    assert!(read_config_at(root.path(), Format::Standard).is_err());
    std::fs::write(&file, [0xff, 0xfe]).unwrap();
    assert!(read_config_at(&file, Format::Standard)
        .unwrap_err()
        .contains("UTF-8"));
    for (format, text) in [
        (Format::Standard, "{private-secret"),
        (Format::Codex, "private-secret = ["),
        (Format::Hermes, "private-secret: ["),
        (Format::Standard, "[]"),
        (Format::Hermes, "[]"),
        (Format::Standard, r#"{"mcpServers":[]}"#),
        (Format::OpenCode, r#"{"mcp":null}"#),
        (Format::Codex, "mcp_servers=[]"),
        (Format::Hermes, "mcp_servers: null"),
        (
            Format::Standard,
            r#"{"mcpServers":{"private-secret":false}}"#,
        ),
        (Format::Codex, "[mcp_servers]\nprivate-secret=42"),
        (Format::Hermes, "mcp_servers: {private-secret: false}"),
    ] {
        std::fs::write(&file, text).unwrap();
        let error = read_config_at(&file, format).unwrap_err();
        assert!(!error.contains("private-secret"), "{error}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    }
}

#[test]
fn malformed_connection_fields_refuse_the_entire_scope_without_leaking_values() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.json");
    let malformed = [
        json!({"command": "private-secret", "args": [1]}),
        json!({"command": "private-secret", "args": null}),
        json!({"command": "private-secret", "env": {"TOKEN": 1}}),
        json!({"command": "private-secret", "enabled": "false"}),
        json!({"command": "private-secret", "disabled": null}),
        json!({"command": "private-secret", "type": "http"}),
        json!({"command": "private-secret", "url": "https://fixture.test/mcp"}),
        json!({"url": "https://fixture.test/mcp", "headers": []}),
        json!({"url": "file:///private-secret"}),
        json!({"url": "private-secret"}),
        json!({"url": "https://fixture.test/mcp", "type": "stdio"}),
        json!({"command": ["private-secret"]}),
        json!({"command": "private-secret", "type": 3}),
        json!({"command": "private-secret", "type": "unknown"}),
        json!({"command": "   "}),
        json!({"env": {"TOKEN": "private-secret"}}),
    ];
    for invalid in malformed {
        let text =
            json!({"mcpServers":{"good":{"command":"retained"},"broken":invalid}}).to_string();
        std::fs::write(&file, &text).unwrap();
        let error = read_config_at(&file, Format::Standard).unwrap_err();
        assert!(!error.contains("private-secret"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    }
}

#[test]
fn duplicate_fields_are_rejected_in_every_native_document_format() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.native");
    for (format, text) in [
        (
            Format::Standard,
            r#"{"mcpServers":{"a":{"command":"one","command":"two"}}}"#,
        ),
        (
            Format::Codex,
            "[mcp_servers.a]\ncommand='one'\ncommand='two'",
        ),
        (
            Format::Hermes,
            "mcp_servers:\n  a:\n    command: one\n    command: two\n",
        ),
    ] {
        std::fs::write(&file, text).unwrap();
        assert!(read_config_at(&file, format).is_err());
    }
}

#[test]
fn commented_bom_documents_and_empty_containers_are_read_without_rewrites() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.native");
    for (format, text) in [
        (
            Format::Standard,
            "\u{feff}{\r\n// retained\r\n\"mcpServers\": {\"a\": {\"command\": \"node\",},},\r\n}",
        ),
        (
            Format::Codex,
            "\u{feff}# retained\r\n[mcp_servers.a]\r\ncommand='node'\r\n",
        ),
        (
            Format::Hermes,
            "\u{feff}# retained\r\nmcp_servers:\r\n  a:\r\n    command: node\r\n",
        ),
    ] {
        std::fs::write(&file, text).unwrap();
        assert_eq!(
            read_config_at(&file, format).unwrap().servers["a"]["command"],
            "node"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    }
    for (format, text) in [
        (Format::Standard, r#"{"other":"keep"}"#),
        (Format::OpenCode, r#"{"mcp":{}}"#),
        (Format::Codex, "model='keep'"),
        (Format::Hermes, "other: keep"),
    ] {
        std::fs::write(&file, text).unwrap();
        assert!(read_config_at(&file, format).unwrap().servers.is_empty());
    }
}

#[test]
fn non_json_native_extensions_are_retained_but_json_exports_fail_without_null_coercion() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.native");
    for (format, text) in [
        (Format::Codex, "[mcp_servers.a]\ncommand='node'\ncustom=nan"),
        (
            Format::Grok,
            "[mcp_servers.a]\ncommand='node'\ncustom=[inf]",
        ),
        (
            Format::Hermes,
            "mcp_servers:\n  a:\n    command: node\n    custom: .nan",
        ),
        (
            Format::Hermes,
            "mcp_servers:\n  a:\n    command: node\n    custom: !fixture native-value",
        ),
        (
            Format::Hermes,
            "mcp_servers:\n  a:\n    command: node\n    custom: {1: numeric-key}",
        ),
    ] {
        std::fs::write(&file, text).unwrap();
        let entries = document::read(&file, format).unwrap();
        assert!(entries["a"].to_json().is_err());
        assert!(read_config_at(&file, format).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    }
}

#[test]
fn native_remote_shapes_and_authentication_extensions_remain_distinct() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.native");
    for (format, text, url_key, header_key) in [
        (Format::Standard, r#"{"mcpServers":{"a":{"type":"sse","url":"https://fixture.test/events","headers":{"Token":"secret"},"oauth":{"client_id":"retained"}}}}"#, "url", "headers"),
        (Format::Gemini, r#"{"mcpServers":{"a":{"httpUrl":"https://fixture.test/mcp","headers":{"Token":"secret"}}}}"#, "httpUrl", "headers"),
        (Format::Gemini, r#"{"mcpServers":{"a":{"url":"https://fixture.test/events","headers":{"Token":"secret"}}}}"#, "url", "headers"),
        (Format::OpenCode, r#"{"mcp":{"a":{"type":"remote","url":"https://fixture.test/mcp","headers":{"Token":"secret"},"enabled":false}}}"#, "url", "headers"),
        (Format::Codex, "[mcp_servers.a]\nurl='https://fixture.test/mcp'\nauth='oauth'\nbearer_token_env_var='TOKEN'\n[mcp_servers.a.http_headers]\nToken='secret'\n", "url", "http_headers"),
        (Format::Grok, "[mcp_servers.a]\nurl='https://fixture.test/mcp'\n[mcp_servers.a.headers]\nToken='secret'\n", "url", "headers"),
        (Format::Hermes, "mcp_servers:\n  a:\n    url: https://fixture.test/mcp\n    headers: {Token: secret}\n    timeout: 130\n", "url", "headers"),
    ] {
        std::fs::write(&file, text).unwrap();
        let view = read_config_at(&file, format).unwrap();
        let entry = &view.servers["a"];
        assert!(entry[url_key].as_str().unwrap().starts_with("https://"));
        assert_eq!(entry[header_key]["Token"], "secret");
        assert!(entry.get("command").is_none());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    }
}

#[test]
fn opencode_arrays_and_gemini_transport_ambiguities_are_strictly_validated() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.json");
    for entry in [
        json!({"command":[]}),
        json!({"command":[1]}),
        json!({"command":["node",false]}),
        json!({"command":"node"}),
        json!({"command":["   "]}),
        json!({"command":["node"],"environment":{"TOKEN":4}}),
    ] {
        std::fs::write(&file, json!({"mcp":{"a":entry}}).to_string()).unwrap();
        assert!(read_config_at(&file, Format::OpenCode).is_err());
    }
    for entry in [
        json!({"url":"https://fixture.test/a","httpUrl":"https://fixture.test/b"}),
        json!({"httpUrl":"https://fixture.test/a","type":"sse"}),
        json!({"url":"https://fixture.test/a","type":"http"}),
    ] {
        std::fs::write(&file, json!({"mcpServers":{"a":entry}}).to_string()).unwrap();
        assert!(read_config_at(&file, Format::Gemini).is_err());
    }
}
