use super::*;
use crate::commands::extra::statusline::backup_sql::BACKUP_TABLE_SCHEMA;

fn live(dir: &Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(dir.join("live.db")).unwrap();
    configure_database_connection(&conn, false).unwrap();
    conn.execute(
        "INSERT INTO app_settings VALUES ('sentinel','original')",
        [],
    )
    .unwrap();
    conn
}

fn dump(body: &str) -> String {
    format!("-- CCHub Database Backup (.sql)\n{}\n{}\nINSERT INTO _backup_meta VALUES ('version','1.6.10');\n{body}", crate::db::schema::get_schema_sql(), BACKUP_TABLE_SCHEMA)
}

fn path_row(tool: &str, path: &Path) -> String {
    format!(
        "INSERT INTO custom_paths (tool_id,config_dir) VALUES ('{tool}','{}');\n",
        path.to_string_lossy().replace('\'', "''")
    )
}

#[test]
fn recovery_waits_for_a_configuration_writer_and_preserves_its_later_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let edited = dir.path().join("edited");
    std::fs::write(&first, [255, 0, 1]).unwrap();
    std::fs::write(&edited, "original").unwrap();
    let guard = crate::json_config::write_lock().unwrap();
    let mut files = super::super::backup_file_rollback::FileRollback::new(dir.path()).unwrap();
    for target in [&first, &edited] {
        files.capture(target).unwrap();
        files.write(target, b"restored").unwrap();
    }
    let (started, ready) = std::sync::mpsc::channel();
    let (finished, completed) = std::sync::mpsc::channel();
    let recovery = std::thread::spawn(move || {
        started.send(()).unwrap();
        let error = rollback_restored_files(&mut files, "failed".into());
        finished.send(()).unwrap();
        error
    });
    ready.recv().unwrap();
    let waited = matches!(
        completed.recv_timeout(std::time::Duration::from_millis(100)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    );
    let written = crate::utils::atomic_write(&edited, b"new application edit");
    drop(guard);
    written.unwrap();
    let error = recovery.join().unwrap();
    assert!(
        waited,
        "recovery must wait for the active configuration writer"
    );
    assert!(error.contains("外部修改已保留"));
    assert_eq!(std::fs::read(&first).unwrap(), [255, 0, 1]);
    assert_eq!(std::fs::read(&edited).unwrap(), b"new application edit");
}

#[test]
fn full_file_and_skill_records_override_their_snapshots_and_commit_exact_bytes() {
    use base64::Engine;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let tool = dir.path().join("codex");
    std::fs::create_dir(&tool).unwrap();
    let raw = b"# retained exact formatting\r\nmodel = 'full-file'\r\n";
    let full = base64::engine::general_purpose::STANDARD.encode(raw);
    let skill = base64::engine::general_purpose::STANDARD.encode("full skill\r\n");
    let snapshot = serde_json::json!({"auth":{}, "config":"model = 'snapshot'"}).to_string();
    let body = format!("{}INSERT INTO _tool_configs VALUES ('codex','','{}'); INSERT INTO _skill_files (tool_id,name,content) VALUES ('codex','SKILL.md','snapshot skill'); INSERT INTO _backup_files (root_key,relative_path,content_base64) VALUES ('tooldir:codex','config.toml','{full}'),('tooldir:codex','skills/SKILL.md','{skill}');", path_row("codex", &tool), snapshot.replace('\'', "''"));
    import_into_connection(&mut conn, &dump(&body)).unwrap();
    assert_eq!(std::fs::read(tool.join("config.toml")).unwrap(), raw);
    assert_eq!(
        std::fs::read(tool.join("skills/SKILL.md")).unwrap(),
        b"full skill\r\n"
    );
    let summary: LastImportSummary = get_json_app_setting(&conn, "last_import_summary")
        .unwrap()
        .unwrap();
    assert_eq!(summary.tool_configs_restored, 1);
    assert_eq!(summary.skills_restored, 1);
    assert_eq!(summary.full_files_restored, 2);
}

