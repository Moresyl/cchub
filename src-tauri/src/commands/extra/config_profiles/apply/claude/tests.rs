use super::*;

fn paths(dir: &tempfile::TempDir) -> [std::path::PathBuf; 2] {
    [
        dir.path().join(".claude.json"),
        dir.path().join(".claude/settings.json"),
    ]
}

fn write(path: &Path, content: &str) {
    crate::utils::atomic_write_string(path, content).unwrap();
}

fn value(path: &Path) -> Value {
    json_config::parse_json_object(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn invalid_either_document_stops_capture_and_application_without_changing_its_sibling() {
    for index in 0..2 {
        for invalid in [
            "{\"secret\":\"private-sentinel\",broken}",
            "[]",
            "null",
            "{\"env\":{},\"env\":{}}",
            "{\"env\":{\"KEY\":1,\"KEY\":2}}",
            "{ /* comment */ \"env\":{} }",
            "{\"env\":{},}",
            "\u{feff}{\"env\":{}}",
            "",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths(&dir);
            write(&paths[index], invalid);
            write(&paths[1 - index], "{\"keep\":1}\n");
            let before = paths.each_ref().map(|path| std::fs::read(path).unwrap());
            let borrowed = paths.each_ref().map(|path| path.as_path());
            let error = read_at(borrowed).unwrap_err();
            assert!(!error.contains("private-sentinel"));
            let error = prepare_at(borrowed, r#"{"primaryApiKey":"new","env":{"KEY":"new"}}"#)
                .err()
                .unwrap();
            assert!(!error.contains("private-sentinel"));
            assert_eq!(
                paths.each_ref().map(|path| std::fs::read(path).unwrap()),
                before
            );
        }
    }
}

#[test]
fn invalid_utf8_and_non_file_targets_are_never_treated_as_missing() {
    for index in 0..2 {
        for is_directory in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths(&dir);
            std::fs::create_dir_all(paths[index].parent().unwrap()).unwrap();
            if is_directory {
                std::fs::create_dir(&paths[index]).unwrap();
            } else {
                std::fs::write(&paths[index], [0xff, 0]).unwrap();
            }
            let borrowed = paths.each_ref().map(|path| path.as_path());
            assert!(read_at(borrowed).is_err());
            assert!(prepare_at(borrowed, r#"{"env":{"KEY":"new"}}"#).is_err());
            assert!(!paths[1 - index].exists());
        }
    }
}

#[test]
fn apply_preserves_format_unrelated_fields_and_protected_settings() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(&dir);
    write(
        &paths[0],
        "{\r\n \"primaryApiKey\":\"old\",\r\n \"projects\" : { \"mine\" : true }\r\n}\r\n",
    );
    write(&paths[1], "{\n \"env\":{\"KEY\":\"old\"},\n \"statusLine\":{\"command\":\"keep\"},\n \"enabledPlugins\":{\"mine\":true},\n \"mcpServers\":{\"local\":{}},\n \"hooks\" : { \"keep\" : [] }\n}\n");
    let snapshot = serde_json::json!({
        "primaryApiKey":"new", "env":{"KEY":"new"},
        "statusLine":null, "enabledPlugins":{}, "mcpServers":{}
    })
    .to_string();
    prepare_at(paths.each_ref().map(|path| path.as_path()), &snapshot)
        .unwrap()
        .commit()
        .unwrap();
    let first = std::fs::read_to_string(&paths[0]).unwrap();
    let second = std::fs::read_to_string(&paths[1]).unwrap();
    assert!(first.contains("\"projects\" : { \"mine\" : true }"));
    assert!(first.ends_with("\r\n"));
    assert!(second.contains("\"hooks\" : { \"keep\" : [] }"));
    assert!(serde_json::from_str::<Value>(&first).is_ok());
    assert!(serde_json::from_str::<Value>(&second).is_ok());
    assert_eq!(value(&paths[0])["primaryApiKey"], "new");
    assert_eq!(value(&paths[0])["projects"]["mine"], true);
    let settings = value(&paths[1]);
    assert_eq!(settings["env"]["KEY"], "new");
    assert_eq!(settings["statusLine"]["command"], "keep");
    assert_eq!(settings["enabledPlugins"]["mine"], true);
    assert_eq!(settings["mcpServers"], serde_json::json!({"local":{}}));
    assert_eq!(settings["hooks"], serde_json::json!({"keep":[]}));
}

#[test]
fn tagged_sources_and_legacy_heuristics_keep_their_existing_file_routing() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(&dir);
    write(&paths[0], r#"{"customGlobal":1,"primaryApiKey":"old"}"#);
    write(
        &paths[1],
        r#"{"customSetting":1,"permissions":{"allow":[]}}"#,
    );
    let borrowed = paths.each_ref().map(|path| path.as_path());
    let mut snapshot: Value = serde_json::from_str(&read_at(borrowed).unwrap()).unwrap();
    snapshot["customGlobal"] = serde_json::json!(2);
    snapshot["customSetting"] = serde_json::json!(3);
    snapshot["newSetting"] = serde_json::json!(4);
    prepare_at(borrowed, &snapshot.to_string())
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(value(&paths[0])["customGlobal"], 2);
    assert!(value(&paths[0]).get("customSetting").is_none());
    assert_eq!(value(&paths[1])["customSetting"], 3);
    assert_eq!(value(&paths[1])["newSetting"], 4);
    assert!(!value(&paths[1])
        .as_object()
        .unwrap()
        .keys()
        .any(|key| SOURCE_KEYS.contains(&key.as_str())));
}

#[test]
fn invalid_snapshot_or_source_metadata_never_creates_configuration_directories() {
    for snapshot in [
        "[]",
        "{broken",
        "{\"env\":{},\"env\":{}}",
        "{ /* comment */ \"env\":{} }",
        "{\"env\":{},}",
        r#"{"__claude_json_keys__":null}"#,
        r#"{"__settings_json_keys__":{}}"#,
        r#"{"__claude_json_keys__":[1]}"#,
        r#"{"__claude_json_keys__":["env","env"]}"#,
        r#"{"__claude_json_keys__":["__settings_json_keys__"]}"#,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(&dir);
        assert!(prepare_at(paths.each_ref().map(|path| path.as_path()), snapshot).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}

#[test]
fn external_changes_after_preparation_prevent_every_write() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(&dir);
    write(&paths[0], "{\"primaryApiKey\":\"old\"}\n");
    write(&paths[1], "{\"env\":{\"KEY\":\"old\"}}\n");
    let first = std::fs::read(&paths[0]).unwrap();
    let plan = prepare_at(
        paths.each_ref().map(|path| path.as_path()),
        r#"{"primaryApiKey":"new","env":{"KEY":"new"}}"#,
    )
    .unwrap();
    write(&paths[1], "{\"external\":true}\n");
    assert!(plan.commit().unwrap_err().contains("externally"));
    assert_eq!(std::fs::read(&paths[0]).unwrap(), first);
    assert_eq!(value(&paths[1]), serde_json::json!({"external":true}));
}

#[test]
fn failed_finalization_rolls_back_exact_bytes_and_new_files() {
    for existed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(&dir);
        write(&paths[0], "{ \"primaryApiKey\": \"old\" }\r\n");
        if existed {
            write(&paths[1], "{\"env\":{\"KEY\":\"old\"}}\n");
        }
        let before = paths
            .each_ref()
            .map(|path| config_write::read(path).unwrap());
        let plan = prepare_at(
            paths.each_ref().map(|path| path.as_path()),
            r#"{"primaryApiKey":"new","env":{"KEY":"new"}}"#,
        )
        .unwrap();
        let error = config_write::commit_then(plan.updates, || {
            assert_eq!(value(&paths[0])["primaryApiKey"], "new");
            assert_eq!(value(&paths[1])["env"]["KEY"], "new");
            Err("finalizer fixture".into())
        })
        .unwrap_err();
        assert!(error.contains("finalizer fixture"));
        assert_eq!(
            paths
                .each_ref()
                .map(|path| config_write::read(path).unwrap()),
            before
        );
        if !existed {
            assert!(!paths[1].parent().unwrap().exists());
        }
    }
}

#[test]
fn unchanged_files_keep_bytes_timestamps_and_missing_global_file_stays_missing() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(&dir);
    write(&paths[1], "{ \"env\": { \"KEY\": \"same\" } }\n");
    let original = std::fs::read(&paths[1]).unwrap();
    let modified = std::fs::metadata(&paths[1]).unwrap().modified().unwrap();
    prepare_at(
        paths.each_ref().map(|path| path.as_path()),
        r#"{"env":{"KEY":"same"}}"#,
    )
    .unwrap()
    .commit()
    .unwrap();
    assert_eq!(std::fs::read(&paths[1]).unwrap(), original);
    assert_eq!(
        std::fs::metadata(&paths[1]).unwrap().modified().unwrap(),
        modified
    );
    assert!(!paths[0].exists());
    assert!(prepare_at([&paths[1], &paths[1]], "{}").is_err());
    assert!(read_at([&paths[1], &paths[1]]).is_err());
}

#[test]
fn appearing_untouched_file_is_preserved_and_prevents_changed_sibling_write() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(&dir);
    write(&paths[1], "{\"env\":{\"KEY\":\"old\"}}\n");
    let before = std::fs::read(&paths[1]).unwrap();
    let modified = std::fs::metadata(&paths[1]).unwrap().modified().unwrap();
    let plan = prepare_at(
        paths.each_ref().map(|path| path.as_path()),
        r#"{"env":{"KEY":"new"}}"#,
    )
    .unwrap();
    write(&paths[0], "{\"external\":true}\n");
    assert!(plan.commit().is_err());
    assert_eq!(value(&paths[0]), serde_json::json!({"external":true}));
    assert_eq!(std::fs::read(&paths[1]).unwrap(), before);
    assert_eq!(
        std::fs::metadata(&paths[1]).unwrap().modified().unwrap(),
        modified
    );
}

#[test]
fn untouched_file_appearing_during_finalization_keeps_external_edit_and_rolls_back_ours() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(&dir);
    write(&paths[1], "{\"env\":{\"KEY\":\"old\"}}\n");
    let before = std::fs::read(&paths[1]).unwrap();
    let plan = prepare_at(
        paths.each_ref().map(|path| path.as_path()),
        r#"{"env":{"KEY":"new"}}"#,
    )
    .unwrap();
    check_absent(&plan.absent).unwrap();
    assert!(config_write::commit_then(plan.updates, || {
        write(&paths[0], "{\"external\":true}\n");
        check_absent(&plan.absent)
    })
    .is_err());
    assert_eq!(value(&paths[0]), serde_json::json!({"external":true}));
    assert_eq!(std::fs::read(&paths[1]).unwrap(), before);
}

