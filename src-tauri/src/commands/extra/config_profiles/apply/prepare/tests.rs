use super::*;

fn connection(root: &std::path::Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    for tool in [
        "codex",
        "gemini",
        "grokbuild",
        "openclaw",
        "pi",
        "opencode",
        "claude",
    ] {
        conn.execute(
            "INSERT INTO custom_paths(tool_id,config_dir,mcp_config_path) VALUES(?1,?2,?3)",
            rusqlite::params![
                tool,
                root.join(tool).to_str().unwrap(),
                (tool == "claude").then(|| root
                    .join("claude-global.json")
                    .to_string_lossy()
                    .into_owned())
            ],
        )
        .unwrap();
    }
    crate::hermes::write_root_override(&conn, Some(root.join("hermes").to_str().unwrap())).unwrap();
    conn
}

#[test]
fn invalid_envelopes_and_native_formats_never_create_directories() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    for (tool, snapshot) in [
        ("codex", r#"{"auth":{},"config":"secret-sentinel = ["}"#),
        ("codex", r#"{"auth":null,"config":"model='new'"}"#),
        ("codex", "{broken}"),
        ("codex", "model = ["),
        ("gemini", r#"{"env":{"KEY":1},"config":{}}"#),
        ("gemini", r#"{"env":{"BAD=KEY":"value"},"config":{}}"#),
        (
            "gemini",
            r#"{"env":{"KEY":"one\nSECOND=injected"},"config":{}}"#,
        ),
        ("gemini", r#"{"env":{},"config":[]}"#),
        ("grokbuild", r#"{"config":"model=["}"#),
        ("pi", "[]"),
        ("openclaw", "null"),
        ("hermes", r#"{"config":{"model":[]},"env":{}}"#),
        ("hermes", r#"{"config":{"model":{"default":1}},"env":{}}"#),
        ("hermes", r#"{"config":{},"env":{"KEY":1}}"#),
        ("mcode", "{}"),
        ("unknown", "{}"),
    ] {
        let error = prepare_tool_snapshot(&conn, tool, snapshot, false)
            .err()
            .unwrap();
        assert!(!error.contains("secret-sentinel"), "{tool}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "{tool}");
    }
}

#[test]
fn a_late_invalid_pair_target_leaves_the_first_file_untouched() {
    for tool in ["codex", "gemini", "hermes"] {
        let dir = tempfile::tempdir().unwrap();
        let conn = connection(dir.path());
        let root = dir.path().join(tool);
        std::fs::create_dir(&root).unwrap();
        let (first, second, snapshot) = match tool {
            "codex" => (
                "auth.json",
                "config.toml",
                r#"{"auth":{"OPENAI_API_KEY":"new"},"config":"model='new'"}"#,
            ),
            "gemini" => (
                ".env",
                "settings.json",
                r#"{"env":{"KEY":"new"},"config":{}}"#,
            ),
            _ => (
                "config.yaml",
                ".env",
                r#"{"config":{"model":{"provider":"openai","default":"new"}},"env":{"OPENAI_API_KEY":"new"}}"#,
            ),
        };
        let bytes = if tool == "hermes" {
            b"model: {provider: old}\n".as_slice()
        } else {
            b"original".as_slice()
        };
        std::fs::write(root.join(first), bytes).unwrap();
        std::fs::create_dir(root.join(second)).unwrap();
        assert!(apply_tool_snapshot_with_options(&conn, tool, snapshot, false).is_err());
        assert_eq!(std::fs::read(root.join(first)).unwrap(), bytes);
        assert!(!std::fs::read_dir(&root).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("backup")));
    }
}

#[test]
fn preparing_every_supported_tool_is_read_only_and_group_finalization_can_roll_back_all() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    let snapshots = [
        ("claude", r#"{"env":{"ANTHROPIC_API_KEY":"fixture"}}"#),
        (
            "codex",
            r#"{"auth":{"OPENAI_API_KEY":"fixture"},"config":"model='fixture'"}"#,
        ),
        (
            "gemini",
            r#"{"env":{"GEMINI_API_KEY":"fixture"},"config":{"theme":"dark"}}"#,
        ),
        ("grokbuild", r#"{"config":"[models]\ndefault='fixture'"}"#),
        (
            "opencode",
            r#"{"options":{"apiKey":"fixture"},"models":{"fixture":{}}}"#,
        ),
        ("openclaw", r#"{"agents":{}}"#),
        ("pi", r#"{"providers":{}}"#),
        (
            "hermes",
            r#"{"config":{"model":{"provider":"openai","default":"fixture"}},"env":{"OPENAI_API_KEY":"fixture"}}"#,
        ),
    ];
    let mut plan = FilePlan::default();
    for (tool, snapshot) in snapshots {
        plan.extend(prepare_tool_snapshot(&conn, tool, snapshot, false).unwrap());
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    assert!(plan
        .commit_then(|| {
            assert!(dir.path().join("codex/auth.json").is_file());
            assert!(dir.path().join("gemini/.env").is_file());
            assert!(dir.path().join("hermes/config.yaml").is_file());
            Err("fixture finalization failure".into())
        })
        .is_err());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn valid_plain_codex_toml_remains_supported_and_env_order_is_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    apply_tool_snapshot_with_options(
        &conn,
        "codex",
        "[model_providers.fixture]\nbase_url='https://fixture.test'\n",
        false,
    )
    .unwrap();
    assert!(!dir.path().join("codex/auth.json").exists());
    assert!(
        std::fs::read_to_string(dir.path().join("codex/config.toml"))
            .unwrap()
            .contains("model_providers.fixture")
    );
    let values = HashMap::from([
        ("Z".to_owned(), "last".to_owned()),
        ("A".to_owned(), "first".to_owned()),
    ]);
    assert_eq!(
        crate::hermes::env::render_env_map(&values).unwrap(),
        "A=first\nZ=last\n"
    );
}

#[test]
fn openclaw_json5_is_validated_without_reformatting_or_losing_native_syntax() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    let source = "// Keep JSON5\r\n{models: {mode: 'merge', providers: {},}, agents: {value: +.5, hex: 0x10,},}\r\n";
    apply_tool_snapshot_with_options(&conn, "openclaw", source, false).unwrap();
    let path = resolve_tool_config_path(&conn, "openclaw").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    for invalid in [
        "{agents: {key:1, 'key':2}}",
        "{private:'secret-sentinel', unfinished:",
    ] {
        let error =
            apply_tool_snapshot_with_options(&conn, "openclaw", invalid, false).unwrap_err();
        assert!(!error.contains("secret-sentinel"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
}

#[test]
fn a_failed_live_capture_keeps_the_last_successful_profile_and_project_reference() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    conn.execute("INSERT INTO config_profiles(id,name,tool_id,config_snapshot,source_type,created_at,updated_at) VALUES('live-claude','Previous','claude','{}','live','old','old')",[]).unwrap();
    let malformed = dir.path().join("claude/settings.json");
    crate::utils::atomic_write_string(&malformed, "{broken-private}").unwrap();
    sync_live_profiles(&conn, &HashMap::new(), "new").unwrap();
    let (snapshot, updated): (String, String) = conn
        .query_row(
            "SELECT config_snapshot,updated_at FROM config_profiles WHERE id='live-claude'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(snapshot, "{}");
    assert_eq!(updated, "old");
    assert_eq!(
        std::fs::read_to_string(malformed).unwrap(),
        "{broken-private}"
    );
}
