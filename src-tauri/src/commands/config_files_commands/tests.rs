use super::*;

#[test]
fn json5_draft_edit_retains_native_sections_comments_and_model_extensions() {
    let source = "\u{feff}{\r\n // keep header\r\n models: { providers: { local: { baseUrl: 'https://example.test/v1', apiKey: {source:'env', id:'KEY'}, models: [{id:'main', cost:{input:1, cacheRead:3}, extra:true}], custom:42 } }, mode:'merge' },\r\n env:{vars:{KEEP:'value'}}, agents:{defaults:{workspace:'unchanged'}}, channels:{extra:true},\r\n}";
    let mut desired = parse_openclaw_config_content(source.into()).unwrap();
    desired["models"]["providers"]["local"]["baseUrl"] = "https://new.test/v1".into();
    let written = edit_openclaw_config_content(source.into(), desired.clone()).unwrap();
    assert_eq!(
        parse_openclaw_config_content(written.clone()).unwrap(),
        desired
    );
    for untouched in [
        "// keep header",
        "apiKey: {source:'env', id:'KEY'}",
        "cost:{input:1, cacheRead:3}",
        "channels:{extra:true}",
        "mode:'merge'",
    ] {
        assert!(
            written.contains(untouched),
            "Unchanged native text must survive"
        );
    }
    assert!(written.starts_with('\u{feff}'));
    assert!(written.contains("\r\n"));
    assert_eq!(
        edit_openclaw_config_content(written.clone(), desired).unwrap(),
        written
    );
}

#[test]
fn native_draft_errors_do_not_echo_source_or_accept_duplicate_fields() {
    for source in [
        "{apiKey:'PRIVATE_TOKEN', broken",
        "{models:{},models:{}}",
        "[]",
    ] {
        let failure = parse_openclaw_config_content(source.into()).unwrap_err();
        assert!(!failure.contains("PRIVATE_TOKEN"));
    }
    assert!(edit_openclaw_config_content("{}".into(), serde_json::json!([])).is_err());
}

#[test]
fn checked_save_rejects_changed_deleted_and_unreadable_original_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.json");
    std::fs::write(&path, "external").unwrap();
    let error = write_checked_content(&path, "draft", Some("old")).unwrap_err();
    assert!(error.contains("changed externally"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
    std::fs::remove_file(&path).unwrap();
    assert!(write_checked_content(&path, "draft", Some("old")).is_err());
    assert!(!path.exists());
    assert!(write_checked_content(directory.path(), "draft", Some("old")).is_err());
}

#[test]
fn checked_save_preserves_noops_and_writes_the_reviewed_draft() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.json");
    std::fs::write(&path, "original").unwrap();
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    write_checked_content(&path, "original", Some("original")).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
    write_checked_content(&path, "draft", Some("original")).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "draft");
}

#[test]
fn configured_roots_include_separate_mcp_files_without_granting_their_parent() {
    let root = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    for candidate in CONFIG_ROOTS {
        conn.execute(
            "INSERT INTO custom_paths(tool_id,config_dir) VALUES(?1,?2)",
            rusqlite::params![
                candidate.id,
                root.path()
                    .join("tools")
                    .join(candidate.id)
                    .to_str()
                    .unwrap()
            ],
        )
        .unwrap();
    }
    let file = root.path().join("external/primary.json");
    let sibling = root.path().join("external/private.txt");
    std::fs::create_dir(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{}").unwrap();
    std::fs::write(&sibling, "unchanged").unwrap();
    conn.execute(
        "UPDATE custom_paths SET mcp_config_path=?1 WHERE tool_id='claude'",
        [file.to_str().unwrap()],
    )
    .unwrap();
    let roots = config_root_paths(&conn).unwrap();
    assert_eq!(roots.len(), CONFIG_ROOTS.len());
    assert!(roots
        .iter()
        .any(|(id, _, path)| id == "mcode" && path == &root.path().join("tools/mcode")));
    assert!(roots
        .iter()
        .any(|(id, _, path)| id == "claude-desktop"
            && path == &root.path().join("tools/claude-desktop")));
    assert!(is_allowed_path(&conn, &file).unwrap());
    assert!(!is_allowed_path(&conn, &sibling).unwrap());
    assert!(!is_allowed_path(&conn, file.parent().unwrap()).unwrap());
    assert_eq!(
        resolve_root_path(&conn, "claude").unwrap(),
        root.path().join("tools/claude")
    );
    let view = roots_from_conn(&conn).unwrap();
    assert!(view.iter().any(|root| root.id == "claude" && root.exists));
    let tree = tree_from_conn(&conn, "claude").unwrap();
    assert!(tree.is_dir);
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].path, file.to_string_lossy());
    assert_eq!(tree.children[0].name, "MCP · primary.json");
    assert!(!root.path().join("tools/claude").exists());
    std::fs::create_dir_all(root.path().join("tools/claude")).unwrap();
    std::fs::write(root.path().join("tools/claude/settings.json"), "{}").unwrap();
    let tree = tree_from_conn(&conn, "claude").unwrap();
    assert_eq!(tree.children.len(), 2);
    assert_eq!(
        tree.children
            .iter()
            .filter(|node| node.path == file.to_string_lossy())
            .count(),
        1
    );
    conn.execute(
        "UPDATE custom_paths SET config_dir=X'FF' WHERE tool_id='codex'",
        [],
    )
    .unwrap();
    assert!(config_root_paths(&conn).is_err());
    assert!(ensure_allowed_file(&conn, sibling.to_str().unwrap()).is_err());
    assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "unchanged");
}