#[test]
fn actual_snapshot_adapter_uses_checked_pair_and_custom_paths() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths(&dir);
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE custom_paths (tool_id TEXT PRIMARY KEY, config_dir TEXT, mcp_config_path TEXT); CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT);").unwrap();
    conn.execute(
        "INSERT INTO custom_paths VALUES ('claude',?1,?2)",
        rusqlite::params![
            paths[1].parent().unwrap().to_str().unwrap(),
            paths[0].to_str().unwrap()
        ],
    )
    .unwrap();
    write(&paths[0], r#"{"primaryApiKey":"old"}"#);
    write(&paths[1], r#"{"env":{"KEY":"old"}}"#);
    super::super::apply_tool_snapshot(
        &conn,
        "claude",
        r#"{"primaryApiKey":"new","env":{"KEY":"new"}}"#,
    )
    .unwrap();
    assert_eq!(value(&paths[0])["primaryApiKey"], "new");
    assert_eq!(value(&paths[1])["env"]["KEY"], "new");
    assert!(super::super::read_tool_snapshot(&conn, "claude").is_ok());
    write(&paths[1], "{broken}");
    let before = std::fs::read(&paths[0]).unwrap();
    assert!(
        super::super::apply_tool_snapshot(&conn, "claude", r#"{"primaryApiKey":"next"}"#).is_err()
    );
    assert_eq!(std::fs::read(&paths[0]).unwrap(), before);
    assert!(super::super::read_tool_snapshot(&conn, "claude").is_err());
}
