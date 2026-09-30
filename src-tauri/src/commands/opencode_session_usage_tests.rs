use super::*;
use serde_json::{json, Value};

fn fixture(
    v1: bool,
    v2: bool,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    Connection,
    Connection,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opencode.db");
    let source = Connection::open(&path).unwrap();
    for (sessions, messages, enabled, extra) in [
        ("session", "message", v1, ""),
        (
            "session_v2",
            "session_message",
            v2,
            ", type TEXT, seq INTEGER",
        ),
    ] {
        if !enabled {
            continue;
        }
        source.execute_batch(&format!("CREATE TABLE {sessions} (id TEXT PRIMARY KEY, title TEXT, directory TEXT, time_created INTEGER, time_updated INTEGER);
            CREATE TABLE {messages} (id TEXT PRIMARY KEY, session_id TEXT, data TEXT, time_created INTEGER, time_updated INTEGER {extra});
            INSERT INTO {sessions} VALUES ('current', 'Current', '/work', 1000, 2000);")).unwrap();
    }
    let target = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&target).unwrap();
    (directory, path, source, target)
}

fn completed() -> Value {
    json!({"role":"assistant", "modelID":"model-a", "providerID":"provider-a",
        "tokens":{"input":100, "output":20, "reasoning":7, "cache":{"read":50, "write":5}},
        "time":{"created":1000,"completed":2000}, "cost":0.125,
        "content":[{"type":"text","text":"SECRET-PROMPT-AND-RESPONSE"}]})
}

fn add(source: &Connection, v2: bool, id: &str, kind: &str, value: &Value) {
    let table = if v2 { "session_message" } else { "message" };
    source
        .execute(
            &format!(
                "INSERT INTO {table} (id, session_id, data, time_created, time_updated)
        VALUES (?1, 'current', ?2, 1000, 2000)"
            ),
            rusqlite::params![id, value.to_string()],
        )
        .unwrap();
    if v2 {
        source
            .execute(
                "UPDATE session_message SET type = ?1, seq = rowid WHERE id = ?2",
                [kind, id],
            )
            .unwrap();
    }
}

fn rows(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

#[test]
fn native_counts_cost_and_durable_identity_are_preserved_in_both_layouts() {
    for v2 in [false, true] {
        let (_directory, path, source, mut target) = fixture(!v2, v2);
        add(&source, v2, "answer", "assistant", &completed());
        let result = sync_from_path(&mut target, &path).unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(result.updated, 0);
        let record: (i64, i64, i64, i64, String, String) = target.query_row(
            "SELECT input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, total_cost_usd, provider_name FROM proxy_request_logs",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        ).unwrap();
        assert_eq!(
            record,
            (
                100,
                27,
                50,
                5,
                "0.125000000".into(),
                "provider-a (Session)".into()
            )
        );
        let repeated = sync_from_path(&mut target, &path).unwrap();
        assert_eq!(repeated.imported, 0);
        assert_eq!(repeated.suspected_duplicates, 1);
        assert_eq!(rows(&target, "proxy_request_logs"), 1);
        target
            .execute("DELETE FROM proxy_request_logs", [])
            .unwrap();
        assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 0);
        assert_eq!(rows(&target, "session_usage_dedup"), 1);
        assert_eq!(rows(&target, "proxy_request_logs"), 0);
    }
}

#[test]
fn completion_is_retried_even_when_session_timestamps_do_not_change() {
    let (_directory, path, source, mut target) = fixture(false, true);
    let mut value = completed();
    value["time"]["completed"] = Value::Null;
    add(&source, true, "answer", "assistant", &value);
    let pending = sync_from_path(&mut target, &path).unwrap();
    assert_eq!(pending.deferred_files, 1);
    assert_eq!(pending.imported, 0);
    assert_eq!(rows(&target, "session_usage_dedup"), 0);
    source
        .execute(
            "UPDATE session_message SET data = ?1",
            [completed().to_string()],
        )
        .unwrap();
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 1);
}

