use super::*;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE config_profiles (
            id TEXT PRIMARY KEY, tool_id TEXT, sort_order INTEGER,
            updated_at TEXT, created_at TEXT
        );
        INSERT INTO config_profiles VALUES
            ('a', 'claude', 0, NULL, '2026-01-01'),
            ('b', 'claude', 1, NULL, '2026-01-01'),
            ('c', 'claude', 2, NULL, '2026-01-01'),
            ('other', 'codex', 8, NULL, '2026-01-01');",
    )
    .unwrap();
    conn
}

fn order(conn: &Connection) -> Vec<(String, i64)> {
    let mut statement = conn
        .prepare("SELECT id, sort_order FROM config_profiles ORDER BY id")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn saves_the_complete_order_without_changing_other_tools() {
    let mut conn = database();
    save_order(&mut conn, "claude", &ids(&["c", "a", "b"])).unwrap();
    assert_eq!(
        order(&conn),
        vec![
            ("a".into(), 1),
            ("b".into(), 2),
            ("c".into(), 0),
            ("other".into(), 8)
        ]
    );
}

#[test]
fn appends_omitted_profiles_in_their_existing_relative_order() {
    let mut conn = database();
    save_order(&mut conn, "claude", &ids(&["c"])).unwrap();
    assert_eq!(
        order(&conn),
        vec![
            ("a".into(), 1),
            ("b".into(), 2),
            ("c".into(), 0),
            ("other".into(), 8)
        ]
    );
}

#[test]
fn keeps_a_profile_added_since_the_client_loaded_the_list() {
    let mut conn = database();
    conn.execute(
        "INSERT INTO config_profiles VALUES ('new', 'claude', 3, NULL, NULL)",
        [],
    )
    .unwrap();
    save_order(&mut conn, "claude", &ids(&["c", "b", "a"])).unwrap();
    assert_eq!(
        order(&conn)
            .into_iter()
            .find(|(id, _)| id == "new")
            .unwrap()
            .1,
        3
    );
}

#[test]
fn rejects_invalid_orders_without_applying_even_the_first_move() {
    for input in [&["c", "missing"][..], &["c", "other"], &["c", "a", "c"]] {
        let mut conn = database();
        let before = order(&conn);
        assert!(save_order(&mut conn, "claude", &ids(input)).is_err());
        assert_eq!(order(&conn), before);
    }
}

#[test]
fn rolls_back_when_a_database_write_fails_partway_through() {
    let mut conn = database();
    conn.execute_batch(
        "CREATE TRIGGER fail_reorder BEFORE UPDATE ON config_profiles
        WHEN NEW.id = 'a' BEGIN SELECT RAISE(ABORT, 'write failed'); END;",
    )
    .unwrap();
    let before = order(&conn);
    assert!(save_order(&mut conn, "claude", &ids(&["c", "a", "b"])).is_err());
    assert_eq!(order(&conn), before);
}

#[test]
fn empty_order_is_a_noop() {
    let mut conn = database();
    let before = order(&conn);
    save_order(&mut conn, "claude", &[]).unwrap();
    assert_eq!(order(&conn), before);
}
