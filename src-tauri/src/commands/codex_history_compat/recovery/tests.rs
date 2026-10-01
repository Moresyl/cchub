use super::*;
use std::fs::{self, OpenOptions};
use std::time::{Duration, UNIX_EPOCH};

mod ipc_tests;

fn old(path: &Path) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1700000000))
        .unwrap();
}

fn log(root: &Path, name: &str, id: &str, provider: &str) -> PathBuf {
    let path = root.join("sessions").join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let content = format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"model_provider\":\"{provider}\",\"extra\":42}}}}\r\n{{\"type\":\"response_item\",\"payload\":{{\"text\":\"原始内容\"}}}}\r\n");
    let bytes = if archive::compressed(&path) {
        zstd::stream::encode_all(content.as_bytes(), 0).unwrap()
    } else {
        content.into_bytes()
    };
    fs::write(&path, bytes).unwrap();
    old(&path);
    path
}

fn migrate(root: &Path, backups: &Path) -> String {
    let result = super::super::migration::MigrationPlan::prepare(
        root.into(),
        vec!["legacy".into(), "other".into()],
        "custom".into(),
    )
    .unwrap()
    .execute(backups, None)
    .unwrap();
    Path::new(&result.backup_path.unwrap())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .into()
}

fn plan(root: &Path, backups: &Path, key: &str) -> RestorePlan {
    RestorePlan::prepare(root.into(), backups, key.into()).unwrap()
}

fn keys(preview: &RestorePreview) -> Vec<String> {
    preview
        .items
        .iter()
        .filter(|item| item.status == "ready")
        .map(|item| item.key.clone())
        .collect()
}

