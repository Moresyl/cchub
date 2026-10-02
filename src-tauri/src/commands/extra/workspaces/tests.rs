use super::*;

fn database() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute_batch("INSERT INTO workspaces(id,name,is_active,base_path) VALUES('old','Old',1,'C:/old'),('next','Next',0,'C:/next');").unwrap();
    conn
}

fn rows(conn: &rusqlite::Connection) -> Vec<(String, i64, String)> {
    conn.prepare("SELECT id,is_active,base_path FROM workspaces ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn missing_or_empty_targets_preserve_the_current_workspace() {
    let mut conn = database();
    let before = rows(&conn);
    for id in ["missing", "", "   ", "next' OR 1=1 --"] {
        assert!(activate_workspace(&mut conn, id).is_err());
        assert_eq!(rows(&conn), before);
    }
    assert!(conn.is_autocommit());
}

#[test]
fn switches_exactly_one_workspace_and_repeated_selection_preserves_paths() {
    let mut conn = database();
    for _ in 0..2 {
        activate_workspace(&mut conn, "next").unwrap();
        assert_eq!(
            rows(&conn),
            vec![
                ("next".into(), 1, "C:/next".into()),
                ("old".into(), 0, "C:/old".into())
            ]
        );
    }
}

#[test]
fn a_failed_activation_rolls_back_the_previous_deactivation() {
    let mut conn = database();
    conn.execute_batch("CREATE TRIGGER reject_activation BEFORE UPDATE OF is_active ON workspaces WHEN NEW.id='next' AND NEW.is_active=1 BEGIN SELECT RAISE(ABORT,'fixture activation denied'); END;").unwrap();
    let before = rows(&conn);
    assert!(activate_workspace(&mut conn, "next")
        .unwrap_err()
        .contains("fixture activation denied"));
    assert_eq!(rows(&conn), before);
    assert!(conn.is_autocommit());
}

#[test]
fn public_switch_command_uses_the_transactional_validation() {
    use std::sync::Mutex;
    use tauri::Manager;

    let app = tauri::test::mock_builder()
        .manage(DbState(Mutex::new(database())))
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    assert!(switch_workspace("missing".into(), app.state::<DbState>()).is_err());
    assert_eq!(rows(&app.state::<DbState>().0.lock().unwrap())[1].1, 1);
    switch_workspace("next".into(), app.state::<DbState>()).unwrap();
    assert_eq!(rows(&app.state::<DbState>().0.lock().unwrap())[0].1, 1);
}

#[test]
fn silently_ignored_activation_cannot_commit_an_empty_selection() {
    let mut conn = database();
    conn.execute_batch("CREATE TRIGGER ignore_activation BEFORE UPDATE OF is_active ON workspaces WHEN NEW.id='next' AND NEW.is_active=1 BEGIN SELECT RAISE(IGNORE); END;").unwrap();
    let before = rows(&conn);
    assert!(activate_workspace(&mut conn, "next")
        .unwrap_err()
        .contains("could not be activated"));
    assert_eq!(rows(&conn), before);
    assert!(conn.is_autocommit());
}
