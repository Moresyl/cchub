use super::*;

#[test]
fn external_project_edit_before_commit_preserves_bytes_and_rolls_back_path_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("live.db")).unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    let source = dir.path().join("source").to_string_lossy().into_owned();
    let target = dir.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("a"), [255, 0, 1]).unwrap();
    std::fs::write(target.join("b"), "original b").unwrap();
    conn.execute(
        "INSERT INTO workspaces (id,name,base_path) VALUES ('w','project',?1)",
        [&source],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO hooks (id,event,command,project_path) VALUES ('h','event','noop',?1)",
        [&source],
    )
    .unwrap();
    set_json_app_setting(&conn, "known_project_roots", &vec![&source]).unwrap();
    super::super::backup_project_state::defer_project_root(&conn, &source).unwrap();
    for name in ["a", "b"] {
        store_imported_project_file(&conn, &source, name, "bmV3").unwrap();
    }
    let error = project_restore_transaction(&conn, |journal| {
        let count = remap_project_root(&conn, &source, &target.to_string_lossy(), journal)?;
        std::fs::write(target.join("b"), "external project edit").unwrap();
        Ok(count)
    })
    .unwrap_err();
    assert!(error.contains("外部修改已保留"));
    assert_eq!(std::fs::read(target.join("a")).unwrap(), [255, 0, 1]);
    assert_eq!(
        std::fs::read(target.join("b")).unwrap(),
        b"external project edit"
    );
    for query in [
        "SELECT base_path FROM workspaces WHERE id='w'",
        "SELECT project_path FROM hooks WHERE id='h'",
    ] {
        assert_eq!(
            conn.query_row(query, [], |row| row.get::<_, String>(0))
                .unwrap(),
            source
        );
    }
    assert!(super::super::backup_project_state::deferred_roots(&conn)
        .unwrap()
        .contains(&source));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM imported_project_files WHERE project_root=?1",
            [&source],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    let stored: Option<Vec<String>> = get_json_app_setting(&conn, "known_project_roots").unwrap();
    assert_eq!(stored.unwrap(), vec![source]);
    assert!(conn.is_autocommit());
    let recovery = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("cchub-file-rollback-")
        })
        .unwrap()
        .path();
    assert_eq!(std::fs::read(recovery.join("1")).unwrap(), b"original b");
    assert!(recovery.join("restore-map.json").exists());
}
