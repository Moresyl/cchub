use super::*;
use std::time::UNIX_EPOCH;

mod ipc_tests;

#[test]
fn database_count_limit_matches_the_recovery_catalog_before_any_write() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..=MAX_STATE_DATABASES {
        let conn = Connection::open(root.path().join(format!("state_{index}.sqlite"))).unwrap();
        conn.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT); INSERT INTO threads VALUES('session','legacy');").unwrap();
        if index + 1 == MAX_STATE_DATABASES {
            assert_eq!(plan(root.path()).preview().state_rows, MAX_STATE_DATABASES);
        }
    }
    let error = MigrationPlan::prepare(root.path().into(), vec!["legacy".into()], "custom".into())
        .err()
        .unwrap();
    assert!(error.contains("数据库超过 128"));
    let conn = Connection::open(root.path().join("state_0.sqlite")).unwrap();
    assert_eq!(
        conn.query_row("SELECT model_provider FROM threads", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "legacy"
    );
    assert_eq!(
        fs::read_dir(root.path()).unwrap().count(),
        MAX_STATE_DATABASES + 1
    );
}

fn log(root: &Path, name: &str) -> PathBuf {
    let dir = root.join("sessions");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let bytes = concat!(
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"same\",\"model_provider\":\"legacy\",\"extra\":42}}\r\n",
        "{\"type\":\"response_item\",\"payload\":{\"text\":\"保留内容\"}}\r\n"
    ).as_bytes();
    fs::write(
        &path,
        if archive::compressed(&path) {
            zstd::stream::encode_all(bytes, 0).unwrap()
        } else {
            bytes.to_vec()
        },
    )
    .unwrap();
    old(&path);
    path
}
fn old(path: &Path) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1700000000))
        .unwrap();
}
fn plan(root: &Path) -> MigrationPlan {
    MigrationPlan::prepare(root.into(), vec!["legacy".into()], "custom".into()).unwrap()
}
fn state(root: &Path) -> Connection {
    let conn = Connection::open(root.join("state_1.sqlite")).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT,title TEXT); INSERT INTO threads VALUES('same','legacy','WAL title'),('official','openai','official');").unwrap();
    conn
}

#[test]
fn plain_and_packed_twins_migrate_with_consistent_wal_backups_and_exact_originals() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let plain = log(root.path(), "rollout.jsonl");
    let packed = log(root.path(), "rollout.jsonl.zst");
    let original_plain = fs::read(&plain).unwrap();
    let original_packed = fs::read(&packed).unwrap();
    let conn = state(root.path());
    let prepared = plan(root.path());
    let preview = prepared.preview();
    assert_eq!(
        (
            preview.jsonl_files,
            preview.compressed_files,
            preview.state_rows
        ),
        (2, 1, 1)
    );
    assert_eq!(
        fs::read(&plain).unwrap(),
        original_plain,
        "preview never writes"
    );
    let result = prepared
        .execute(backups.path(), Some(&preview.revision))
        .unwrap();
    assert_eq!(
        (result.migrated_jsonl_files, result.migrated_state_rows),
        (2, 1)
    );
    let backup = PathBuf::from(result.backup_path.unwrap());
    assert_eq!(
        fs::read(backup.join("jsonl/sessions/rollout.jsonl")).unwrap(),
        original_plain
    );
    assert_eq!(
        fs::read(backup.join("jsonl/sessions/rollout.jsonl.zst")).unwrap(),
        original_packed
    );
    let saved = Connection::open(backup.join("state/state_1.sqlite")).unwrap();
    assert_eq!(
        saved
            .query_row(
                "SELECT model_provider,title FROM threads WHERE id='same'",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            )
            .unwrap(),
        ("legacy".into(), "WAL title".into())
    );
    assert_eq!(
        conn.query_row(
            "SELECT model_provider FROM threads WHERE id='same'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "custom"
    );
    assert_eq!(
        conn.query_row(
            "SELECT model_provider FROM threads WHERE id='official'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "openai"
    );
    let text =
        String::from_utf8(zstd::stream::decode_all(fs::read(&packed).unwrap().as_slice()).unwrap())
            .unwrap();
    assert!(
        text.contains("\"model_provider\":\"custom\"")
            && text.contains("\"extra\":42")
            && text.contains("保留内容")
    );
    assert!(text.ends_with("\r\n"));
    assert!(plan(root.path())
        .execute(backups.path(), None)
        .unwrap()
        .backup_path
        .is_none());
}

#[test]
fn stale_preview_or_live_log_change_cannot_write_any_target() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let path = log(root.path(), "rollout.jsonl");
    let original = fs::read(&path).unwrap();
    let preview = plan(root.path()).preview();
    let changed = [original.as_slice(), b"{}\n"].concat();
    fs::write(&path, &changed).unwrap();
    old(&path);
    assert!(plan(root.path())
        .execute(backups.path(), Some(&preview.revision))
        .unwrap_err()
        .contains("预览后"));
    assert_eq!(fs::read(&path).unwrap(), changed);
    assert_eq!(fs::read_dir(backups.path()).unwrap().count(), 0);
    let prepared = plan(root.path());
    fs::write(&path, "external content").unwrap();
    assert!(prepared
        .execute(backups.path(), None)
        .unwrap_err()
        .contains("检查后"));
    assert_eq!(fs::read(&path).unwrap(), b"external content");
}