#[test]
fn external_edit_after_artifacts_blocks_database_install_and_preserves_user_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let tool = dir.path().join("codex");
    std::fs::create_dir(&tool).unwrap();
    let target = tool.join("config.toml");
    std::fs::write(&target, [0, 255, 1]).unwrap();
    let snapshot = serde_json::json!({"auth":{}, "config":"model = 'restored'"}).to_string();
    let body = format!("{}INSERT INTO app_settings VALUES ('restored','new'); INSERT INTO _tool_configs VALUES ('codex','','{}');", path_row("codex", &tool), snapshot.replace('\'', "''"));
    let prepared = rusqlite::Connection::open_in_memory().unwrap();
    configure_database_connection(&prepared, false).unwrap();
    super::super::backup_sql::load_backup_sql(&prepared, &dump(&body)).unwrap();
    let mut files = super::super::backup_file_rollback::FileRollback::new(dir.path()).unwrap();
    super::super::backup_artifacts::restore_artifacts_with_rollback(
        &prepared, 0, &mut files, false,
    )
    .unwrap();
    std::fs::write(&target, "model = 'external'").unwrap();
    let error = install_restored_database(&prepared, &mut conn, &files).unwrap_err();
    assert!(files.rollback(error).contains("外部修改已保留"));
    assert_eq!(std::fs::read(&target).unwrap(), b"model = 'external'");
    assert!(!tool.join("auth.json").exists());
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='sentinel'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "original"
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM app_settings WHERE key='restored'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn successful_restore_keeps_the_connection_and_persists_summary_and_native_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let tool = dir.path().join("codex");
    std::fs::create_dir(&tool).unwrap();
    std::fs::write(tool.join("config.toml"), "model = 'original'").unwrap();
    let observer = rusqlite::Connection::open(dir.path().join("live.db")).unwrap();
    let before = get_main_db_path(&conn).unwrap();
    let snapshot =
        serde_json::json!({"auth":{"OPENAI_API_KEY":"fixture"}, "config":"model = \"restored\""})
            .to_string();
    let sql = dump(&format!("{}INSERT INTO app_settings VALUES ('restored','new'); INSERT INTO _tool_configs VALUES ('codex','','{}');", path_row("codex", &tool), snapshot.replace('\'', "''")));
    let message = import_into_connection(&mut conn, &sql).unwrap();
    assert!(message.contains("1 个工具配置"));
    assert_eq!(get_main_db_path(&conn).unwrap(), before);
    assert_eq!(
        std::fs::read_to_string(tool.join("config.toml")).unwrap(),
        "model = \"restored\""
    );
    assert!(tool.join("auth.json").exists());
    let restored: String = observer
        .query_row(
            "SELECT value FROM app_settings WHERE key='restored'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(restored, "new");
    let summary: LastImportSummary = get_json_app_setting(&conn, "last_import_summary")
        .unwrap()
        .unwrap();
    let safety = rusqlite::Connection::open(summary.safety_backup_path).unwrap();
    assert_eq!(
        safety
            .query_row(
                "SELECT value FROM app_settings WHERE key='sentinel'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "original"
    );
    assert_eq!(summary.tool_configs_restored, 1);
    assert!(conn.is_autocommit());
}

#[test]
fn an_existing_safety_backup_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let conn = live(dir.path());
    let target = dir.path().join("saved.db");
    std::fs::write(&target, b"previous safety backup").unwrap();
    assert!(create_safety_db_backup(&conn, &target).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"previous safety backup");
}

#[test]
fn cloud_restore_uses_local_tool_paths_and_keeps_current_cloud_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let local = dir.path().join("local");
    let foreign = dir.path().join("foreign");
    std::fs::create_dir(&local).unwrap();
    std::fs::create_dir(&foreign).unwrap();
    conn.execute(
        "INSERT INTO custom_paths (tool_id,config_dir) VALUES ('claude',?1)",
        [local.to_string_lossy()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO app_settings VALUES ('s3_sync_settings','current device account')",
        [],
    )
    .unwrap();
    let sql=dump(&format!("{}INSERT INTO app_settings VALUES ('s3_sync_settings','foreign account'); INSERT INTO app_settings VALUES ('common_config_snippets','remote library'); INSERT INTO _tool_configs VALUES ('claude-settings','','{{\"env\":{{}}}}');",path_row("claude",&foreign)));
    import_into_connection_with_mode(&mut conn, &sql, true).unwrap();
    let summary: LastImportSummary = get_json_app_setting(&conn, "last_import_summary")
        .unwrap()
        .unwrap();
    // Four seeded pricing records and the shared library; local account/path
    // records are retained rather than restored from the remote backup.
    assert_eq!(summary.db_rows_restored, 5);
    assert!(local.join("settings.json").exists());
    assert!(!foreign.join("settings.json").exists());
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='s3_sync_settings'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "current device account"
    );
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='common_config_snippets'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "remote library"
    );
}