#[test]
fn corrections_update_existing_logs_without_rebilling_or_resurrecting_removed_rows() {
    let (_directory, path, source, mut target) = fixture(false, true);
    add(&source, true, "answer", "assistant", &completed());
    sync_from_path(&mut target, &path).unwrap();
    let mut corrected = completed();
    corrected["tokens"]["output"] = json!(30);
    source
        .execute(
            "UPDATE session_message SET data = ?1",
            [corrected.to_string()],
        )
        .unwrap();
    let update = sync_from_path(&mut target, &path).unwrap();
    assert_eq!(update.updated, 1);
    assert_eq!(update.imported, 0);
    assert_eq!(rows(&target, "proxy_request_logs"), 1);
    assert_eq!(
        target
            .query_row("SELECT output_tokens FROM proxy_request_logs", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        37
    );
    target
        .execute("DELETE FROM proxy_request_logs", [])
        .unwrap();
    source
        .execute(
            "UPDATE session_message SET data = ?1",
            [completed().to_string()],
        )
        .unwrap();
    assert_eq!(sync_from_path(&mut target, &path).unwrap().updated, 0);
    assert_eq!(rows(&target, "proxy_request_logs"), 0);
}

#[test]
fn v2_models_compaction_and_failures_use_native_accounting_without_secret_payloads() {
    let (_directory, path, source, mut target) = fixture(false, true);
    let mut value = completed();
    value.as_object_mut().unwrap().remove("modelID");
    value.as_object_mut().unwrap().remove("providerID");
    value["model"] =
        json!({"id":"v2-model","providerID":"v2-provider","secret":"SECRET-PROMPT-AND-RESPONSE"});
    value["status"] = json!("failed");
    value["error"] = json!({"message":"SECRET-PROMPT-AND-RESPONSE"});
    value["time"] = json!({"created":1000});
    add(&source, true, "compression", "compaction", &value);
    add(&source, true, "not-billable", "user", &completed());
    let batch = read_usage(&source, None).unwrap();
    assert_eq!(batch.records.len(), 1);
    assert_eq!(batch.records[0].model, "v2-model");
    assert!(!serde_json::to_string(&batch.records[0])
        .unwrap()
        .contains("SECRET"));
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 1);
    let status: (i64, String) = target
        .query_row(
            "SELECT status_code, error_message FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, (500, "Native request failed".into()));
    assert!(!target
        .query_row("SELECT semantic_id FROM session_usage_dedup", [], |row| row
            .get::<_, String>(0))
        .unwrap()
        .contains("SECRET"));
}

#[test]
fn unknown_or_zero_usage_is_skipped_and_model_pricing_is_used_only_without_reported_cost() {
    let (_directory, path, source, mut target) = fixture(true, false);
    let mut value = completed();
    value["cost"] = json!(0);
    add(&source, false, "estimated", "assistant", &value);
    value["tokens"] = json!({"input":0,"output":0});
    add(&source, false, "zero", "assistant", &value);
    source
        .execute(
            "INSERT INTO message VALUES ('broken','current','invalid json',1000,2000)",
            [],
        )
        .unwrap();
    target
        .execute(
            "INSERT INTO model_pricing VALUES ('model-a','model-a','2','4','0.5','3','','')",
            [],
        )
        .unwrap();
    let result = sync_from_path(&mut target, &path).unwrap();
    assert_eq!(result.imported, 1);
    assert_eq!(result.skipped, 1);
    let cost: String = target
        .query_row("SELECT total_cost_usd FROM proxy_request_logs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!((cost.parse::<f64>().unwrap() - 0.000348).abs() < 1e-10);
}

#[test]
fn ledger_and_logs_roll_back_together_and_failed_batches_remain_retryable() {
    let (_directory, path, source, mut target) = fixture(false, true);
    add(&source, true, "one", "assistant", &completed());
    add(&source, true, "two", "assistant", &completed());
    target.execute_batch("CREATE TRIGGER stop_ledger BEFORE INSERT ON session_usage_dedup
        WHEN (SELECT count(*) FROM session_usage_dedup) = 1 BEGIN SELECT RAISE(ABORT, 'write failed'); END;").unwrap();
    let result = sync_from_path(&mut target, &path).unwrap();
    assert_eq!(result.imported, 0);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(rows(&target, "proxy_request_logs"), 0);
    assert_eq!(rows(&target, "session_usage_dedup"), 0);
    target.execute_batch("DROP TRIGGER stop_ledger").unwrap();
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 2);
}

#[test]
fn mixed_migration_imports_only_current_and_independent_post_migration_sessions() {
    let (_directory, path, source, mut target) = fixture(true, true);
    add(&source, false, "legacy-copy", "assistant", &completed());
    add(&source, true, "current-answer", "assistant", &completed());
    source
        .execute_batch(
            "CREATE TABLE kv (key TEXT PRIMARY KEY, time_created INTEGER, time_updated INTEGER);
        INSERT INTO kv VALUES ('migration.v1-v2', 1500, 1500);
        INSERT INTO session VALUES ('deleted-in-v2','Deleted','/work',1000,1000);
        INSERT INTO session VALUES ('independent','New','/work',1600,1800);",
        )
        .unwrap();
    for session in ["deleted-in-v2", "independent"] {
        source
            .execute(
                "INSERT INTO message VALUES (?1, ?1, ?2, 1600, 1800)",
                rusqlite::params![session, completed().to_string()],
            )
            .unwrap();
    }
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 2);
    source.execute("DELETE FROM kv", []).unwrap();
    assert_eq!(read_usage(&source, None).unwrap().records.len(), 1);
}

#[test]
fn migration_and_copied_paths_preserve_one_stable_identity() {
    let (directory, path, source, mut target) = fixture(true, true);
    add(&source, false, "same-answer", "assistant", &completed());
    source
        .execute_batch("DROP TABLE session_message; DROP TABLE session_v2;")
        .unwrap();
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 1);
    source
        .execute_batch(
            "CREATE TABLE session_v2 AS SELECT * FROM session;
        CREATE TABLE session_message AS SELECT *, 'assistant' AS type, rowid AS seq FROM message;",
        )
        .unwrap();
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 0);
    drop(source);
    let copied = directory.path().join("moved.db");
    std::fs::copy(&path, &copied).unwrap();
    assert_eq!(sync_from_path(&mut target, &copied).unwrap().imported, 0);
    assert_eq!(rows(&target, "proxy_request_logs"), 1);
}

