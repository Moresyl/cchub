use super::*;
use crate::commands::extra_commands::{
    apply_tool_snapshot, bootstrap_tool_environment_from_conn, read_tool_snapshot,
    resolve_claude_paths, resolve_codex_structured_paths, resolve_tool_config_dir,
    resolve_tool_config_path, resolve_tool_mcp_path, resolve_tool_skills_dir,
};

fn connection() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn
}

fn configure(conn: &Connection, tool: &str, dir: &std::path::Path, file: Option<&std::path::Path>) {
    conn.execute(
        "INSERT INTO custom_paths(tool_id,config_dir,mcp_config_path) VALUES(?1,?2,?3)",
        rusqlite::params![
            tool,
            dir.to_str().unwrap(),
            file.map(|path| path.to_str().unwrap())
        ],
    )
    .unwrap();
}

#[test]
fn all_eight_mcp_families_honor_exact_filenames_without_creating_files() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    for tool in [
        "claude",
        "claude-desktop",
        "codex",
        "gemini",
        "grokbuild",
        "opencode",
        "hermes",
        "mcode",
    ] {
        let directory = root.path().join(tool).join("sidecars");
        let file = root
            .path()
            .join(tool)
            .join("separate")
            .join("custom-mcp.native");
        configure(&conn, tool, &directory, Some(&file));
        assert_eq!(resolve_tool_mcp_path(&conn, tool).unwrap(), file, "{tool}");
        if tool != "claude-desktop" {
            assert_eq!(
                resolve_tool_config_dir(&conn, tool).unwrap(),
                directory,
                "{tool}"
            );
            let provider = resolve_tool_config_path(&conn, tool).unwrap();
            assert_eq!(
                provider,
                match tool {
                    "claude" => directory.join("settings.json"),
                    "mcode" => directory.join("config.yaml"),
                    _ => file,
                },
                "{tool}"
            );
        }
    }
    assert!(resolve_tool_mcp_path(&conn, "openclaw").is_err());
    assert!(resolve_tool_mcp_path(&conn, "unknown").is_err());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn directory_overrides_use_native_filenames_and_keep_separate_mcp_files() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    for (tool, filename) in [
        ("claude-desktop", "claude_desktop_config.json"),
        ("codex", "config.toml"),
        ("gemini", "settings.json"),
        ("grokbuild", "config.toml"),
        ("opencode", "opencode.json"),
        ("hermes", "config.yaml"),
        ("mcode", "mcp.json"),
    ] {
        let directory = root.path().join(tool);
        configure(&conn, tool, &directory, None);
        assert_eq!(
            resolve_tool_mcp_path(&conn, tool).unwrap(),
            directory.join(filename)
        );
    }
    let directory = root.path().join("opencode");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("opencode.jsonc"), "// fixture\n{}").unwrap();
    assert_eq!(
        resolve_tool_mcp_path(&conn, "opencode").unwrap(),
        directory.join("opencode.jsonc")
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn missing_values_are_optional_but_schema_types_and_busy_reads_fail() {
    let conn = connection();
    assert!(read(&conn, "codex", Field::McpFile).unwrap().is_none());
    conn.execute(
        "INSERT INTO custom_paths(tool_id,config_dir) VALUES('codex',NULL)",
        [],
    )
    .unwrap();
    assert!(read(&conn, "codex", Field::ConfigDir).unwrap().is_none());
    conn.execute("UPDATE custom_paths SET config_dir = X'0102'", [])
        .unwrap();
    let error = resolve_tool_config_dir(&conn, "codex").unwrap_err();
    assert!(!error.contains("0102"));
    conn.execute_batch("DROP TABLE custom_paths").unwrap();
    assert!(resolve_tool_config_path(&conn, "codex").is_err());
    assert!(crate::opencode_paths::config_path(&conn).is_err());
    assert!(crate::hermes::config_path(&conn).is_err());

    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let writer = Connection::open(&path).unwrap();
    writer
        .execute_batch("CREATE TABLE custom_paths(tool_id TEXT, config_dir TEXT); BEGIN EXCLUSIVE;")
        .unwrap();
    let reader = Connection::open(&path).unwrap();
    reader.busy_timeout(std::time::Duration::ZERO).unwrap();
    assert!(read(&reader, "codex", Field::ConfigDir).is_err());
    writer.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn invalid_locations_are_rejected_without_creating_directories_or_disclosing_them() {
    let root = tempfile::tempdir().unwrap();
    let private = root.path().join("private-sentinel");
    std::fs::write(&private, "unchanged").unwrap();
    for value in ["relative/config.json", "bad\npath", "C:relative.json"] {
        assert!(validate(value, true).is_err());
    }
    assert!(validate(root.path().to_str().unwrap(), true).is_err());
    assert!(validate(private.to_str().unwrap(), false).is_err());
    assert!(validate("  ", false).unwrap().is_none());
    let spaced = root.path().join(" 带空格的目录 ").join("custom.toml");
    assert_eq!(
        validate(spaced.to_str().unwrap(), true).unwrap(),
        Some(spaced)
    );
    assert_eq!(std::fs::read_to_string(&private).unwrap(), "unchanged");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn profile_capture_apply_and_structured_editor_share_custom_files_and_sidecar_roots() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    for (tool, filename, snapshot) in [
        (
            "codex",
            "named.toml",
            r#"{"auth":{"OPENAI_API_KEY":"fixture"},"config":"model='fixture'"}"#,
        ),
        (
            "gemini",
            "named.json",
            r#"{"env":{"GEMINI_API_KEY":"fixture"},"config":{"theme":"dark"}}"#,
        ),
        (
            "grokbuild",
            "named.toml",
            r#"{"config":"[models]\ndefault='fixture'"}"#,
        ),
        (
            "hermes",
            "named.yaml",
            r#"{"config":{"model":{"provider":"openai","default":"fixture"}},"env":{"OPENAI_API_KEY":"fixture"}}"#,
        ),
    ] {
        let directory = root.path().join(tool).join("sidecars");
        let file = root.path().join(tool).join("native").join(filename);
        configure(&conn, tool, &directory, Some(&file));
        apply_tool_snapshot(&conn, tool, snapshot).unwrap();
        let captured: serde_json::Value =
            serde_json::from_str(&read_tool_snapshot(&conn, tool).unwrap()).unwrap();
        assert!(file.is_file(), "{tool}");
        let default = match tool {
            "gemini" => "settings.json",
            "hermes" => "config.yaml",
            _ => "config.toml",
        };
        assert!(!directory.join(default).exists(), "{tool}");
        assert!(captured.get("config").is_some(), "{tool}");
        if tool == "codex" {
            assert_eq!(
                resolve_codex_structured_paths(&conn, None).unwrap(),
                (file.clone(), directory.join("auth.json"))
            );
            assert_eq!(
                resolve_codex_structured_paths(&conn, Some(file.to_string_lossy().into_owned()))
                    .unwrap(),
                (file.clone(), directory.join("auth.json"))
            );
            assert_eq!(captured["auth"]["OPENAI_API_KEY"], "fixture");
        }
    }
}

#[test]
fn hermes_custom_paths_precede_legacy_override_and_corrupt_legacy_settings_fail() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    let legacy = root.path().join("legacy");
    crate::hermes::write_root_override(&conn, Some(legacy.to_str().unwrap())).unwrap();
    assert_eq!(
        crate::hermes::config_path(&conn).unwrap(),
        legacy.join("config.yaml")
    );
    let directory = root.path().join("current");
    let file = root.path().join("separate/native.yaml");
    configure(&conn, "hermes", &directory, Some(&file));
    assert_eq!(crate::hermes::hermes_root(&conn).unwrap(), directory);
    assert_eq!(crate::hermes::config_path(&conn).unwrap(), file);
    conn.execute("DELETE FROM custom_paths", []).unwrap();
    conn.execute(
        "UPDATE app_settings SET value = X'FF' WHERE key = ?1",
        [crate::hermes::ROOT_OVERRIDE_SETTING_KEY],
    )
    .unwrap();
    assert!(crate::hermes::config_path(&conn).is_err());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn bootstrap_checks_late_path_errors_before_any_creation_and_uses_the_exact_file() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    let directory = root.path().join("sidecars");
    let file = root.path().join("native/named.toml");
    configure(&conn, "codex", &directory, Some(&file));
    conn.execute("UPDATE custom_paths SET skills_dir=X'FF'", [])
        .unwrap();
    assert!(bootstrap_tool_environment_from_conn(&conn, "codex").is_err());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    conn.execute("UPDATE custom_paths SET skills_dir=NULL", [])
        .unwrap();
    conn.execute(
        "INSERT INTO app_settings(key,value) VALUES('skill_storage_location',X'FF')",
        [],
    )
    .unwrap();
    assert!(bootstrap_tool_environment_from_conn(&conn, "codex").is_err());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    conn.execute(
        "DELETE FROM app_settings WHERE key='skill_storage_location'",
        [],
    )
    .unwrap();
    bootstrap_tool_environment_from_conn(&conn, "codex").unwrap();
    assert!(file.is_file());
    assert!(directory.join("auth.json").is_file());
    assert!(!directory.join("config.toml").exists());
}

#[test]
fn claude_primary_mcp_override_never_relocates_its_settings_directory() {
    let root = tempfile::tempdir().unwrap();
    let conn = connection();
    let file = root.path().join("outside/custom-primary.json");
    let directory = root.path().join("settings");
    configure(&conn, "claude", &directory, Some(&file));
    assert_eq!(
        resolve_claude_paths(&conn).unwrap(),
        (file.clone(), directory.join("settings.json"))
    );
    apply_tool_snapshot(
        &conn,
        "claude",
        r#"{"env":{"ANTHROPIC_API_KEY":"fixture"}}"#,
    )
    .unwrap();
    // Environment-only profiles belong to settings.json and intentionally do
    // not create an otherwise absent primary MCP document.
    assert!(!file.exists());
    apply_tool_snapshot(
        &conn,
        "claude",
        r#"{"primaryApiKey":"fixture-primary","env":{"ANTHROPIC_API_KEY":"fixture"}}"#,
    )
    .unwrap();
    assert!(file.is_file());
    let primary: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(primary["primaryApiKey"], "fixture-primary");
    assert!(directory.join("settings.json").is_file());
    assert!(!file.parent().unwrap().join("settings.json").exists());
    conn.execute("UPDATE custom_paths SET config_dir=NULL", [])
        .unwrap();
    assert_eq!(
        resolve_tool_config_dir(&conn, "claude").unwrap(),
        dirs::home_dir().unwrap().join(".claude")
    );
    assert!(resolve_tool_skills_dir(&conn, "unknown").is_err());
}
