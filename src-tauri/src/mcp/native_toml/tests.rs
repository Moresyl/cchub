use super::*;
use std::collections::HashMap;

fn local() -> McpServerConfig {
    McpServerConfig {
        command: "node".into(),
        args: vec!["server.js".into()],
        env: HashMap::from([("TOKEN".into(), "secret".into())]),
        transport_type: Some("stdio".into()),
    }
}

#[test]
fn updates_connection_fields_without_replacing_extensions_or_comments() {
    for format in [Format::Codex, Format::Grok] {
        let source = "# document comment\r\nmodel = 'kept' # provider\r\n[mcp_servers.service]\r\ncommand = 'old' # launch comment\r\nargs = ['server.js'] # keep this exact spelling\r\ntimeout = 123 # custom timeout\r\n[mcp_servers.service.oauth]\r\nclient_id = 'private-original'\r\n[mcp_servers.service.env]\r\nTOKEN = 'old' # token comment\r\nOTHER = 'remove'\r\n[mcp_servers.other]\r\ncommand = 'unchanged'\r\n";
        let result = edit(source, "service", Some(&local()), format).unwrap();
        for expected in [
            "# document comment\r\nmodel = 'kept' # provider\r\n",
            "\"node\" # launch comment",
            "args = ['server.js'] # keep this exact spelling",
            "timeout = 123 # custom timeout",
            "client_id = 'private-original'",
            "\"secret\" # token comment",
            "[mcp_servers.other]\r\ncommand = 'unchanged'",
        ] {
            assert!(
                result.contains(expected),
                "missing {expected:?} in {result}"
            );
        }
        let value: toml::Table = toml::from_str(&result).unwrap();
        assert!(value["mcp_servers"]["service"]["env"]
            .get("OTHER")
            .is_none());
    }
}

#[test]
fn converts_transports_and_clears_only_the_managed_connection_fields() {
    for (format, header_key) in [(Format::Codex, "http_headers"), (Format::Grok, "headers")] {
        let source = "[mcp_servers.service]\ncommand = 'old'\nargs = ['old']\ntimeout = 42\nenabled = false\n[mcp_servers.service.env]\nOLD = 'discard'\n";
        let remote = McpServerConfig {
            command: "https://fixture.test/mcp".into(),
            args: vec![],
            env: HashMap::from([("Authorization".into(), "Bearer fixture".into())]),
            transport_type: Some("http".into()),
        };
        let result = edit(source, "service", Some(&remote), format).unwrap();
        let value: toml::Table = toml::from_str(&result).unwrap();
        let service = &value["mcp_servers"]["service"];
        assert_eq!(service["timeout"].as_integer(), Some(42));
        assert_eq!(service["enabled"].as_bool(), Some(false));
        for key in ["command", "args", "env"] {
            assert!(service.get(key).is_none());
        }
        assert_eq!(
            service[header_key]["Authorization"].as_str(),
            Some("Bearer fixture")
        );
        let result = edit(&result, "service", Some(&local()), format).unwrap();
        let value: toml::Table = toml::from_str(&result).unwrap();
        let service = &value["mcp_servers"]["service"];
        assert!(service.get("url").is_none());
        assert!(service.get(header_key).is_none());
        assert_eq!(service["command"].as_str(), Some("node"));
        assert_eq!(service["timeout"].as_integer(), Some(42));
    }
}

#[test]
fn supports_inline_and_dotted_tables_with_quoted_names_and_removes_only_one_entry() {
    for source in [
        "mcp_servers = { 'service.name' = { command = 'old', args = ['server.js'], env = {TOKEN='old'}, timeout=7 }, other={command='kept'} } # inline\n",
        "mcp_servers.'service.name'.command = 'old'\nmcp_servers.'service.name'.args = ['server.js']\nmcp_servers.'service.name'.env.TOKEN = 'old'\nmcp_servers.'service.name'.timeout = 7\nmcp_servers.other.command = 'kept'\n",
    ] {
        let result = edit(source, "service.name", Some(&local()), Format::Codex).unwrap();
        let value: toml::Table = toml::from_str(&result).unwrap();
        assert_eq!(value["mcp_servers"]["service.name"]["command"].as_str(), Some("node"));
        assert_eq!(value["mcp_servers"]["service.name"]["timeout"].as_integer(), Some(7));
        let removed = edit(&result, "service.name", None, Format::Codex).unwrap();
        let value: toml::Table = toml::from_str(&removed).unwrap();
        assert!(value["mcp_servers"].get("service.name").is_none());
        assert_eq!(value["mcp_servers"]["other"]["command"].as_str(), Some("kept"));
    }
}