#[test]
fn selective_restore_keeps_new_messages_latest_wal_titles_and_unrelated_rows() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let plain = log(root.path(), "a.jsonl", "a", "legacy");
    let packed = log(root.path(), "a.jsonl.zst", "a", "legacy");
    let conflicting = log(root.path(), "b.jsonl", "b", "other");
    let state_path = root.path().join("state.sqlite");
    let conn = rusqlite::Connection::open(&state_path).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT,title TEXT); INSERT INTO threads VALUES('a','legacy','old'),('b','other','b');").unwrap();
    let key = migrate(root.path(), backups.path());
    let appended = b"{\"type\":\"response_item\",\"payload\":{\"text\":\"new content\"}}\r\n";
    for path in [&plain, &packed] {
        let mut bytes = decode(path, &fs::read(path).unwrap(), MAX_SESSION_FILE_BYTES).unwrap();
        bytes.extend(appended);
        fs::write(
            path,
            if archive::compressed(path) {
                zstd::stream::encode_all(bytes.as_slice(), 0).unwrap()
            } else {
                bytes
            },
        )
        .unwrap();
        old(path);
    }
    let changed = String::from_utf8(fs::read(&conflicting).unwrap())
        .unwrap()
        .replace("custom", "external");
    fs::write(&conflicting, &changed).unwrap();
    old(&conflicting);
    conn.execute_batch("UPDATE threads SET title='latest WAL title' WHERE id='a'; INSERT INTO threads VALUES('new','custom','new row');").unwrap();
    let prepared = plan(root.path(), backups.path(), &key);
    let preview = prepared.preview();
    assert_eq!(preview.items.len(), 2);
    let a = preview
        .items
        .iter()
        .find(|item| item.session_id == "a")
        .unwrap();
    assert_eq!((a.status, a.log_files, a.state_rows), ("ready", 2, 1));
    assert_eq!(
        preview
            .items
            .iter()
            .find(|item| item.session_id == "b")
            .unwrap()
            .status,
        "conflict"
    );
    let before_plain = fs::read(&plain).unwrap();
    assert_eq!(
        fs::read_dir(backups.path()).unwrap().count(),
        1,
        "preview is read-only"
    );
    let restored = prepared
        .execute(backups.path(), &preview.revision, keys(&preview))
        .unwrap();
    assert_eq!(
        (restored.restored_jsonl_files, restored.restored_state_rows),
        (2, 1)
    );
    for path in [&plain, &packed] {
        let bytes = decode(path, &fs::read(path).unwrap(), MAX_SESSION_FILE_BYTES).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("\"model_provider\":\"legacy\"") && text.contains("\"extra\":42"));
        assert!(bytes.ends_with(appended));
        assert_eq!(text.matches("\r\n").count(), 3);
    }
    assert_eq!(fs::read(&conflicting).unwrap(), changed.as_bytes());
    assert_eq!(
        conn.query_row(
            "SELECT model_provider,title FROM threads WHERE id='a'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        )
        .unwrap(),
        ("legacy".into(), "latest WAL title".into())
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM threads WHERE model_provider='custom'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    let safety = Path::new(&restored.backup_path);
    assert_eq!(
        fs::read(safety.join("jsonl/sessions/a.jsonl")).unwrap(),
        before_plain
    );
    let saved = rusqlite::Connection::open(safety.join("state/state.sqlite")).unwrap();
    assert_eq!(provider(&saved, "a").unwrap().as_deref(), Some("custom"));
    assert!(safety.join("restore.json").exists());
    let again = plan(root.path(), backups.path(), &key).preview();
    assert_eq!(
        again
            .items
            .iter()
            .find(|item| item.session_id == "a")
            .unwrap()
            .status,
        "restored"
    );
    assert!(keys(&again).is_empty());
    assert_eq!(
        list(root.path(), backups.path()).unwrap().len(),
        1,
        "restore copies are not migration sources"
    );
}

#[test]
fn stale_preview_invalid_selection_and_identity_conflicts_cannot_write() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let path = log(root.path(), "a.jsonl", "a", "legacy");
    let key = migrate(root.path(), backups.path());
    old(&path);
    let preview = plan(root.path(), backups.path(), &key).preview();
    let before = fs::read(&path).unwrap();
    for selection in [
        Vec::new(),
        vec!["unknown".into()],
        vec![keys(&preview)[0].clone(); 2],
    ] {
        assert!(plan(root.path(), backups.path(), &key)
            .execute(backups.path(), &preview.revision, selection)
            .is_err());
    }
    let updated = [before.as_slice(), b"{}\n"].concat();
    fs::write(&path, &updated).unwrap();
    old(&path);
    assert!(plan(root.path(), backups.path(), &key)
        .execute(backups.path(), &preview.revision, keys(&preview))
        .unwrap_err()
        .contains("预览后"));
    assert_eq!(fs::read(&path).unwrap(), updated);
    let replaced = String::from_utf8(updated)
        .unwrap()
        .replace("\"id\":\"a\"", "\"id\":\"someone-else\"");
    fs::write(&path, replaced.as_bytes()).unwrap();
    old(&path);
    let conflict = plan(root.path(), backups.path(), &key).preview();
    assert_eq!(conflict.items[0].status, "conflict");
    assert!(conflict.items[0]
        .reason
        .as_ref()
        .unwrap()
        .contains("另一个会话"));
    assert_eq!(fs::read_dir(backups.path()).unwrap().count(), 1);
}

#[test]
fn packing_after_migration_is_supported_and_recent_writes_are_conflicts() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let plain = log(root.path(), "a.jsonl", "a", "legacy");
    let key = migrate(root.path(), backups.path());
    let packed = plain.with_extension("jsonl.zst");
    fs::write(
        &packed,
        zstd::stream::encode_all(fs::read(&plain).unwrap().as_slice(), 0).unwrap(),
    )
    .unwrap();
    fs::remove_file(&plain).unwrap();
    let active = plan(root.path(), backups.path(), &key).preview();
    assert_eq!(active.items[0].status, "conflict");
    old(&packed);
    let prepared = plan(root.path(), backups.path(), &key);
    let preview = prepared.preview();
    let result = prepared
        .execute(backups.path(), &preview.revision, keys(&preview))
        .unwrap();
    assert_eq!(result.restored_jsonl_files, 1);
    assert_eq!(
        identity(&packed, &fs::read(&packed).unwrap()).unwrap(),
        ("a".into(), "legacy".into())
    );
    assert!(!plain.exists());
}