#[test]
fn second_file_failure_rolls_back_logs_and_keeps_state_and_all_original_copies() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let first = log(root.path(), "a.jsonl");
    let second = log(root.path(), "b.jsonl.zst");
    let original_first = fs::read(&first).unwrap();
    let original_second = fs::read(&second).unwrap();
    let conn = state(root.path());
    let mut calls = 0;
    let error = plan(root.path())
        .execute_with(backups.path(), None, |path, bytes| {
            calls += 1;
            if calls == 2 {
                Err("fixture write failure".into())
            } else {
                crate::utils::atomic_write(path, bytes).map_err(|e| e.to_string())
            }
        })
        .unwrap_err();
    assert!(error.contains("已回滚"));
    assert_eq!(fs::read(first).unwrap(), original_first);
    assert_eq!(fs::read(second).unwrap(), original_second);
    assert_eq!(
        conn.query_row(
            "SELECT model_provider FROM threads WHERE id='same'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "legacy"
    );
    let backup = fs::read_dir(backups.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(backup.join("migration.json").exists() && backup.join("state/state_1.sqlite").exists());
}

#[test]
fn rollback_does_not_overwrite_an_external_edit_and_retains_recovery_evidence() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let first = log(root.path(), "a.jsonl");
    log(root.path(), "b.jsonl");
    let mut calls = 0;
    let error = plan(root.path())
        .execute_with(backups.path(), None, |path, bytes| {
            calls += 1;
            if calls == 2 {
                fs::write(&first, "external edit").unwrap();
                Err("fixture failure".into())
            } else {
                crate::utils::atomic_write(path, bytes).map_err(|e| e.to_string())
            }
        })
        .unwrap_err();
    assert!(error.contains("部分内容未能回滚"));
    assert_eq!(fs::read(first).unwrap(), b"external edit");
    assert_eq!(fs::read_dir(backups.path()).unwrap().count(), 1);
}

#[test]
fn changed_state_rows_and_active_or_corrupt_logs_fail_before_modification() {
    let root = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let path = log(root.path(), "a.jsonl");
    let original = fs::read(&path).unwrap();
    let conn = state(root.path());
    let prepared = plan(root.path());
    conn.execute(
        "UPDATE threads SET model_provider='another' WHERE id='same'",
        [],
    )
    .unwrap();
    assert!(prepared
        .execute(backups.path(), None)
        .unwrap_err()
        .contains("状态在检查后变化"));
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::write(&path, &original).unwrap();
    assert!(
        MigrationPlan::prepare(root.path().into(), vec!["legacy".into()], "custom".into())
            .err()
            .unwrap()
            .contains("最近仍在更新")
    );
    old(&path);
    fs::write(root.path().join("sessions/b.jsonl.zst"), "broken archive").unwrap();
    assert!(
        MigrationPlan::prepare(root.path().into(), vec!["legacy".into()], "custom".into()).is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn rewritten_size_and_corrupt_metadata_fail_before_any_backup_or_mutation() {
    let original = br#"{"type":"session_meta","payload":{"model_provider":"a"}}"#;
    for name in ["a.jsonl", "a.jsonl.zst"] {
        let path = Path::new(name);
        let bytes = if archive::compressed(path) {
            zstd::stream::encode_all(original.as_slice(), 0).unwrap()
        } else {
            original.to_vec()
        };
        assert!(
            rewrite_limited(path, &bytes, &["a".into()], &"b".repeat(128), 100)
                .unwrap_err()
                .contains("迁移后的历史文件")
        );
    }
    let root = tempfile::tempdir().unwrap();
    let path = log(root.path(), "broken.jsonl");
    let malformed = br#"{"type":"session_meta","payload":{"model_provider":"legacy"}"#;
    fs::write(&path, malformed).unwrap();
    old(&path);
    assert!(
        MigrationPlan::prepare(root.path().into(), vec!["legacy".into()], "custom".into())
            .err()
            .unwrap()
            .contains("元数据损坏")
    );
    assert_eq!(fs::read(path).unwrap(), malformed);
}

#[test]
fn provider_inference_rejects_invalid_configuration_and_explicit_sources() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        super::super::prepare_migration(root.path().into(), None, None)
            .unwrap()
            .preview()
            .source_provider_ids
            .is_empty()
    );
    fs::write(
        root.path().join("config.toml"),
        "[model_providers.legacy]\nname='old'\n[model_providers.openai]\nname='official'",
    )
    .unwrap();
    assert_eq!(
        super::super::prepare_migration(root.path().into(), None, None)
            .unwrap()
            .preview()
            .source_provider_ids,
        vec!["legacy"]
    );
    fs::write(
        root.path().join("config.toml"),
        "secret='do-not-echo'\n[broken",
    )
    .unwrap();
    let error = super::super::prepare_migration(root.path().into(), None, None)
        .err()
        .unwrap();
    assert!(error.contains("TOML 无效") && !error.contains("do-not-echo"));
    assert!(super::super::prepare_migration(
        root.path().into(),
        Some(vec!["../outside".into()]),
        None
    )
    .is_err());
    assert!(super::super::prepare_migration(
        root.path().into(),
        Some(vec!["legacy".into(); 129]),
        None
    )
    .is_err());
}