#[test]
fn keeps_noop_bytes_and_bom_and_does_not_create_a_missing_file_on_removal() {
    let source = "\u{feff}# BOM\r\n[mcp_servers.service]\r\ncommand='node'\r\nargs=['server.js']\r\n[mcp_servers.service.env]\r\nTOKEN='secret'\r\n";
    assert_eq!(
        edit(source, "service", Some(&local()), Format::Codex).unwrap(),
        source
    );
    assert_eq!(
        edit(source, "missing", None, Format::Codex).unwrap(),
        source
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing").join("config.toml");
    update_at(&path, "missing", None, Format::Codex).unwrap();
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn rejects_corrupt_files_and_wrong_containers_without_disclosing_source_or_mutating() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    for bytes in [
        b"private-secret = 'unfinished".as_slice(),
        b"mcp_servers = 7",
        b"[mcp_servers]\nservice = 'private-secret'",
        &[255, 254, 0],
    ] {
        std::fs::write(&path, bytes).unwrap();
        for config in [Some(local()), None] {
            let error = update_at(&path, "service", config.as_ref(), Format::Codex).unwrap_err();
            assert!(!error.contains("private-secret"));
            assert!(!error.contains(path.to_str().unwrap()));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(update_at(&path, "service", Some(&local()), Format::Codex).is_err());
    assert!(path.is_dir());
}

#[test]
fn invalid_input_does_not_create_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing/config.toml");
    for name in ["", " ", "bad\nname"] {
        assert!(update_at(&path, name, Some(&local()), Format::Codex).is_err());
    }
    for (command, transport) in [
        ("", "stdio"),
        ("node", "unknown"),
        ("file:///private-secret", "http"),
    ] {
        let config = McpServerConfig {
            command: command.into(),
            transport_type: Some(transport.into()),
            ..local()
        };
        assert!(update_at(&path, "service", Some(&config), Format::Codex).is_err());
    }
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn prepared_edits_reject_external_changes_and_roll_back_on_database_failure() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.toml");
    let second = dir.path().join("second.toml");
    let original = "# original\n[mcp_servers.service]\ncommand='old'\ntimeout=99\n";
    for path in [&first, &second] {
        std::fs::write(path, original).unwrap();
    }
    let _guard = crate::json_config::write_lock().unwrap();
    let plan = prepare(&first, "service", Some(&local()), Format::Codex).unwrap();
    std::fs::write(&first, "# newer external edit\n").unwrap();
    assert!(plan.commit().is_err());
    assert_eq!(
        std::fs::read_to_string(&first).unwrap(),
        "# newer external edit\n"
    );
    std::fs::write(&first, original).unwrap();
    let mut plan = prepare(&first, "service", Some(&local()), Format::Codex).unwrap();
    plan.extend(prepare(&second, "service", Some(&local()), Format::Grok).unwrap());
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE selected (name TEXT CHECK (name = 'original')); INSERT INTO selected VALUES ('original');").unwrap();
    let tx = conn.transaction().unwrap();
    let error = plan
        .commit_then(|| {
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
    assert_eq!(std::fs::read_to_string(&first).unwrap(), original);
    assert_eq!(std::fs::read_to_string(&second).unwrap(), original);
}

#[test]
fn removes_legacy_transport_type_without_discarding_native_policy_fields() {
    for format in [Format::Codex, Format::Grok] {
        let source = "[mcp_servers.service]\ntype='stdio'\ncommand='node'\nargs=['server.js']\nstartup_timeout_sec=90\nrequired=true\nenabled=false\n[mcp_servers.service.env]\nTOKEN='secret'\n";
        let result = edit(source, "service", Some(&local()), format).unwrap();
        let value: toml::Table = toml::from_str(&result).unwrap();
        let entry = &value["mcp_servers"]["service"];
        assert!(entry.get("type").is_none());
        assert_eq!(entry["startup_timeout_sec"].as_integer(), Some(90));
        assert_eq!(entry["required"].as_bool(), Some(true));
        assert_eq!(entry["enabled"].as_bool(), Some(false));
        assert_eq!(entry["env"]["TOKEN"].as_str(), Some("secret"));
    }
}

#[test]
fn retains_unrelated_special_floats_in_noop_and_changed_documents() {
    let source = "limits = { values = [nan, inf, -inf, 42], nested = { value = nan } } # keep exact floats\n[mcp_servers.service]\ncommand='node'\nargs=['server.js']\n[mcp_servers.service.env]\nTOKEN='secret'\n";
    for format in [Format::Codex, Format::Grok] {
        assert_eq!(
            edit(source, "service", Some(&local()), format).unwrap(),
            source
        );
        let config = McpServerConfig {
            command: "updated".into(),
            ..local()
        };
        let result = edit(source, "service", Some(&config), format).unwrap();
        assert_eq!(result.lines().next(), source.lines().next());
        let value: toml::Table = toml::from_str(&result).unwrap();
        assert!(value["limits"]["values"][0].as_float().unwrap().is_nan());
        assert_eq!(value["limits"]["values"][1].as_float(), Some(f64::INFINITY));
        assert_eq!(
            value["limits"]["values"][2].as_float(),
            Some(f64::NEG_INFINITY)
        );
        assert_eq!(
            value["mcp_servers"]["service"]["command"].as_str(),
            Some("updated")
        );
    }
}