#[test]
fn existing_foreign_project_paths_are_deferred_until_explicit_mapping() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let project = dir.path().join("foreign project");
    std::fs::create_dir(&project).unwrap();
    let file = project.join("AGENTS.md");
    std::fs::write(&file, "original").unwrap();
    let root = project.to_string_lossy().replace('\'', "''");
    let sql=dump(&format!("INSERT INTO _backup_files (root_key,relative_path,content_base64) VALUES ('project:{root}','AGENTS.md','cmVzdG9yZWQ=');"));
    import_into_connection_with_mode(&mut conn, &sql, true).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");
    let pending = get_pending_imported_project_roots_from_conn(&conn).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        apply_project_root_remap(&conn, &pending[0].project_root, &pending[0].project_root)
            .unwrap(),
        1
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "restored");
    assert!(get_pending_imported_project_roots_from_conn(&conn)
        .unwrap()
        .is_empty());
}

#[test]
fn legacy_raw_claude_configuration_retains_comments_and_exact_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let tool = dir.path().join("claude");
    std::fs::create_dir(&tool).unwrap();
    let path = tool.join("mcp.json");
    let raw = "// legacy note\r\n{\"mcpServers\":{}}\r\n";
    let body = format!("{}UPDATE custom_paths SET mcp_config_path='{}' WHERE tool_id='claude'; INSERT INTO _tool_configs VALUES ('claude','','{}');", path_row("claude", &tool), path.to_string_lossy().replace('\'', "''"), raw.replace('\'', "''"));
    import_into_connection(&mut conn, &dump(&body)).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), raw);
}

#[cfg(windows)]
#[test]
fn a_late_artifact_write_failure_restores_earlier_binary_and_skill_files() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let tool = dir.path().join("pi");
    std::fs::create_dir(&tool).unwrap();
    let first = tool.join("a.bin");
    let second = tool.join("b.bin");
    std::fs::write(&first, [0, 255, 1]).unwrap();
    std::fs::write(&second, b"original second").unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&second)
        .unwrap();
    let body = format!("{}INSERT INTO _skill_files (tool_id,name,content) VALUES ('pi','SKILL.md','new skill'); INSERT INTO _backup_files (root_key,relative_path,content_base64) VALUES ('tooldir:pi','a.bin','bmV3'),('tooldir:pi','b.bin','bmV3');", path_row("pi", &tool));
    let error = import_into_connection(&mut conn, &dump(&body)).unwrap_err();
    assert!(error.contains("附属文件恢复失败"));
    assert_eq!(std::fs::read(&first).unwrap(), [0, 255, 1]);
    assert_eq!(std::fs::read(&second).unwrap(), b"original second");
    assert!(!tool.join("skills").exists());
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='sentinel'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "original"
    );
    drop(held);
}

#[test]
fn invalid_later_tool_rolls_back_prior_tool_and_preserves_live_database() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let claude = dir.path().join("claude");
    let mcp = claude.join("mcp.json");
    let opencode = dir.path().join("opencode");
    std::fs::create_dir(&claude).unwrap();
    std::fs::create_dir(&opencode).unwrap();
    std::fs::write(claude.join("settings.json"), "original settings bytes").unwrap();
    let body = format!("{}{}UPDATE custom_paths SET mcp_config_path='{}' WHERE tool_id='claude'; INSERT INTO _tool_configs VALUES ('claude-settings','','{{\"env\":{{}}}}'); INSERT INTO _tool_configs VALUES ('opencode','','invalid-secret-value');", path_row("claude", &claude), path_row("opencode", &opencode), mcp.to_string_lossy().replace('\'', "''"));
    let error = import_into_connection(&mut conn, &dump(&body)).unwrap_err();
    assert!(!error.contains("invalid-secret-value"));
    assert_eq!(
        std::fs::read_to_string(claude.join("settings.json")).unwrap(),
        "original settings bytes"
    );
    assert!(!opencode.join("opencode.json").exists());
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='sentinel'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "original"
    );
    assert!(conn.is_autocommit());
}

#[test]
fn busy_sqlite_install_does_not_replace_database_and_native_changes_can_roll_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    conn.busy_timeout(std::time::Duration::from_millis(20))
        .unwrap();
    let tool = dir.path().join("codex");
    std::fs::create_dir(&tool).unwrap();
    let target = tool.join("config.toml");
    std::fs::write(&target, [0, 255, 1]).unwrap();
    let snapshot =
        serde_json::json!({"auth":{"OPENAI_API_KEY":"fixture"}, "config":"model = \"restored\""})
            .to_string();
    let sql = dump(&format!(
        "{}INSERT INTO _tool_configs VALUES ('codex','','{}');",
        path_row("codex", &tool),
        snapshot.replace('\'', "''")
    ));
    let external = rusqlite::Connection::open(dir.path().join("live.db")).unwrap();
    external
        .execute_batch(
            "BEGIN IMMEDIATE; UPDATE app_settings SET value='uncommitted' WHERE key='sentinel';",
        )
        .unwrap();
    let error = import_into_connection(&mut conn, &sql).unwrap_err();
    assert!(error.contains("占用"));
    assert_eq!(std::fs::read(&target).unwrap(), [0, 255, 1]);
    assert!(!tool.join("auth.json").exists());
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='sentinel'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "original"
    );
    external.execute_batch("ROLLBACK;").unwrap();
    assert!(conn.is_autocommit());
}

