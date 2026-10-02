use super::*;
use crate::config_write::FilePlan;

fn connection(root: &std::path::Path) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO custom_paths(tool_id,config_dir,mcp_config_path) VALUES('hermes',?1,?2)",
        rusqlite::params![
            root.join("credentials").to_str().unwrap(),
            root.join("native/custom-model.yaml").to_str().unwrap(),
        ],
    )
    .unwrap();
    conn
}

fn write(path: &std::path::Path, bytes: impl AsRef<[u8]>) {
    crate::utils::atomic_write(path, bytes.as_ref()).unwrap();
}

fn parse(source: &str) -> Value {
    serde_yaml::from_str(source.strip_prefix('\u{feff}').unwrap_or(source)).unwrap()
}

fn snapshot() -> String {
    json!({
        "config": {
            "model": {"provider": "gemini", "base_url": "https://fixture.test", "default": "new"},
            "mcp_servers": {"ignored": {"command": "must-not-import"}}
        },
        "env": {"GEMINI_API_KEY": " fixture-new ", "REMOVE": ""},
        "metadata": {"hermesApiKeyEnv": "GEMINI_API_KEY"}
    })
    .to_string()
}

#[test]
fn switching_preserves_full_yaml_stream_policy_extensions_and_exact_backup() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    let path = super::super::config_path(&conn).unwrap();
    let env_path = super::super::env_path(&conn).unwrap();
    let source = "\u{feff}# user header\r\n%YAML 1.2\r\n---\r\nmodel:\r\n  provider: 'openrouter' # provider\r\n  base_url: 'https://old.test' # endpoint\r\n  default: 'old' # model\r\n  max_tokens: 8192 # policy\r\n  extra: !option {score: .nan, 7: keep}\r\nmcp_servers:\r\n  source:\r\n    command: 'node' # connection\r\n    enabled: false\r\n    auth: oauth\r\ncustom: !option {nan: .nan, 12: keep}\r\n...\r\n# user footer\r\n";
    write(&path, source);
    write(
        &env_path,
        "# secrets\nOPENROUTER_API_KEY=old\nKEEP=value\nREMOVE=old\n",
    );
    let (plan, backup) = prepare_snapshot(&conn, &snapshot(), true).unwrap();
    let backup = backup.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert!(!backup.exists());
    plan.commit().unwrap();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), source);
    let output = std::fs::read_to_string(&path).unwrap();
    let before = parse(source);
    let next = parse(&output);
    assert_eq!(next["model"]["provider"], "gemini");
    assert_eq!(next["model"]["base_url"], "https://fixture.test");
    assert_eq!(next["model"]["default"], "new");
    for field in ["max_tokens", "extra"] {
        assert!(crate::yaml_config::same(
            &next["model"][field],
            &before["model"][field]
        ));
    }
    for field in ["mcp_servers", "custom"] {
        assert!(crate::yaml_config::same(&next[field], &before[field]));
    }
    for comment in [
        "# user header",
        "%YAML 1.2",
        "# provider",
        "# endpoint",
        "# model",
        "# policy",
        "# connection",
        "...\r\n# user footer",
    ] {
        assert!(output.contains(comment), "missing {comment}");
    }
    assert!(output.starts_with('\u{feff}'));
    assert!(!output.replace("\r\n", "").contains('\n'));
    assert_eq!(
        std::fs::read_to_string(env_path).unwrap(),
        "GEMINI_API_KEY=fixture-new\nKEEP=value\n"
    );
}