#[test]
fn root_binding_tampered_backups_and_invalid_keys_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let path = log(root.path(), "a.jsonl", "a", "legacy");
    let key = migrate(root.path(), backups.path());
    old(&path);
    assert!(list(other.path(), backups.path()).unwrap().is_empty());
    assert!(
        RestorePlan::prepare(other.path().into(), backups.path(), key.clone())
            .err()
            .unwrap()
            .contains("不属于")
    );
    for bad in [
        "../outside",
        "codex-history-migration-not-a-uuid",
        "C:/absolute",
    ] {
        assert!(RestorePlan::prepare(root.path().into(), backups.path(), bad.into()).is_err());
    }
    fs::write(
        backups.path().join(&key).join("jsonl/sessions/a.jsonl"),
        "tampered",
    )
    .unwrap();
    assert!(
        RestorePlan::prepare(root.path().into(), backups.path(), key)
            .err()
            .unwrap()
            .contains("校验失败")
    );
    assert!(String::from_utf8(fs::read(path).unwrap())
        .unwrap()
        .contains("custom"));
}

#[test]
fn multi_file_failure_rolls_back_selected_changes_without_overwriting_external_edits() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let first = log(root.path(), "a.jsonl", "a", "legacy");
    let second = log(root.path(), "b.jsonl", "b", "other");
    let key = migrate(root.path(), backups.path());
    old(&first);
    old(&second);
    let before = fs::read(&first).unwrap();
    let second_before = fs::read(&second).unwrap();
    let prepared = plan(root.path(), backups.path(), &key);
    let preview = prepared.preview();
    let mut calls = 0;
    let error = prepared
        .execute_with(
            backups.path(),
            &preview.revision,
            keys(&preview),
            |path, bytes| {
                calls += 1;
                if calls == 2 {
                    Err("fixture failure".into())
                } else {
                    crate::utils::atomic_write(path, bytes).map_err(|error| error.to_string())
                }
            },
        )
        .unwrap_err();
    assert!(error.contains("已回滚"));
    assert_eq!(fs::read(&first).unwrap(), before);
    assert_eq!(fs::read(&second).unwrap(), second_before);
    old(&first);
    old(&second);
    let prepared = plan(root.path(), backups.path(), &key);
    let preview = prepared.preview();
    let mut written = None;
    let error = prepared
        .execute_with(
            backups.path(),
            &preview.revision,
            keys(&preview),
            |path, bytes| {
                if let Some(previous) = &written {
                    fs::write(previous, "external edit").unwrap();
                    return Err("fixture failure".into());
                }
                written = Some(path.to_path_buf());
                crate::utils::atomic_write(path, bytes).map_err(|error| error.to_string())
            },
        )
        .unwrap_err();
    assert!(error.contains("部分内容未能回滚"));
    assert_eq!(fs::read(written.unwrap()).unwrap(), b"external edit");
}

#[test]
fn state_snapshot_corruption_and_contradictory_ownership_are_not_silently_accepted() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let plain = log(root.path(), "a.jsonl", "a", "legacy");
    let packed = log(root.path(), "b.jsonl.zst", "a", "other");
    let conn = rusqlite::Connection::open(root.path().join("state_1.sqlite")).unwrap();
    conn.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT); INSERT INTO threads VALUES('a','legacy');").unwrap();
    let key = migrate(root.path(), backups.path());
    old(&plain);
    old(&packed);
    let preview = plan(root.path(), backups.path(), &key).preview();
    assert!(preview.items.iter().all(|item| item.status == "conflict"));
    let saved = backups.path().join(&key).join("state/state_1.sqlite");
    let corrupted = rusqlite::Connection::open(&saved).unwrap();
    corrupted
        .execute("UPDATE threads SET model_provider='changed'", [])
        .unwrap();
    drop(corrupted);
    assert!(
        RestorePlan::prepare(root.path().into(), backups.path(), key)
            .err()
            .unwrap()
            .contains("校验失败")
    );
}

