use super::*;

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
