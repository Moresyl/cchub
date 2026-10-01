use super::super::snapshot::{library_revision, snapshot_at};
use super::*;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&crate::db::schema::get_schema_sql())
        .unwrap();
    conn
}

fn input(content: &str, enabled: bool) -> PromptInput {
    PromptInput {
        name: "Fixture".into(),
        content: content.into(),
        description: None,
        enabled,
    }
}

fn save(conn: &mut Connection, path: &Path, content: &str) -> Result<PromptRecord, String> {
    let state = snapshot_at(conn, "claude", path)?;
    save_at(
        conn,
        "claude",
        "target",
        input(content, true),
        Some(path),
        Some(&state.library_revision),
        state.live.as_ref().map(|live| live.revision.as_str()),
    )
}

#[test]
fn replacement_retains_handwritten_bytes_and_exclusive_app_state() {
    for app in SUPPORTED_APPS {
        let directory = tempfile::tempdir().unwrap();
        let path = prompt_path_for_home(directory.path(), app).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "\u{feff}手写🦀\r\n").unwrap();
        let mut conn = database();
        save_at(
            &mut conn,
            "other",
            "other",
            input("separate", true),
            None,
            None,
            None,
        )
        .unwrap();
        let state = snapshot_at(&conn, app, &path).unwrap();
        save_at(
            &mut conn,
            app,
            "target",
            input("new", true),
            Some(&path),
            Some(&state.library_revision),
            Some(&state.live.unwrap().revision),
        )
        .unwrap();
        let records = load_prompts(&conn, app).unwrap();
        assert_eq!(records.len(), 2);
        assert!(records["target"].enabled);
        assert!(records
            .values()
            .any(|record| !record.enabled && record.content == "\u{feff}手写🦀\r\n"));
        assert!(load_prompts(&conn, "other").unwrap()["other"].enabled);
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }
}

#[test]
fn editing_the_active_record_retains_its_previous_version_and_avoids_duplicate_backups() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("CLAUDE.md");
    let mut conn = database();
    save(&mut conn, &path, "first").unwrap();
    save(&mut conn, &path, "second").unwrap();
    let records = load_prompts(&conn, "claude").unwrap();
    assert_eq!(records.len(), 2);
    assert!(records
        .values()
        .any(|record| record.content == "first" && !record.enabled));
    save(&mut conn, &path, "second").unwrap();
    assert_eq!(load_prompts(&conn, "claude").unwrap().len(), 2);
    let old = records
        .values()
        .find(|record| record.content == "first")
        .unwrap();
    enable_at(&mut conn, "claude", &old.id, Some(&path), None, None).unwrap();
    assert_eq!(load_prompts(&conn, "claude").unwrap().len(), 2);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "first");
}

#[test]
fn inactive_saves_never_touch_a_live_file_even_if_it_is_unreadable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("directory");
    std::fs::create_dir(&path).unwrap();
    let mut conn = database();
    save_at(
        &mut conn,
        "claude",
        "target",
        input("draft", false),
        Some(&path),
        None,
        None,
    )
    .unwrap();
    assert!(path.is_dir());
    assert!(!load_prompts(&conn, "claude").unwrap()["target"].enabled);
}

#[test]
fn imported_files_remain_byte_identical_and_repeated_imports_keep_the_same_id() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("CLAUDE.md");
    let bytes = "\u{feff}# 用户\r\n".as_bytes();
    std::fs::write(&path, bytes).unwrap();
    let mut conn = database();
    let id = import_at(&mut conn, "claude", &path, None, None).unwrap();
    assert_eq!(
        import_at(&mut conn, "claude", &path, None, None).unwrap(),
        id
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(load_prompts(&conn, "claude").unwrap().len(), 1);
    assert!(load_prompts(&conn, "claude").unwrap()[&id].enabled);
}