#[test]
fn a_later_database_commit_failure_compensates_only_the_selected_provider_fields() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let path = log(root.path(), "a.jsonl", "a", "legacy");
    let first = rusqlite::Connection::open(root.path().join("state_1.sqlite")).unwrap();
    let second = rusqlite::Connection::open(root.path().join("state_2.sqlite")).unwrap();
    first.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT,title TEXT); INSERT INTO threads VALUES('a','legacy','before');").unwrap();
    second.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT); INSERT INTO threads VALUES('b','other'); CREATE TABLE parents(id INTEGER PRIMARY KEY); CREATE TABLE guard(parent INTEGER REFERENCES parents(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fail_restore AFTER UPDATE ON threads WHEN NEW.model_provider='other' BEGIN INSERT INTO guard VALUES(999); END;").unwrap();
    let key = migrate(root.path(), backups.path());
    old(&path);
    first
        .execute("UPDATE threads SET title='latest title' WHERE id='a'", [])
        .unwrap();
    let before = fs::read(&path).unwrap();
    let prepared = plan(root.path(), backups.path(), &key);
    let preview = prepared.preview();
    let error = prepared
        .execute(backups.path(), &preview.revision, keys(&preview))
        .unwrap_err();
    assert!(error.contains("无法提交") && error.contains("已回滚"));
    assert_eq!(provider(&first, "a").unwrap().as_deref(), Some("custom"));
    assert_eq!(provider(&second, "b").unwrap().as_deref(), Some("custom"));
    assert_eq!(
        first
            .query_row("SELECT title FROM threads WHERE id='a'", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "latest title"
    );
    assert_eq!(
        second
            .query_row("SELECT COUNT(*) FROM guard", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn legacy_snapshot_proof_path_validation_and_bad_catalog_entries_remain_explicit() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open(root.path().join("state_1.sqlite")).unwrap();
    conn.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT); INSERT INTO threads VALUES('a','legacy');").unwrap();
    let key = migrate(root.path(), backups.path());
    let journal_path = backups.path().join(&key).join("migration.json");
    let mut journal: serde_json::Value =
        serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    journal["states"][0]
        .as_object_mut()
        .unwrap()
        .remove("originalHash");
    fs::write(&journal_path, serde_json::to_vec(&journal).unwrap()).unwrap();
    let prepared = plan(root.path(), backups.path(), &key);
    let preview = prepared.preview();
    assert_eq!(
        (preview.items[0].log_files, preview.items[0].state_rows),
        (0, 1)
    );
    prepared
        .execute(backups.path(), &preview.revision, keys(&preview))
        .unwrap();
    assert_eq!(provider(&conn, "a").unwrap().as_deref(), Some("legacy"));
    for relative in [
        "../outside.jsonl",
        "sessions/../../outside.jsonl",
        "config.toml",
        "sessions/bad:stream.jsonl",
    ] {
        assert!(catalog::relative(&root.path().join(relative), root.path(), true).is_err());
    }
    let bad_key = format!("codex-history-migration-{}", uuid::Uuid::new_v4());
    let bad = backups.path().join(bad_key);
    fs::create_dir(&bad).unwrap();
    fs::write(
        bad.join("migration.json"),
        "malformed data with fixture-secret",
    )
    .unwrap();
    let summaries = list(root.path(), backups.path()).unwrap();
    assert_eq!(summaries.len(), 2);
    let error = summaries
        .iter()
        .find_map(|summary| summary.problem.as_ref())
        .unwrap();
    assert!(!error.contains("fixture-secret"));
    assert!(error.contains("账本损坏"));
    assert!(identity(Path::new("x.jsonl"), b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\",\"model_provider\":\"custom\"}}\n{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\",\"model_provider\":\"custom\"}}\n").unwrap_err().contains("重复身份"));
}

#[test]
fn invalid_database_sidecars_stop_before_sqlite_or_any_restore_write() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let path = root.path().join("state_1.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT); INSERT INTO threads VALUES('a','legacy');").unwrap();
    drop(conn);
    let key = migrate(root.path(), backups.path());
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = root.path().join(format!("state_1.sqlite{suffix}"));
        fs::create_dir(&sidecar).unwrap();
        let preview = plan(root.path(), backups.path(), &key).preview();
        assert_eq!(preview.items[0].status, "conflict");
        assert!(preview.items[0]
            .reason
            .as_ref()
            .unwrap()
            .contains("附属文件"));
        assert!(open_state(&path, true).err().unwrap().contains("附属文件"));
        assert_eq!(fs::read_dir(backups.path()).unwrap().count(), 1);
        fs::remove_dir(&sidecar).unwrap();
    }
    assert_eq!(
        provider(&open_state(&path, false).unwrap(), "a")
            .unwrap()
            .as_deref(),
        Some("custom")
    );
}
