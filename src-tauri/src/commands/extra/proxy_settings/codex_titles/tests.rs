use super::super::session_scanners::scan_codex_sessions_from_plan;
use super::*;
use std::io::Cursor;

fn write_index(root: &Path, text: &str) {
    std::fs::write(root.join("session_index.jsonl"), text).unwrap();
}

fn wanted<'a>(ids: &[&'a str]) -> HashSet<&'a str> {
    ids.iter().copied().collect()
}

#[test]
fn requested_names_use_latest_valid_rename_and_do_not_retain_other_threads() {
    let root = tempfile::tempdir().unwrap();
    write_index(root.path(), "{\"id\":\"main\",\"thread_name\":\"旧标题\"}\n{\"id\":\"other\",\"thread_name\":\"私有会话\"}\n{\"id\":\"main\",\"thread_name\":\"  新标题 \\n 多行  \"}\n{broken");
    let mut names = load(root.path(), &wanted(&["main", "missing"]));
    assert_eq!(names.len(), 1);
    assert_eq!(names.remove("main").unwrap(), "新标题 多行");
    assert_eq!(load(root.path(), &wanted(&["main"]))["main"], "新标题 多行");
    assert!(load(root.path(), &wanted(&[])).is_empty());
}

#[test]
fn same_size_rewrites_replacement_missing_files_and_other_roots_never_use_stale_names() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    write_index(first.path(), "{\"id\":\"main\",\"thread_name\":\"one\"}\n");
    let requested = wanted(&["main"]);
    assert_eq!(load(first.path(), &requested)["main"], "one");
    let previous = std::fs::metadata(first.path().join("session_index.jsonl"))
        .unwrap()
        .modified()
        .unwrap();
    write_index(first.path(), "{\"id\":\"main\",\"thread_name\":\"two\"}\n");
    File::options()
        .write(true)
        .open(first.path().join("session_index.jsonl"))
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(previous))
        .unwrap();
    assert_eq!(load(first.path(), &requested)["main"], "two");
    assert!(load(second.path(), &requested).is_empty());
    write_index(
        second.path(),
        "{\"id\":\"main\",\"thread_name\":\"other\"}\n",
    );
    assert_eq!(load(second.path(), &requested)["main"], "other");
    std::fs::remove_file(first.path().join("session_index.jsonl")).unwrap();
    assert!(load(first.path(), &requested).is_empty());
    write_index(
        first.path(),
        "{\"id\":\"main\",\"thread_name\":\"replaced\"}\n",
    );
    assert_eq!(load(first.path(), &requested)["main"], "replaced");
}

#[test]
fn malformed_duplicate_typed_or_empty_names_do_not_erase_a_valid_name() {
    let root = tempfile::tempdir().unwrap();
    let mut wire = "{\"id\":\"main\",\"thread_name\":\"valid\"}\n".to_owned();
    for row in [
        "null",
        "[]",
        "{",
        r#"{"id":7,"thread_name":"wrong"}"#,
        r#"{"id":"main","thread_name":7}"#,
        r#"{"id":"main","thread_name":null}"#,
        r#"{"id":"main","thread_name":"  \n  "}"#,
        r#"{"id":"main","id":"main","thread_name":"wrong"}"#,
        r#"{"id":"main","thread_name":"wrong","thread_name":"other"}"#,
    ] {
        wire += row;
        wire += "\n";
    }
    write_index(root.path(), &wire);
    assert_eq!(load(root.path(), &wanted(&["main"]))["main"], "valid");
    std::fs::write(
        root.path().join("session_index.jsonl"),
        b"\xff\n{\"id\":\"main\",\"thread_name\":\"after invalid UTF-8\"}",
    )
    .unwrap();
    assert_eq!(
        load(root.path(), &wanted(&["main"]))["main"],
        "after invalid UTF-8"
    );
}

#[test]
fn unicode_titles_are_bounded_without_breaking_utf8_and_opaque_unknown_fields_are_ignored() {
    let root = tempfile::tempdir().unwrap();
    let title = "你好🦀".repeat(100);
    write_index(
        root.path(),
        &format!(
            r#"{{"id":"main","thread_name":"{title}","opaque":{{"n":1.2e9999,"signature":"abc=="}}}}"#
        ),
    );
    let names = load(root.path(), &wanted(&["main"]));
    assert_eq!(
        names["main"],
        title.chars().take(MAX_TITLE_CHARS).collect::<String>()
    );
    assert_eq!(names["main"].chars().count(), MAX_TITLE_CHARS);
}

#[test]
fn oversized_index_lines_are_skipped_and_later_valid_renames_still_win() {
    for ending in ["\n", "\r\n"] {
        let input = format!(
            "{}{}{{\"id\":\"main\",\"thread_name\":\"after huge row\"}}\n",
            "x".repeat(MAX_INDEX_LINE + 100),
            ending
        );
        let names = read_names(Cursor::new(input), &wanted(&["main"])).unwrap();
        assert_eq!(names["main"], "after huge row");
    }
    assert!(read_names(
        Cursor::new("x".repeat(MAX_INDEX_LINE + 100)),
        &wanted(&["main"])
    )
    .unwrap()
    .is_empty());
}

