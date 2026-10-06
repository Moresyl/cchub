use super::*;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::Manager;

#[test]
fn native_config_commands_validate_every_member_before_mutating_database() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(DbState(Mutex::new(conn)));
    let snapshot = json!({"providers":{"local":{"settings":{"apiKey":"fixture"},"models":{"m":{"variants":[{"id":"fast","settings":{"effort":"low"}}]}}}},"model":"local/m#fast"});
    let id = save_config_profile(
        "Original".into(),
        "opencode".into(),
        snapshot.to_string(),
        app.state::<DbState>(),
    )
    .unwrap();
    let invalid = json!({"providers":{"local":{"settings":{"apiKey":"secret-sentinel"},"models":{"m":{"variants":{}}}}},"model":"local/m"}).to_string();
    assert!(save_config_profile(
        "Invalid".into(),
        "opencode".into(),
        invalid.clone(),
        app.state::<DbState>()
    )
    .is_err());
    assert!(update_config_profile(
        id.clone(),
        "Invalid".into(),
        invalid.clone(),
        app.state::<DbState>()
    )
    .is_err());
    let inputs = vec![
        SharedConfigProfileInput {
            tool_id: "claude".into(),
            config_snapshot: json!({"env":{}}).to_string(),
        },
        SharedConfigProfileInput {
            tool_id: "opencode".into(),
            config_snapshot: invalid,
        },
    ];
    let error =
        save_shared_config_profiles("Invalid".into(), inputs, None, None, app.state::<DbState>())
            .unwrap_err();
    assert!(error.contains("variants"));
    assert!(!error.contains("secret-sentinel"));
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM config_profiles", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let (name, stored): (String, String) = conn
        .query_row(
            "SELECT name, config_snapshot FROM config_profiles WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(name, "Original");
    let stored: Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["settings"]["apiKey"], "fixture");
    assert_eq!(stored["metadata"]["nativeFormat"], "providers");
    assert_eq!(
        stored["models"]["m"]["variants"],
        json!([{"id":"fast","settings":{"effort":"low"}}])
    );
}