#[test]
fn semantic_noops_guard_existing_bytes_and_do_not_create_missing_files_or_backups() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    let path = super::super::config_path(&conn).unwrap();
    let env_path = super::super::env_path(&conn).unwrap();
    let empty = r#"{"config":{},"env":{}}"#;
    let (plan, backup) = prepare_snapshot(&conn, empty, true).unwrap();
    assert!(plan.updates.is_empty());
    assert_eq!(plan.guards.len(), 2);
    assert!(backup.is_none());
    plan.commit().unwrap();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);

    let source = "# keep\nmodel: {provider: gemini, default: new}\ncustom: !flag .nan\n";
    let env = "# keep order and comments\r\nZ=last\r\n GEMINI_API_KEY = same \r\nA=first\r\n";
    write(&path, source);
    write(&env_path, env);
    let matching = r#"{"config":{"model":{"provider":"gemini","default":"new"}},"env":{"GEMINI_API_KEY":"same"}}"#;
    let (plan, backup) = prepare_snapshot(&conn, matching, true).unwrap();
    assert!(plan.updates.is_empty());
    assert!(backup.is_none());
    plan.commit().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(std::fs::read_to_string(&env_path).unwrap(), env);
    for changed in [&path, &env_path] {
        let (plan, _) = prepare_snapshot(&conn, matching, true).unwrap();
        let original = std::fs::read(changed).unwrap();
        write(changed, "# external newer edit\n");
        assert!(plan.commit().unwrap_err().contains("externally"));
        assert_eq!(
            std::fs::read_to_string(changed).unwrap(),
            "# external newer edit\n"
        );
        write(changed, original);
    }
}

#[test]
fn inherited_model_options_survive_switching_and_matching_inheritance_remains_a_noop() {
    for source in [
        "# inheritance\ndefaults: &defaults\n  model: {provider: gemini, default: old, max_tokens: 8192, extra: !option .nan}\n<<: *defaults # root policy\n",
        "# inheritance\nmodel_defaults: &defaults {provider: gemini, default: old, max_tokens: 8192, extra: !option .nan}\nmodel:\n  <<: *defaults # model policy\n",
    ] {
        let matching = serde_yaml::from_str("model: {provider: gemini, default: old}\n").unwrap();
        let output = edit_snapshot_config(source, &matching).unwrap();
        assert_eq!(output, source);
        let mut effective_before = parse(source);
        effective_before.apply_merge().unwrap();
        let mut effective_matching = parse(&output);
        effective_matching.apply_merge().unwrap();
        assert!(crate::yaml_config::same(&effective_before, &effective_matching));
        let incoming = serde_yaml::from_str("model: {provider: openrouter, default: new}\n").unwrap();
        let output = edit_snapshot_config(source, &incoming).unwrap();
        let mut effective_after = parse(&output);
        effective_after.apply_merge().unwrap();
        assert_eq!(effective_after["model"]["provider"], "openrouter");
        assert_eq!(effective_after["model"]["default"], "new");
        for field in ["max_tokens", "extra"] {
            assert!(crate::yaml_config::same(&effective_before["model"][field], &effective_after["model"][field]));
        }
        assert!(output.contains("# inheritance"));
        assert!(output.contains("<<: *defaults #"));
        assert!(crate::yaml_config::same(&parse(source)["defaults"], &parse(&output)["defaults"]));
    }
    assert_eq!(
        edit_snapshot_config("{}\n", &serde_yaml::from_str("model: {}\n").unwrap()).unwrap(),
        "{}\n"
    );
}

#[test]
fn env_only_changes_leave_yaml_exact_and_do_not_back_up_an_unchanged_config() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    let path = super::super::config_path(&conn).unwrap();
    let env_path = super::super::env_path(&conn).unwrap();
    let source = "# custom config\nmodel: {provider: gemini, default: current}\n";
    write(&path, source);
    let (plan, backup) = prepare_snapshot(
        &conn,
        r#"{"config":{},"env":{"GEMINI_API_KEY":"new"}}"#,
        true,
    )
    .unwrap();
    assert!(backup.is_none());
    assert_eq!(plan.updates.len(), 1);
    assert_eq!(plan.updates[0].path, env_path);
    plan.commit().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
    assert_eq!(
        std::fs::read_to_string(env_path).unwrap(),
        "GEMINI_API_KEY=new\n"
    );
}

