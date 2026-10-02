use super::*;
use rusqlite::params;

fn fixture() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    for tool in ["codex", "gemini", "claude", "grokbuild", "pi"] {
        let root = dir.path().join(tool);
        conn.execute(
            "INSERT INTO custom_paths(tool_id,config_dir,mcp_config_path) VALUES(?1,?2,?3)",
            params![
                tool,
                root.to_str().unwrap(),
                dir.path().join("claude-global.json").to_str().unwrap()
            ],
        )
        .unwrap();
    }
    (dir, conn)
}

fn insert(conn: &Connection, id: &str, tool: &str, snapshot: &str) {
    conn.execute("INSERT INTO config_profiles(id,name,tool_id,config_snapshot,created_at,updated_at) VALUES(?1,?1,?2,?3,'old','old')",params![id,tool,snapshot]).unwrap();
}

fn count(conn: &Connection, table: &str) -> u32 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

const CODEX: &str = r#"{"auth":{"OPENAI_API_KEY":"fixture"},"config":"model='fixture'"}"#;
const GEMINI: &str = r#"{"env":{"GEMINI_API_KEY":"fixture"},"config":{"theme":"dark"}}"#;

#[test]
fn a_profile_group_commits_files_active_ids_and_activity_together() {
    let (dir, conn) = fixture();
    insert(&conn, "codex-new", "codex", CODEX);
    insert(&conn, "gemini-new", "gemini", GEMINI);
    insert(&conn, "untouched", "pi", r#"{"providers":{}}"#);
    conn.execute(
        "INSERT INTO app_settings VALUES(?1,'untouched')",
        [current_profile_setting_key("pi")],
    )
    .unwrap();
    let ids = vec!["codex-new".into(), "gemini-new".into()];
    let active =
        apply_profile_group(&conn, &ids, false, |_, _, active| Ok(active.to_vec())).unwrap();
    assert_eq!(active, vec!["codex-new", "gemini-new", "untouched"]);
    assert!(dir.path().join("codex/auth.json").is_file());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("gemini/.env")).unwrap(),
        "GEMINI_API_KEY=fixture\n"
    );
    assert_eq!(count(&conn, "activity_logs"), 2);
}

#[test]
fn invalid_later_profile_missing_profile_duplicate_tool_and_extension_stop_all_files_and_sql() {
    for problem in ["invalid", "missing", "duplicate", "extension"] {
        let (dir, conn) = fixture();
        insert(&conn, "first", "codex", CODEX);
        let second = match problem {
            "missing" => "missing",
            "duplicate" => {
                insert(&conn, "second", "codex", CODEX);
                "second"
            }
            "extension" => {
                insert(&conn, "second", "claude", r#"{"env":{"KEY":"fixture"}}"#);
                conn.execute(
                    "INSERT INTO app_settings VALUES('claude_extension_integration','true')",
                    [],
                )
                .unwrap();
                crate::utils::atomic_write_string(
                    &dir.path().join("claude/config.json"),
                    "{bad-private-sentinel}",
                )
                .unwrap();
                "second"
            }
            _ => {
                insert(&conn, "second", "gemini", r#"{"env":{},"config":null}"#);
                "second"
            }
        };
        assert!(apply_profile_group(
            &conn,
            &["first".into(), second.into()],
            false,
            |_, _, _| Ok(())
        )
        .is_err());
        assert!(!dir.path().join("codex").exists());
        assert_eq!(count(&conn, "activity_logs"), 0);
        let updated: String = conn
            .query_row(
                "SELECT updated_at FROM config_profiles WHERE id='first'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(updated, "old");
    }
}

#[test]
fn a_deferred_database_commit_failure_recovers_exact_files_and_rolls_back_selection() {
    let (dir, conn) = fixture();
    conn.execute_batch("PRAGMA foreign_keys=ON;CREATE TABLE parent(id INTEGER PRIMARY KEY);CREATE TABLE deferred_child(id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);").unwrap();
    insert(&conn, "first", "codex", CODEX);
    insert(&conn, "second", "gemini", GEMINI);
    let config = dir.path().join("codex/config.toml");
    crate::utils::atomic_write_string(&config, "model = 'old'\r\n").unwrap();
    let before = std::fs::read(&config).unwrap();
    let error = apply_profile_group(
        &conn,
        &["first".into(), "second".into()],
        false,
        |tx, _, _| {
            tx.execute("INSERT INTO deferred_child VALUES(77)", [])
                .unwrap();
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("commit"));
    assert_eq!(std::fs::read(config).unwrap(), before);
    assert!(!dir.path().join("codex/auth.json").exists());
    assert!(!dir.path().join("gemini").exists());
    assert_eq!(count(&conn, "activity_logs"), 0);
    assert_eq!(count(&conn, "deferred_child"), 0);
    assert_eq!(count(&conn, "app_settings"), 0);
}

#[test]
fn aliased_native_targets_reject_the_whole_group_and_do_not_commit_sql() {
    let (dir, conn) = fixture();
    insert(&conn, "first", "codex", CODEX);
    insert(
        &conn,
        "second",
        "grokbuild",
        r#"{"config":"[models]\ndefault='fixture'"}"#,
    );
    conn.execute(
        "UPDATE custom_paths SET config_dir=?1 WHERE tool_id='grokbuild'",
        [dir.path().join("codex").to_str().unwrap()],
    )
    .unwrap();
    assert!(apply_profile_group(
        &conn,
        &["first".into(), "second".into()],
        false,
        |_, _, _| Ok(())
    )
    .unwrap_err()
    .contains("same location"));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    assert_eq!(count(&conn, "app_settings"), 0);
    assert_eq!(count(&conn, "activity_logs"), 0);
}

#[test]
fn enabled_claude_extension_is_committed_and_rollback_preserves_its_original_format() {
    let (dir, conn) = fixture();
    insert(
        &conn,
        "claude-new",
        "claude",
        r#"{"env":{"KEY":"fixture"},"metadata":{"category":"custom"}}"#,
    );
    conn.execute(
        "INSERT INTO app_settings VALUES('claude_extension_integration','true')",
        [],
    )
    .unwrap();
    let extension = dir.path().join("claude/config.json");
    crate::utils::atomic_write_string(&extension, "{ \"theme\" : \"dark\" }\r\n").unwrap();
    let original = std::fs::read(&extension).unwrap();
    assert!(apply_profile_group(
        &conn,
        &["claude-new".into()],
        false,
        |_, _, _| Err::<(), _>("fixture".into())
    )
    .is_err());
    assert_eq!(std::fs::read(&extension).unwrap(), original);
    apply_profile_group(&conn, &["claude-new".into()], false, |_, _, _| Ok(())).unwrap();
    let text = std::fs::read_to_string(extension).unwrap();
    assert!(text.contains("\"theme\" : \"dark\""));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["primaryApiKey"], "any");
}