#[test]
fn wal_commits_are_visible_and_source_database_is_never_written_or_created() {
    let (directory, path, source, mut target) = fixture(false, true);
    source
        .execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;")
        .unwrap();
    add(&source, true, "first", "assistant", &completed());
    let before = std::fs::read(&path).unwrap();
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 1);
    add(&source, true, "second", "assistant", &completed());
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 1);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let missing = directory.path().join("missing.db");
    assert_eq!(
        sync_from_path(&mut target, &missing).unwrap().files_scanned,
        0
    );
    assert!(!missing.exists());
}

#[test]
fn session_list_totals_include_cache_reasoning_and_hidden_compaction_usage() {
    let (_directory, path, source, _target) = fixture(false, true);
    add(&source, true, "answer", "assistant", &completed());
    let mut compression = completed();
    compression["status"] = json!("completed");
    add(&source, true, "compression", "compaction", &compression);
    let sessions = crate::commands::extra_commands::scan_opencode_sessions(&path, "").unwrap();
    assert_eq!(sessions[0].input_tokens, Some(310));
    assert_eq!(sessions[0].output_tokens, Some(54));
    assert_eq!(sessions[0].tokens_used, Some(364));
    assert_eq!(sessions[0].message_count, 1);
}

#[test]
fn invalid_pricing_does_not_record_imported_identity() {
    let (_directory, path, source, mut target) = fixture(false, true);
    let mut value = completed();
    value["cost"] = Value::Null;
    add(&source, true, "answer", "assistant", &value);
    target
        .execute(
            "INSERT INTO model_pricing VALUES ('model-a','model-a','NaN','1','0','0','','')",
            [],
        )
        .unwrap();
    assert_eq!(sync_from_path(&mut target, &path).unwrap().errors.len(), 1);
    assert_eq!(rows(&target, "session_usage_dedup"), 0);
    target
        .execute("UPDATE model_pricing SET input_cost_per_million = '1'", [])
        .unwrap();
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 1);
}

#[test]
fn corrupt_databases_report_an_error_instead_of_claiming_successful_empty_import() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("corrupt.db");
    std::fs::write(&path, "not a SQLite database").unwrap();
    let mut target = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&target).unwrap();
    assert!(sync_from_path(&mut target, &path).is_err());
    assert_eq!(rows(&target, "session_usage_dedup"), 0);
}

#[test]
fn restored_backup_retains_durable_identities_even_when_usage_logs_were_cleaned() {
    let (_directory, path, source, mut target) = fixture(false, true);
    add(&source, true, "answer", "assistant", &completed());
    assert_eq!(sync_from_path(&mut target, &path).unwrap().imported, 1);
    target
        .execute("DELETE FROM proxy_request_logs", [])
        .unwrap();
    let mut backup = crate::db::schema::get_schema_sql();
    crate::commands::extra_commands::append_backup_database_rows(&target, &mut backup);
    let mut restored = Connection::open_in_memory().unwrap();
    restored.execute_batch(&backup).unwrap();
    assert_eq!(rows(&restored, "session_usage_dedup"), 1);
    assert_eq!(sync_from_path(&mut restored, &path).unwrap().imported, 0);
    assert_eq!(rows(&restored, "proxy_request_logs"), 0);
}
