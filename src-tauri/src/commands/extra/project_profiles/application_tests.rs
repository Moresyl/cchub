use super::*;
use rusqlite::Connection;

fn fixture() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    for tool in ["codex", "gemini", "pi"] {
        conn.execute(
            "INSERT INTO custom_paths(tool_id,config_dir) VALUES(?1,?2)",
            params![tool, dir.path().join(tool).to_str().unwrap()],
        )
        .unwrap();
    }
    for (id, active) in [("old-workspace", 1), ("new-workspace", 0)] {
        conn.execute(
            "INSERT INTO workspaces(id,name,is_active,created_at) VALUES(?1,?1,?2,'old')",
            params![id, active],
        )
        .unwrap();
    }
    for (id, tool, snapshot) in [
        (
            "codex-new",
            "codex",
            r#"{"auth":{"OPENAI_API_KEY":"fixture"},"config":"model='new'"}"#,
        ),
        (
            "gemini-new",
            "gemini",
            r#"{"env":{"GEMINI_API_KEY":"fixture"},"config":{"theme":"dark"}}"#,
        ),
    ] {
        conn.execute("INSERT INTO config_profiles(id,name,tool_id,config_snapshot,created_at,updated_at) VALUES(?1,?1,?2,?3,'old','old')", params![id,tool,snapshot]).unwrap();
    }
    let snapshot = ProjectProfileSnapshot {
        version: SNAPSHOT_VERSION,
        workspace_id: Some("new-workspace".into()),
        config_profile_ids: vec!["codex-new".into(), "gemini-new".into()],
    };
    conn.execute("INSERT INTO project_profiles(id,name,snapshot,created_at,updated_at) VALUES('project','Fixture',?1,'old','old')",[serde_json::to_string(&snapshot).unwrap()]).unwrap();
    (dir, conn)
}

