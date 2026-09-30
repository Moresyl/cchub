use super::*;

fn fixture(v1: bool, v2: bool) -> (tempfile::TempDir, std::path::PathBuf, Connection) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.db");
    let conn = Connection::open(&path).unwrap();
    if v1 {
        conn.execute_batch("CREATE TABLE session (id TEXT PRIMARY KEY, title TEXT, directory TEXT, time_created INTEGER, time_updated INTEGER);
            CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);
            CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, time_created INTEGER, data TEXT);").unwrap();
    }
    if v2 {
        conn.execute_batch("CREATE TABLE session_v2 (id TEXT PRIMARY KEY, title TEXT, directory TEXT, time_created INTEGER, time_updated INTEGER);
            CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT REFERENCES session_v2(id), type TEXT, seq INTEGER, data TEXT, time_created INTEGER, time_updated INTEGER);").unwrap();
    }
    (directory, path, conn)
}

fn add_v2(conn: &Connection, id: &str, seq: i64, kind: &str, data: &str, created: i64) {
    conn.execute(
        "INSERT INTO session_message VALUES (?1, 'ses_current', ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![id, kind, seq, data, created],
    )
    .unwrap();
}

#[test]
fn reads_v2_by_sequence_and_searches_visible_content_without_modifying_database() {
    let (_directory, path, conn) = fixture(false, true);
    conn.execute_batch(
        "INSERT INTO session_v2 VALUES ('ses_current', NULL, '/work/项目', 1000, 2000)",
    )
    .unwrap();
    add_v2(
        &conn,
        "msg_question",
        1,
        "user",
        r#"{"text":"请检查配置"}"#,
        1000,
    );
    add_v2(
        &conn,
        "msg_answer",
        2,
        "assistant",
        r#"{"content":[{"type":"reasoning","text":"private reasoning"},{"type":"tool","name":"shell","state":{"output":"result"}},{"type":"text","text":"RETRIED answer"}]}"#,
        5000,
    );
    add_v2(
        &conn,
        "msg_followup",
        3,
        "user",
        r#"{"text":"next question"}"#,
        3000,
    );
    add_v2(
        &conn,
        "msg_compact",
        4,
        "compaction",
        r#"{"status":"completed"}"#,
        6000,
    );
    drop(conn);
    let before = std::fs::read(&path).unwrap();

    let sessions = scan_opencode_sessions(&path, " retried ").unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "项目");
    assert_eq!(sessions[0].message_count, 3);
    assert!(sessions[0].preview.contains("RETRIED"));
    assert!(sessions[0].can_resume && sessions[0].can_delete);
    let entries = load_opencode_session_entries(&path, "ses_current").unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        ["msg_question", "msg_answer", "msg_followup"]
    );
    assert_eq!(entries[1].content, "[Tool: shell]\nresult\nRETRIED answer");
    assert!(scan_opencode_sessions(&path, "private reasoning")
        .unwrap()
        .is_empty());
    assert_eq!(before, std::fs::read(&path).unwrap());
}

