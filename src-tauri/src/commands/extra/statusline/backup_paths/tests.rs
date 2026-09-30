use super::*;

#[test]
fn relative_names_are_portable_and_confined() {
    for path in [
        "../escape",
        "/absolute",
        "C:/escape",
        "a\\b",
        "a/../b",
        "a//b",
        "./file",
        "file:stream",
        "nul.txt",
        "COM1",
        "LPT².log",
        "a.",
        "a ",
        "a\n",
        "",
    ] {
        assert!(relative_path(path, false).is_err(), "accepted {path:?}");
    }
    for path in [
        ".claude/settings.json",
        "技能/指令.md",
        "normal/file.txt",
        "com10.txt",
    ] {
        assert!(relative_path(path, false).is_ok());
    }
    assert!(relative_path("", true).is_ok());
}

fn fixture() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE _skill_files (tool_id TEXT, name TEXT); CREATE TABLE _backup_files (root_key TEXT, relative_path TEXT, content_base64 TEXT);").unwrap();
    conn
}

#[test]
fn artifact_preflight_rejects_invalid_domains_names_and_content() {
    for (root, path, body) in [
        ("tooldir:codex", "../escape", "YQ=="),
        ("tooldir:unknown", "config", "YQ=="),
        ("claude-desktop:unknown", "", "YQ=="),
        ("claude_mcp", "child", "YQ=="),
        ("project:", "file", "YQ=="),
        ("project:/demo", "file", "broken!"),
    ] {
        let conn = fixture();
        conn.execute(
            "INSERT INTO _backup_files VALUES (?1, ?2, ?3)",
            (root, path, body),
        )
        .unwrap();
        assert!(validate_artifact_paths(&conn).is_err());
    }
    for (tool, name) in [
        ("claude", "../escape"),
        ("claude", "nested/file"),
        ("unknown", "file"),
    ] {
        let conn = fixture();
        conn.execute("INSERT INTO _skill_files VALUES (?1, ?2)", (tool, name))
            .unwrap();
        assert!(validate_artifact_paths(&conn).is_err());
    }
    let conn = fixture();
    conn.execute(
        "INSERT INTO _backup_files VALUES ('skillsdir:pi', '技能.md', 'YQ==')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO _backup_files VALUES ('claude_mcp', '', 'YQ==')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO _skill_files VALUES ('hermes', 'SKILL.md')", [])
        .unwrap();
    validate_artifact_paths(&conn).unwrap();
}

#[test]
fn invalid_later_project_records_do_not_write_earlier_files() {
    use crate::commands::extra::statusline::project_roots::restore_imported_project_root_snapshot;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let existing = target.join("a.txt");
    std::fs::write(&existing, b"original").unwrap();
    for (path, body) in [("z/../../escape", "YQ=="), ("z.txt", "broken!")] {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE imported_project_files (project_root TEXT, relative_path TEXT, content_base64 TEXT);").unwrap();
        let source = dir.path().join("source").to_string_lossy().into_owned();
        conn.execute(
            "INSERT INTO imported_project_files VALUES (?1, 'a.txt', 'bmV3')",
            [&source],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO imported_project_files VALUES (?1, ?2, ?3)",
            (&source, path, body),
        )
        .unwrap();
        assert!(
            restore_imported_project_root_snapshot(&conn, &source, &target.to_string_lossy())
                .is_err()
        );
        assert_eq!(std::fs::read(&existing).unwrap(), b"original");
        assert!(!dir.path().join("escape").exists());
    }
}

#[cfg(unix)]
#[test]
fn descendant_links_cannot_redirect_a_restore() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    symlink(&outside, root.join("link")).unwrap();
    assert!(confined_target(&root, "link/file.txt", false).is_err());
    // A user-selected linked root is allowed, while descendants stay confined.
    let selected = dir.path().join("selected");
    symlink(&root, &selected).unwrap();
    assert_eq!(
        confined_target(&selected, "file.txt", false).unwrap(),
        root.join("file.txt")
    );
}

#[cfg(windows)]
#[test]
fn windows_junctions_are_rejected_below_a_selected_restore_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let link = root.join("junction");
    let mut command = std::process::Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:CCHUB_TEST_LINK -Target $env:CCHUB_TEST_TARGET -ErrorAction Stop | Out-Null"])
        .env("CCHUB_TEST_LINK", &link).env("CCHUB_TEST_TARGET", &outside);
    crate::utils::configure_background_command(&mut command);
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(confined_target(&root, "junction/file.txt", false).is_err());
    assert_eq!(
        confined_target(&link, "file.txt", false).unwrap(),
        outside.canonicalize().unwrap().join("file.txt")
    );
    std::fs::remove_dir(&link).unwrap();
    assert!(outside.is_dir());
}

#[test]
fn malformed_artifacts_stop_before_any_tool_configuration_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO custom_paths (tool_id, config_dir) VALUES ('claude', ?1)",
        [dir.path().to_string_lossy()],
    )
    .unwrap();
    conn.execute_batch("CREATE TABLE _backup_meta (key TEXT, value TEXT); CREATE TABLE _tool_configs (tool_id TEXT, config_path TEXT, config_content TEXT); CREATE TABLE _skill_files (tool_id TEXT, name TEXT, content TEXT); CREATE TABLE _backup_files (root_key TEXT, relative_path TEXT, content_base64 TEXT);").unwrap();
    conn.execute(
        "INSERT INTO _tool_configs VALUES ('claude-settings', '', '{\"env\":{}}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO _skill_files VALUES ('claude', '../escape', 'bad')",
        [],
    )
    .unwrap();
    assert!(super::super::backup_artifacts::restore_imported_artifacts(&conn, 2).is_err());
    assert!(!dir.path().join("settings.json").exists());
}