fn assert_sql_unchanged(conn: &Connection) {
    assert_eq!(
        current_workspace_id(conn).unwrap().as_deref(),
        Some("old-workspace")
    );
    let (updated, applied): (String, Option<String>) = conn
        .query_row(
            "SELECT updated_at,last_applied_at FROM project_profiles WHERE id='project'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(updated, "old");
    assert_eq!(applied, None);
    let changed: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM config_profiles WHERE updated_at!='old'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(changed, 0);
    for table in ["activity_logs", "app_settings"] {
        let count: u32 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[test]
fn actual_project_application_returns_committed_workspace_files_and_timestamps() {
    let (dir, conn) = fixture();
    let result = apply_project_from_conn(&conn, "project").unwrap();
    assert!(result.profile.is_active);
    assert_eq!(result.applied_profile_ids, vec!["codex-new", "gemini-new"]);
    assert_eq!(
        current_workspace_id(&conn).unwrap().as_deref(),
        Some("new-workspace")
    );
    assert_eq!(
        result.profile.last_applied_at.as_ref(),
        Some(&result.profile.updated_at)
    );
    assert_ne!(result.profile.updated_at, "old");
    assert!(load_profiles(&conn).unwrap()[0].is_active);
    assert!(
        std::fs::read_to_string(dir.path().join("codex/config.toml"))
            .unwrap()
            .contains("model='new'")
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("gemini/.env")).unwrap(),
        "GEMINI_API_KEY=fixture\n"
    );
}

#[test]
fn invalid_target_or_ignored_sql_mutation_leaves_all_files_and_selection_untouched() {
    for problem in [
        "invalid-config",
        "missing-workspace",
        "ignore-workspace",
        "ignore-project",
        "invalid-other-project",
    ] {
        let (dir, conn) = fixture();
        match problem {
            "invalid-config" => {
                conn.execute(
                    "UPDATE config_profiles SET config_snapshot='broken' WHERE id='gemini-new'",
                    [],
                )
                .unwrap();
            }
            "missing-workspace" => {
                conn.execute("DELETE FROM workspaces WHERE id='new-workspace'", [])
                    .unwrap();
            }
            "ignore-workspace" => {
                conn.execute_batch("CREATE TRIGGER refuse_workspace BEFORE UPDATE OF is_active ON workspaces WHEN NEW.is_active=1 BEGIN SELECT RAISE(IGNORE); END;").unwrap();
            }
            "ignore-project" => {
                conn.execute_batch("CREATE TRIGGER refuse_project BEFORE UPDATE ON project_profiles BEGIN SELECT RAISE(IGNORE); END;").unwrap();
            }
            _ => {
                conn.execute("INSERT INTO project_profiles(id,name,snapshot,created_at,updated_at) VALUES('invalid','Invalid','broken','old','old')",[]).unwrap();
            }
        }
        assert!(
            apply_project_from_conn(&conn, "project").is_err(),
            "{problem}"
        );
        assert_sql_unchanged(&conn);
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "{problem}"
        );
    }
}

#[test]
fn actual_project_commit_failure_restores_exact_files_workspace_and_metadata() {
    let (dir, conn) = fixture();
    let config = dir.path().join("codex/config.toml");
    crate::utils::atomic_write_string(&config, "# Keep exact bytes\r\nmodel = 'old'\r\n").unwrap();
    let original = std::fs::read(&config).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE deferred_child(id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fail_commit AFTER UPDATE ON project_profiles BEGIN INSERT INTO deferred_child VALUES(77); END;").unwrap();
    assert!(apply_project_from_conn(&conn, "project")
        .unwrap_err()
        .contains("commit"));
    assert_eq!(std::fs::read(config).unwrap(), original);
    assert!(!dir.path().join("codex/auth.json").exists());
    assert!(!dir.path().join("gemini").exists());
    assert_sql_unchanged(&conn);
    let count: u32 = conn
        .query_row("SELECT COUNT(*) FROM deferred_child", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn snapshot_without_workspace_restores_none_and_retains_untouched_tool_selections() {
    let (_dir, conn) = fixture();
    conn.execute(
        "UPDATE project_profiles SET snapshot=?1 WHERE id='project'",
        [r#"{"version":1,"workspaceId":null,"configProfileIds":["codex-new","gemini-new"]}"#],
    )
    .unwrap();
    conn.execute("INSERT INTO config_profiles(id,name,tool_id,config_snapshot,created_at,updated_at) VALUES('pi-old','Pi','pi','{}','old','old')",[]).unwrap();
    conn.execute(
        "INSERT INTO app_settings VALUES('current_config_profile:pi','pi-old')",
        [],
    )
    .unwrap();
    let result = apply_project_from_conn(&conn, "project").unwrap();
    assert_eq!(current_workspace_id(&conn).unwrap(), None);
    assert!(!result.profile.is_active);
    assert!(get_active_config_profile_ids_from_conn(&conn)
        .unwrap()
        .contains(&"pi-old".into()));
}

#[test]
fn creation_and_update_return_known_committed_records_without_post_commit_list_reads() {
    let (_dir, conn) = fixture();
    conn.execute("INSERT INTO project_profiles(id,name,snapshot,created_at,updated_at) VALUES('invalid','Invalid','broken','old','old')",[]).unwrap();
    let created = create_project_from_conn(&conn, "  Current  ", Some(" Notes ".into())).unwrap();
    assert_eq!(created.name, "Current");
    assert_eq!(created.description.as_deref(), Some("Notes"));
    assert!(created.is_active);
    let updated = update_project_from_conn(&conn, &created.id, "Renamed", None, true).unwrap();
    assert_eq!(updated.created_at, created.created_at);
    assert_eq!(updated.name, "Renamed");
    assert!(updated.is_active);
    let stored: String = conn
        .query_row(
            "SELECT name FROM project_profiles WHERE id=?1",
            [&created.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, "Renamed");
}

#[test]
fn creation_and_update_commit_failure_never_report_or_leave_a_saved_record() {
    let (_dir, conn) = fixture();
    conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE deferred_child(id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fail_create AFTER INSERT ON project_profiles BEGIN INSERT INTO deferred_child VALUES(77); END; CREATE TRIGGER fail_update AFTER UPDATE ON project_profiles BEGIN INSERT INTO deferred_child VALUES(77); END;").unwrap();
    assert!(create_project_from_conn(&conn, "New", None).is_err());
    let count: u32 = conn
        .query_row("SELECT COUNT(*) FROM project_profiles", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    assert!(update_project_from_conn(&conn, "project", "Changed", None, true).is_err());
    let name: String = conn
        .query_row(
            "SELECT name FROM project_profiles WHERE id='project'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(name, "Fixture");
    assert_sql_unchanged(&conn);
}