#[test]
fn reads_legacy_parts_and_preserves_tool_output() {
    let (_directory, path, conn) = fixture(true, false);
    conn.execute_batch(r#"INSERT INTO session VALUES ('ses_old','Legacy','/work',1000,2000);
        INSERT INTO message VALUES ('question','ses_old',1000,1000,'{"role":"user"}');
        INSERT INTO message VALUES ('answer','ses_old',2000,2000,'{"role":"assistant"}');
        INSERT INTO part VALUES ('part_1','question','ses_old',1000,'{"type":"text","text":"question text"}');
        INSERT INTO part VALUES ('part_2','answer','ses_old',2000,'{"type":"tool","tool":"read","state":{"output":"file contents"}}');
        INSERT INTO part VALUES ('part_3','answer','ses_old',2001,'{"type":"text","text":"finished"}');"#).unwrap();
    let entries = load_opencode_session_entries(&path, "ses_old").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].content, "question text");
    assert_eq!(entries[1].content, "[Tool: read]\nfile contents\nfinished");
    assert_eq!(
        scan_opencode_sessions(&path, "file contents")
            .unwrap()
            .len(),
        1
    );
    delete_opencode_session(&path, "ses_old").unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM part", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn mixed_database_keeps_post_migration_v1_without_resurrecting_deleted_sessions() {
    let (_directory, path, conn) = fixture(true, true);
    conn.execute_batch(r#"CREATE TABLE kv (key TEXT, time_created INTEGER, time_updated INTEGER);
        INSERT INTO kv VALUES ('migration.v1-v2',2000,2000);
        INSERT INTO session VALUES ('ses_deleted','Deleted','/old',1000,1500);
        INSERT INTO session VALUES ('ses_current','Legacy copy','/old',1000,2500);
        INSERT INTO session VALUES ('ses_after','After downgrade','/new',3000,4000);
        INSERT INTO session_v2 VALUES ('ses_current','Current','/new',1000,5000);
        INSERT INTO message VALUES ('legacy_answer','ses_after',3500,3500,'{"role":"user","text":"post migration"}');"#).unwrap();
    add_v2(
        &conn,
        "current_answer",
        1,
        "user",
        r#"{"text":"current transcript"}"#,
        5000,
    );
    let sessions = scan_opencode_sessions(&path, "").unwrap();
    assert_eq!(
        sessions
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        ["ses_current", "ses_after"]
    );
    assert_eq!(
        load_opencode_session_entries(&path, "ses_after").unwrap()[0].content,
        "post migration"
    );
    delete_opencode_session(&path, "ses_current").unwrap();
    assert_eq!(
        scan_opencode_sessions(&path, "").unwrap()[0].id,
        "ses_after"
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM session WHERE id = 'ses_current'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn mixed_database_without_marker_uses_v2_only_and_is_not_rewritten() {
    let (_directory, path, conn) = fixture(true, true);
    conn.execute_batch(
        "INSERT INTO session VALUES ('ses_old','Old','/work',1000,2000);
        INSERT INTO session_v2 VALUES ('ses_current','New','/work',1000,2000);",
    )
    .unwrap();
    assert_eq!(scan_opencode_sessions(&path, "").unwrap().len(), 1);
    assert_eq!(
        conn.query_row("SELECT count(*) FROM session", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn failed_delete_rolls_back_all_message_removals() {
    let (_directory, path, conn) = fixture(false, true);
    conn.execute_batch("INSERT INTO session_v2 VALUES ('ses_current','Current','/work',1000,2000);
        CREATE TRIGGER protect_session BEFORE DELETE ON session_v2 BEGIN SELECT RAISE(ABORT, 'busy'); END;").unwrap();
    add_v2(&conn, "answer", 1, "user", r#"{"text":"keep me"}"#, 1000);
    assert!(delete_opencode_session(&path, "ses_current")
        .unwrap_err()
        .contains("no changes saved"));
    assert_eq!(
        load_opencode_session_entries(&path, "ses_current").unwrap()[0].content,
        "keep me"
    );
    assert_eq!(scan_opencode_sessions(&path, "").unwrap().len(), 1);
}

#[test]
fn missing_database_is_never_created_and_unknown_id_is_not_success() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.db");
    assert!(load_opencode_session_entries(&missing, "ses_missing").is_err());
    assert!(scan_opencode_sessions(&missing, "").is_err());
    assert!(delete_opencode_session(&missing, "ses_missing").is_err());
    assert!(!missing.exists());
    let (_directory, path, _) = fixture(false, true);
    assert!(delete_opencode_session(&path, "ses_missing")
        .unwrap_err()
        .contains("no longer exists"));
}

#[test]
fn malformed_messages_are_skipped_but_valid_neighbors_remain_readable() {
    let (_directory, path, conn) = fixture(false, true);
    conn.execute_batch("INSERT INTO session_v2 VALUES ('ses_current','Current','/work',1000,2000)")
        .unwrap();
    add_v2(&conn, "bad", 1, "assistant", "{bad json", 1000);
    add_v2(&conn, "valid", 2, "user", r#"{"text":"keep this"}"#, 2000);
    assert_eq!(
        load_opencode_session_entries(&path, "ses_current").unwrap()[0].id,
        "valid"
    );
}