#[test]
fn malformed_documents_and_alias_side_effects_fail_before_any_write() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection(dir.path());
    let path = super::super::config_path(&conn).unwrap();
    let env_path = super::super::env_path(&conn).unwrap();
    let env = "GEMINI_API_KEY=old\n";
    write(&env_path, env);
    for source in [
        b"private: secret-sentinel\nmodel: [".as_slice(),
        b"[]\n",
        b"model: {default: first, default: second}\n",
        b"model: old\n",
        b"---\nmodel: {}\n---\nprivate: secret-sentinel\n",
        b"model:\n  provider: &provider old\ncustom: *provider\n",
        &[0xff, 0xfe],
    ] {
        write(&path, source);
        let error = prepare_snapshot(&conn, &snapshot(), true).err().unwrap();
        assert!(!error.contains("secret-sentinel"));
        assert_eq!(std::fs::read(&path).unwrap(), source);
        assert_eq!(std::fs::read_to_string(&env_path).unwrap(), env);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }
    write(&path, "model: {}\n");
    write(&env_path, [0xff]);
    assert!(prepare_snapshot(&conn, &snapshot(), true)
        .err()
        .unwrap()
        .contains("UTF-8"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "model: {}\n");
    assert_eq!(std::fs::read(&env_path).unwrap(), [0xff]);
}

#[test]
fn profile_and_mcp_changes_compose_into_one_file_and_rollback_with_catalog_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = connection(dir.path());
    let path = super::super::config_path(&conn).unwrap();
    let env_path = super::super::env_path(&conn).unwrap();
    let source = "# profile and connections\nmodel: {provider: openrouter, default: old} # model\nmcp_servers:\n  service:\n    command: old # command\n    enabled: false # policy\n";
    let original_env = "OPENROUTER_API_KEY=old\n";
    write(&path, source);
    write(&env_path, original_env);
    conn.execute_batch("CREATE TABLE acceptance(value TEXT CHECK(value='allowed'));")
        .unwrap();
    let transaction = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let (mut plan, backup) = prepare_snapshot(&transaction, &snapshot(), true).unwrap();
    let connection = crate::mcp::config::McpServerConfig {
        command: "https://fixture.test/mcp".into(),
        args: vec![],
        env: HashMap::new(),
        transport_type: Some("sse".into()),
    };
    let target = plan
        .updates
        .iter_mut()
        .find(|update| update.path == path)
        .unwrap();
    target.desired = crate::mcp::native_yaml::edit_text(
        std::str::from_utf8(&target.desired).unwrap(),
        &[crate::mcp::native_yaml::Edit {
            name: "service",
            config: Some(&connection),
        }],
    )
    .unwrap()
    .into_bytes();
    assert_eq!(
        plan.updates
            .iter()
            .filter(|update| update.path == path)
            .count(),
        1
    );
    assert!(plan
        .commit_then(|| {
            let current = parse(&std::fs::read_to_string(&path).unwrap());
            assert_eq!(current["model"]["default"], "new");
            assert_eq!(current["mcp_servers"]["service"]["transport"], "sse");
            assert_eq!(current["mcp_servers"]["service"]["enabled"], false);
            transaction
                .execute("INSERT INTO acceptance VALUES('rejected')", [])
                .map_err(|_| "catalog finalization failed".to_string())?;
            transaction
                .commit()
                .map_err(|_| "catalog commit failed".into())
        })
        .unwrap_err()
        .contains("catalog finalization"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(std::fs::read_to_string(&env_path).unwrap(), original_env);
    assert!(!backup.unwrap().exists());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM acceptance", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );

    let (plan, backup) = prepare_snapshot(&conn, &snapshot(), true).unwrap();
    let backup = backup.unwrap();
    let external = "# newer credentials\nGEMINI_API_KEY=external\n";
    write(&env_path, external);
    assert!(plan.commit().unwrap_err().contains("externally"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(std::fs::read_to_string(&env_path).unwrap(), external);
    assert!(!backup.exists());
    let mut plan = FilePlan::default();
    plan.extend(prepare_snapshot(&conn, &snapshot(), true).unwrap().0);
    plan.commit().unwrap();
    assert_eq!(
        parse(&std::fs::read_to_string(path).unwrap())["model"]["default"],
        "new"
    );
}