#[test]
fn read_failures_never_return_a_partial_index_as_complete() {
    struct Fails;
    impl Read for Fails {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("fixture read failure"))
        }
    }
    let prefix = Cursor::new(b"{\"id\":\"main\",\"thread_name\":\"partial\"}\n".to_vec());
    let reader = BufReader::new(prefix.chain(Fails));
    assert!(read_names(reader, &wanted(&["main"])).is_err());
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("session_index.jsonl")).unwrap();
    assert!(load(root.path(), &wanted(&["main"])).is_empty());
}

fn native_fixture(root: &Path) -> PathBuf {
    let path = root.join("state_5.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE threads(id TEXT,rollout_path TEXT,created_at INTEGER,updated_at INTEGER,cwd TEXT,title TEXT,first_user_message TEXT);").unwrap();
    conn.execute("INSERT INTO threads VALUES('main','sessions/rollout-main.jsonl',1700000000,1700000001,'C:/fixture','数据库旧标题','initial-only-query')", []).unwrap();
    std::fs::create_dir_all(root.join("sessions")).unwrap();
    std::fs::write(
        root.join("sessions/rollout-main.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"main\",\"title\":\"文件旧标题\"}}\n",
    )
    .unwrap();
    path
}

#[test]
fn native_database_names_and_search_use_the_index_without_changing_identity_or_files() {
    let root = tempfile::tempdir().unwrap();
    let db = native_fixture(root.path());
    let roots = vec![root.path().to_path_buf()];
    let before = scan_codex_sessions_from_plan(Some(root.path()), &[db.clone()], &roots, "");
    assert_eq!(before.len(), 1);
    let db_bytes = std::fs::read(&db).unwrap();
    let rollout = root.path().join("sessions/rollout-main.jsonl");
    let rollout_bytes = std::fs::read(&rollout).unwrap();
    write_index(
        root.path(),
        "{\"id\":\"main\",\"thread_name\":\"重命名后的会话\"}\n",
    );
    let index_bytes = std::fs::read(root.path().join("session_index.jsonl")).unwrap();
    let after = scan_codex_sessions_from_plan(Some(root.path()), &[db.clone()], &roots, "");
    let mut expected = before[0].clone();
    expected.title = "重命名后的会话".to_owned();
    assert_eq!(
        serde_json::to_value(&after[0]).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    let matches = scan_codex_sessions_from_plan(Some(root.path()), &[db.clone()], &roots, "重命名");
    assert_eq!(matches.len(), 1);
    assert!(matches[0].search_hit_count > 0);
    assert_eq!(
        scan_codex_sessions_from_plan(
            Some(root.path()),
            &[db.clone()],
            &roots,
            "initial-only-query"
        )
        .len(),
        1
    );
    assert!(scan_codex_sessions_from_plan(
        Some(root.path()),
        &[db.clone()],
        &roots,
        "数据库旧标题"
    )
    .is_empty());
    assert!(
        scan_codex_sessions_from_plan(Some(root.path()), &[db.clone()], &roots, "文件旧标题")
            .is_empty()
    );
    assert_eq!(std::fs::read(&db).unwrap(), db_bytes);
    assert_eq!(std::fs::read(rollout).unwrap(), rollout_bytes);
    assert_eq!(
        std::fs::read(root.path().join("session_index.jsonl")).unwrap(),
        index_bytes
    );
}

fn generic_fixture(root: &Path, title: &str) {
    std::fs::create_dir_all(root.join("sessions")).unwrap();
    std::fs::write(
        root.join("sessions/rollout-shared.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"shared\",\"title\":\"旧标题\"}}\n",
    )
    .unwrap();
    write_index(
        root,
        &format!("{{\"id\":\"shared\",\"thread_name\":\"{title}\"}}\n"),
    );
    std::fs::write(
        root.join("history.jsonl"),
        "{\"session_id\":\"shared\",\"text\":\"全局输入索引\"}\n",
    )
    .unwrap();
}

#[test]
fn generic_project_titles_are_scoped_and_metadata_indexes_are_not_sessions() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    generic_fixture(first.path(), "第一个项目");
    generic_fixture(second.path(), "第二个项目");
    let roots = vec![first.path().to_path_buf(), second.path().to_path_buf()];
    let sessions = scan_codex_sessions_from_plan(Some(first.path()), &[], &roots, "");
    assert_eq!(
        sessions.len(),
        2,
        "indexes must not become phantom deletable sessions"
    );
    for session in &sessions {
        let owner = Path::new(&session.source_path).canonicalize().unwrap();
        let expected = if owner.starts_with(first.path().canonicalize().unwrap()) {
            "第一个项目"
        } else {
            "第二个项目"
        };
        assert_eq!(session.id, "shared");
        assert_eq!(session.title, expected);
    }
    assert_eq!(
        scan_codex_sessions_from_plan(Some(first.path()), &[], &roots, "第二个").len(),
        1
    );
    assert!(
        scan_codex_sessions_from_plan(Some(first.path()), &[], &roots, "全局输入索引").is_empty()
    );
}

#[test]
fn missing_and_bad_indexes_fall_back_to_the_existing_session_title() {
    let root = tempfile::tempdir().unwrap();
    let db = native_fixture(root.path());
    let roots = vec![root.path().to_path_buf()];
    for index in [
        None,
        Some("broken"),
        Some("[]"),
        Some("{\"id\":\"unrelated\",\"thread_name\":\"wrong\"}\n"),
    ] {
        if let Some(index) = index {
            write_index(root.path(), index);
        }
        let sessions =
            scan_codex_sessions_from_plan(Some(root.path()), &[db.clone()], &roots, "数据库旧标题");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title, "数据库旧标题");
    }
}