#[test]
fn grok_profile_read_apply_and_restore_use_the_configured_directory() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let tool = dir.path().join("grok");
    std::fs::create_dir(&tool).unwrap();
    let path = tool.join("config.toml");
    conn.execute(
        "INSERT INTO custom_paths (tool_id, config_dir) VALUES ('grokbuild',?1)",
        [tool.to_string_lossy()],
    )
    .unwrap();
    std::fs::write(&path, "model = \"original\"").unwrap();
    let original = read_tool_snapshot(&conn, "grokbuild").unwrap();
    let snapshot = serde_json::json!({"config":"model = \"restored\""}).to_string();
    apply_tool_snapshot(&conn, "grokbuild", &snapshot).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "model = \"restored\""
    );
    apply_tool_snapshot(&conn, "grokbuild", &original).unwrap();
    let sql = dump(&format!(
        "{}INSERT INTO _tool_configs VALUES ('grokbuild','','{}');",
        path_row("grokbuild", &tool),
        snapshot.replace('\'', "''")
    ));
    import_into_connection(&mut conn, &sql).unwrap();
    assert_eq!(
        read_tool_snapshot(&conn, "grokbuild").unwrap(),
        serde_json::to_string_pretty(&serde_json::json!({"config":"model = \"restored\""}))
            .unwrap()
    );
}

#[cfg(windows)]
#[test]
fn hermes_restore_rolls_back_without_leaving_untracked_backup_files() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let tool = dir.path().join("hermes");
    std::fs::create_dir(&tool).unwrap();
    let config = tool.join("config.yaml");
    let env = tool.join(".env");
    std::fs::write(&config, "model:\n  provider: openai\n  default: original\n").unwrap();
    std::fs::write(&env, "OPENAI_API_KEY=original\r\n").unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&env)
        .unwrap();
    let snapshot = serde_json::json!({"config":{"model":{"provider":"openai","default":"restored"}},"env":{"OPENAI_API_KEY":"fixture"}}).to_string();
    let body = format!("INSERT INTO app_settings VALUES ('hermes.rootOverride','{}'); INSERT INTO _tool_configs VALUES ('hermes','','{}');", tool.to_string_lossy().replace('\'', "''"), snapshot.replace('\'', "''"));
    assert!(import_into_connection(&mut conn, &dump(&body)).is_err());
    assert_eq!(
        std::fs::read_to_string(&config).unwrap(),
        "model:\n  provider: openai\n  default: original\n"
    );
    assert_eq!(
        std::fs::read_to_string(&env).unwrap(),
        "OPENAI_API_KEY=original\r\n"
    );
    assert_eq!(std::fs::read_dir(&tool).unwrap().count(), 2);
    drop(held);
}

#[cfg(windows)]
#[test]
fn failure_on_second_native_file_restores_first_and_keeps_database_unchanged() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut conn = live(dir.path());
    let tool = dir.path().join("gemini");
    std::fs::create_dir(&tool).unwrap();
    let settings = tool.join("settings.json");
    std::fs::write(tool.join(".env"), b"ORIGINAL=unchanged\r\n").unwrap();
    std::fs::write(&settings, b"{\"original\":true}").unwrap();
    // Permit snapshot reads while refusing ReplaceFileW's deletion access.
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&settings)
        .unwrap();
    let body = format!("{}INSERT INTO _tool_configs VALUES ('gemini','','{{\"env\":{{\"NEW\":\"changed\"}},\"config\":{{\"new\":true}}}}');", path_row("gemini", &tool));
    assert!(import_into_connection(&mut conn, &dump(&body)).is_err());
    assert_eq!(
        std::fs::read(tool.join(".env")).unwrap(),
        b"ORIGINAL=unchanged\r\n"
    );
    assert_eq!(std::fs::read(&settings).unwrap(), b"{\"original\":true}");
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='sentinel'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "original"
    );
    drop(held);
}