#[test]
fn skill_restore_includes_every_exported_tool() {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute_batch("CREATE TABLE _backup_meta (key TEXT, value TEXT); CREATE TABLE _tool_configs (tool_id TEXT, config_path TEXT, config_content TEXT); CREATE TABLE _skill_files (tool_id TEXT, name TEXT, content TEXT); CREATE TABLE _backup_files (root_key TEXT, relative_path TEXT, content_base64 TEXT);").unwrap();
    let tools = super::super::backups_restore::TOOL_BACKUP_IDS;
    for tool in tools {
        let target = dir.path().join(tool);
        conn.execute(
            "INSERT INTO custom_paths (tool_id, skills_dir) VALUES (?1, ?2)",
            (tool, target.to_string_lossy()),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO _skill_files VALUES (?1, 'SKILL.md', 'instructions')",
            [tool],
        )
        .unwrap();
    }
    let counts =
        super::super::backup_artifacts::restore_imported_artifacts(&conn, tools.len()).unwrap();
    assert_eq!(counts.2, tools.len());
    for tool in tools {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(tool).join("SKILL.md")).unwrap(),
            "instructions"
        );
    }
}

#[cfg(windows)]
#[test]
fn project_remap_rolls_back_paths_and_prior_file_writes_on_a_late_failure() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    let source = dir.path().join("source").to_string_lossy().into_owned();
    let target = dir.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("a"), "original a").unwrap();
    std::fs::write(target.join("b"), "original b").unwrap();
    conn.execute(
        "INSERT INTO workspaces (id,name,base_path) VALUES ('w','项目',?1)",
        [&source],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO hooks (id,event,command,project_path) VALUES ('h','event','noop',?1)",
        [&source],
    )
    .unwrap();
    super::super::super::config_profiles::set_json_app_setting(
        &conn,
        "known_project_roots",
        &vec![&source],
    )
    .unwrap();
    super::super::backup_project_state::defer_project_root(&conn, &source).unwrap();
    for name in ["a", "b"] {
        super::super::project_roots::store_imported_project_file(&conn, &source, name, "bmV3")
            .unwrap();
    }
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(target.join("b"))
        .unwrap();
    assert!(super::super::project_roots::apply_project_root_remap(
        &conn,
        &source,
        &target.to_string_lossy()
    )
    .is_err());
    assert_eq!(
        std::fs::read_to_string(target.join("a")).unwrap(),
        "original a"
    );
    assert_eq!(
        std::fs::read_to_string(target.join("b")).unwrap(),
        "original b"
    );
    for query in [
        "SELECT base_path FROM workspaces WHERE id='w'",
        "SELECT project_path FROM hooks WHERE id='h'",
    ] {
        assert_eq!(
            conn.query_row(query, [], |r| r.get::<_, String>(0))
                .unwrap(),
            source
        );
    }
    assert!(super::super::backup_project_state::deferred_roots(&conn)
        .unwrap()
        .contains(&source));
    let roots: Vec<String> =
        super::super::super::config_profiles::get_json_app_setting(&conn, "known_project_roots")
            .unwrap()
            .unwrap();
    assert_eq!(roots, vec![source.clone()]);
    assert!(conn.is_autocommit());
    drop(held);
    assert_eq!(
        super::super::project_roots::apply_project_root_remap(
            &conn,
            &source,
            &target.to_string_lossy()
        )
        .unwrap(),
        2
    );
    assert_eq!(std::fs::read_to_string(target.join("a")).unwrap(), "new");
    assert!(super::super::backup_project_state::deferred_roots(&conn)
        .unwrap()
        .is_empty());
}

#[test]
fn project_remap_database_failure_after_file_write_restores_files_and_rows() {
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    let source = dir.path().join("source").to_string_lossy().into_owned();
    let target = dir.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("file"), "original").unwrap();
    conn.execute(
        "INSERT INTO workspaces (id,name,base_path) VALUES ('w','项目',?1)",
        [&source],
    )
    .unwrap();
    super::super::project_roots::store_imported_project_file(&conn, &source, "file", "bmV3")
        .unwrap();
    conn.authorizer(Some(|context: AuthContext<'_>| {
        if matches!(
            context.action,
            AuthAction::Insert {
                table_name: "imported_project_files"
            }
        ) {
            Authorization::Deny
        } else {
            Authorization::Allow
        }
    }));
    assert!(super::super::project_roots::apply_project_root_remap(
        &conn,
        &source,
        &target.to_string_lossy()
    )
    .is_err());
    conn.authorizer(None::<fn(AuthContext<'_>) -> Authorization>);
    assert_eq!(
        std::fs::read_to_string(target.join("file")).unwrap(),
        "original"
    );
    assert_eq!(
        conn.query_row("SELECT base_path FROM workspaces WHERE id='w'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        source
    );
    assert_eq!(
        conn.query_row("SELECT project_root FROM imported_project_files", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        source
    );
    assert!(conn.is_autocommit());
}