#[test]
fn stale_library_or_live_revisions_leave_both_stores_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("CLAUDE.md");
    let mut conn = database();
    save(&mut conn, &path, "first").unwrap();
    let stale = snapshot_at(&conn, "claude", &path).unwrap();
    save_at(
        &mut conn,
        "claude",
        "draft",
        input("new draft", false),
        None,
        None,
        None,
    )
    .unwrap();
    let before = load_prompts(&conn, "claude").unwrap();
    assert!(save_at(
        &mut conn,
        "claude",
        "target",
        input("second", true),
        Some(&path),
        Some(&stale.library_revision),
        Some(&stale.live.as_ref().unwrap().revision)
    )
    .is_err());
    assert_eq!(load_prompts(&conn, "claude").unwrap(), before);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
    let current = snapshot_at(&conn, "claude", &path).unwrap();
    std::fs::write(&path, "external").unwrap();
    assert!(save_at(
        &mut conn,
        "claude",
        "target",
        input("second", true),
        Some(&path),
        Some(&current.library_revision),
        Some(&current.live.unwrap().revision)
    )
    .is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
    assert_eq!(load_prompts(&conn, "claude").unwrap(), before);
}

#[test]
fn database_write_failure_cannot_replace_the_live_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("CLAUDE.md");
    std::fs::write(&path, "original").unwrap();
    let mut conn = database();
    conn.execute_batch("CREATE TRIGGER reject BEFORE INSERT ON prompt_library BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(save(&mut conn, &path, "replacement").is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
    assert!(load_prompts(&conn, "claude").unwrap().is_empty());
}

#[test]
fn a_deferred_commit_failure_restores_original_bytes_or_original_absence() {
    for existed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new-directory").join("CLAUDE.md");
        if existed {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "\u{feff}original\r\n").unwrap();
        }
        let mut conn = database();
        conn.execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE parent(id INTEGER PRIMARY KEY);
            CREATE TABLE dangling(parent_id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);
            CREATE TRIGGER fail_commit AFTER INSERT ON prompt_library WHEN NEW.id='target'
            BEGIN INSERT INTO dangling VALUES(99); END;").unwrap();
        assert!(save(&mut conn, &path, "replacement").is_err());
        assert!(load_prompts(&conn, "claude").unwrap().is_empty());
        if existed {
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                "\u{feff}original\r\n"
            );
        } else {
            assert!(!path.exists());
            assert!(!path.parent().unwrap().exists());
        }
    }
}

#[test]
fn snapshots_distinguish_missing_empty_invalid_and_oversized_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("CLAUDE.md");
    let conn = database();
    let missing = snapshot_at(&conn, "claude", &path).unwrap().live.unwrap();
    assert_eq!(missing.content, None);
    std::fs::write(&path, "").unwrap();
    let empty = read_live(&path).unwrap();
    assert_ne!(empty.revision, missing.revision);
    assert_eq!(empty.content.as_deref(), Some(""));
    std::fs::write(&path, [255]).unwrap();
    let invalid = snapshot_at(&conn, "claude", &path).unwrap();
    assert!(invalid.live.is_none());
    assert!(invalid.live_error.unwrap().contains("UTF-8"));
    std::fs::write(&path, vec![b'a'; MAX_PROMPT_BYTES + 1]).unwrap();
    assert!(read_live(&path).is_err());
}

#[test]
fn revisions_are_app_owned_and_detect_content_changes_without_timestamp_changes() {
    let mut conn = database();
    save_at(
        &mut conn,
        "claude",
        "target",
        input("first", false),
        None,
        None,
        None,
    )
    .unwrap();
    let before = load_prompts(&conn, "claude").unwrap();
    let first = library_revision("claude", &before).unwrap();
    assert_ne!(first, library_revision("codex", &before).unwrap());
    conn.execute(
        "UPDATE prompt_library SET content='second' WHERE app_id='claude'",
        [],
    )
    .unwrap();
    assert_ne!(
        first,
        library_revision("claude", &load_prompts(&conn, "claude").unwrap()).unwrap()
    );
}

#[test]
fn failed_or_stale_imports_do_not_change_database_or_live_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("CLAUDE.md");
    let mut conn = database();
    assert!(import_at(&mut conn, "claude", &path, None, None).is_err());
    std::fs::write(&path, "current").unwrap();
    assert!(import_at(&mut conn, "claude", &path, None, Some("missing")).is_err());
    assert!(load_prompts(&conn, "claude").unwrap().is_empty());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "current");
}

#[cfg(windows)]
#[test]
fn a_real_windows_file_replace_failure_rolls_back_the_library_and_its_backup() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("CLAUDE.md");
    std::fs::write(&path, "original").unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    let mut conn = database();
    assert!(save(&mut conn, &path, "replacement").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    assert!(load_prompts(&conn, "claude").unwrap().is_empty());
    drop(held);
}
