use super::claude;
use crate::db::DbState;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{path::Path, sync::Mutex};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};

fn ipc(window: &tauri::WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    tauri::test::get_ipc_response(
        window,
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}
fn app(path: &Path) -> tauri::App<MockRuntime> {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE custom_paths (tool_id TEXT PRIMARY KEY, config_dir TEXT, mcp_config_path TEXT);").unwrap();
    conn.execute(
        "INSERT INTO custom_paths VALUES ('claude', ?1, 'unrelated-mcp.json')",
        [path.to_str().unwrap()],
    )
    .unwrap();
    mock_builder()
        .manage(DbState(Mutex::new(conn)))
        .invoke_handler(tauri::generate_handler![
            super::get_claude_settings,
            super::set_claude_setting
        ])
        .build(mock_context(noop_assets()))
        .unwrap()
}
fn save(path: &Path, key: &str, value: &str) -> claude::ClaudeSettings {
    let revision = claude::read(path).unwrap().config_revision;
    claude::write(path, key, value, &revision).unwrap()
}

#[test]
fn ipc_reads_configured_paths_and_rejects_missing_or_stale_revisions() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let initial = ipc(&window, "get_claude_settings", json!({})).unwrap();
    assert_eq!(initial["permission_mode"], "");
    assert_eq!(initial["model"], "");
    assert!(ipc(
        &window,
        "set_claude_setting",
        json!({"key":"model", "value":"custom"})
    )
    .is_err());
    assert!(!root.path().join("settings.json").exists());
    let saved = ipc(&window, "set_claude_setting", json!({
        "key":"permission_mode", "value":"acceptEdits", "expectedRevision":initial["config_revision"],
    })).unwrap();
    assert_eq!(saved["permission_mode"], "acceptEdits");
    let source = std::fs::read(root.path().join("settings.json")).unwrap();
    assert!(ipc(
        &window,
        "set_claude_setting",
        json!({
            "key":"model", "value":"custom", "expectedRevision":initial["config_revision"],
        })
    )
    .is_err());
    assert_eq!(
        std::fs::read(root.path().join("settings.json")).unwrap(),
        source
    );
}

#[test]
fn permission_modes_preserve_every_existing_rule_and_opaque_field() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    let source = "{\r\n  \"permissions\": {\"allow\": [\"Read(*)\"], \"ask\": [\"Bash(git *)\"], \"deny\": [\"Read(.env)\"], \"additionalDirectories\": [\"D:/owned\"], \"defaultMode\": \"normal\"},\r\n  \"env\": {\"ANTHROPIC_AUTH_TOKEN\":\"keep-secret\"},\r\n  \"opaque\": {\"precise\":9007199254740993123,\"exponent\":1e3},\r\n  \"skipDangerousModePermissionPrompt\":false\r\n}\r\n";
    for mode in [
        "default",
        "acceptEdits",
        "plan",
        "auto",
        "dontAsk",
        "bypassPermissions",
        "manual",
        "",
    ] {
        std::fs::write(&path, source).unwrap();
        assert_eq!(claude::read(&path).unwrap().permission_mode, "normal");
        let result = save(&path, "permission_mode", mode);
        assert_eq!(result.permission_mode, mode);
        assert_eq!(
            (result.allow_count, result.ask_count, result.deny_count),
            (1, 1, 1)
        );
        let changed = std::fs::read_to_string(&path).unwrap();
        for unchanged in [
            "\"allow\": [\"Read(*)\"]",
            "\"ask\": [\"Bash(git *)\"]",
            "\"deny\": [\"Read(.env)\"]",
            "\"additionalDirectories\": [\"D:/owned\"]",
            "\"opaque\": {\"precise\":9007199254740993123,\"exponent\":1e3}",
            "\"skipDangerousModePermissionPrompt\":false",
            "\"ANTHROPIC_AUTH_TOKEN\":\"keep-secret\"",
        ] {
            assert!(
                changed.contains(unchanged),
                "unowned field changed: {unchanged}"
            );
        }
        assert!(!changed.replace("\r\n", "").contains('\n'));
    }
}

#[test]
fn update_channel_round_trips_without_changing_other_environment_controls() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{"autoUpdatesChannel":"stable","env":{"DISABLE_UPDATES":"1","token":"keep-secret"}}"#,
    )
    .unwrap();
    assert_eq!(
        save(&path, "auto_update", "disabled").auto_update,
        "disabled"
    );
    let disabled: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(disabled["autoUpdatesChannel"], "stable");
    assert_eq!(save(&path, "auto_update", "latest").auto_update, "latest");
    let enabled: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(enabled["env"].get("DISABLE_AUTOUPDATER").is_none());
    assert_eq!(enabled["env"]["DISABLE_UPDATES"], "1");
    assert_eq!(enabled["env"]["token"], "keep-secret");
    assert_eq!(save(&path, "auto_update", "").auto_update, "");
}

#[test]
fn model_ids_are_exact_and_default_removes_only_the_owned_key() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{"model":"gateway/custom-opus-model","env":{"ANTHROPIC_MODEL":"other"},"other":1e3}"#,
    )
    .unwrap();
    assert_eq!(
        claude::read(&path).unwrap().model,
        "gateway/custom-opus-model"
    );
    assert_eq!(
        save(&path, "model", "new/custom[1m]").model,
        "new/custom[1m]"
    );
    assert_eq!(save(&path, "model", "").model, "");
    let source = std::fs::read_to_string(&path).unwrap();
    assert!(source.contains("\"ANTHROPIC_MODEL\":\"other\""));
    assert!(source.contains("\"other\":1e3"));
}

#[test]
fn tool_search_migrates_only_on_explicit_save_and_preserves_both_file_rules() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    let local = root.path().join("settings.local.json");
    let user = r#"{"env":{"ENABLE_TOOL_SEARCH":"auto:5","token":"user-secret"},"permissions":{"deny":["Read(.env)"]}}"#;
    let legacy = "{\r\n  \"env\": {\"ENABLE_TOOL_SEARCH\":\"true\",\"other\":\"keep\"},\r\n  \"permissions\": {\"allow\":[\"Read(*)\"]},\r\n  \"opaque\": 1e3\r\n}\r\n";
    std::fs::write(&path, user).unwrap();
    std::fs::write(&local, legacy).unwrap();
    let initial = claude::read(&path).unwrap();
    assert_eq!(
        (
            initial.tool_search.as_str(),
            initial.legacy_tool_search.as_str()
        ),
        ("auto:5", "true")
    );
    save(&path, "model", "sonnet");
    assert_eq!(std::fs::read_to_string(&local).unwrap(), legacy);
    let result = save(&path, "tool_search", "false");
    assert_eq!(
        (
            result.tool_search.as_str(),
            result.legacy_tool_search.as_str()
        ),
        ("false", "")
    );
    let changed = std::fs::read_to_string(&local).unwrap();
    assert!(changed.contains("\"other\":\"keep\""));
    assert!(changed.contains("\"allow\":[\"Read(*)\"]"));
    assert!(changed.contains("\"opaque\": 1e3\r\n"));
    assert!(!changed.contains("ENABLE_TOOL_SEARCH"));
    assert_eq!(save(&path, "tool_search", "auto:0").tool_search, "auto:0");
    assert_eq!(
        save(&path, "tool_search", "auto:100").tool_search,
        "auto:100"
    );
    assert_eq!(save(&path, "tool_search", "").tool_search, "");
}

#[test]
fn either_file_or_location_changing_invalidates_the_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    let local = root.path().join("settings.local.json");
    std::fs::write(&path, "{}\n").unwrap();
    std::fs::write(other.path().join("settings.json"), "{}\n").unwrap();
    let revision = claude::read(&path).unwrap().config_revision;
    assert!(claude::write(
        &other.path().join("settings.json"),
        "model",
        "opus",
        &revision
    )
    .is_err());
    std::fs::write(&local, r#"{"env":{"ENABLE_TOOL_SEARCH":"true"}}"#).unwrap();
    assert!(claude::write(&path, "model", "opus", &revision).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}\n");
}

#[test]
fn malformed_and_ambiguous_settings_fail_without_exposing_content_or_changing_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    for source in [
        "{\"token\":\"keep-secret\"",
        "[]",
        "{\"env\":[]}",
        "{\"permissions\":null}",
        "{\"permissions\":{\"deny\":[42]}}",
        "{\"model\":true}",
        "{\"model\":\"a\",\"model\":\"b\"}",
        "{ /* keep-secret */ }",
        "{\"model\":\"x\",}",
    ] {
        std::fs::write(&path, source).unwrap();
        let error = claude::read(&path).unwrap_err();
        assert!(!error.contains("keep-secret"));
        assert!(claude::write(&path, "model", "opus", "invalid").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
    std::fs::write(&path, "{}").unwrap();
    std::fs::write(
        root.path().join("settings.local.json"),
        "{\"token\":\"keep-secret",
    )
    .unwrap();
    assert!(!claude::read(&path).unwrap_err().contains("keep-secret"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
}

#[test]
fn invalid_inputs_never_create_files_and_empty_defaults_are_noops() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("missing/settings.json");
    let initial = claude::read(&path).unwrap();
    for (key, value) in [
        ("permission_mode", "normal"),
        ("auto_update", "other"),
        ("tool_search", "auto:101"),
        ("tool_search", "auto:-1"),
        ("tool_search", "auto:+5"),
        ("model", " leading"),
        ("model", "bad\nmodel"),
        ("unknown", "value"),
    ] {
        assert!(claude::write(&path, key, value, &initial.config_revision).is_err());
        assert!(!path.parent().unwrap().exists());
    }
    for key in ["permission_mode", "auto_update", "model", "tool_search"] {
        assert_eq!(
            save(&path, key, "").config_revision,
            initial.config_revision
        );
        assert!(!path.parent().unwrap().exists());
    }
}

#[test]
fn unchanged_windows_settings_keep_bytes_revision_and_timestamp() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    let source = "{\r\n  \"model\":\"custom\",\r\n  \"opaque\":1e3\r\n}\r\n";
    std::fs::write(&path, source).unwrap();
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let revision = claude::read(&path).unwrap().config_revision;
    assert_eq!(save(&path, "model", "custom").config_revision, revision);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
}

#[test]
fn oversized_or_non_regular_files_cannot_be_edited() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("settings.json");
    std::fs::create_dir(&path).unwrap();
    assert!(claude::read(&path).is_err());
    let local = root.path().join("settings.local.json");
    std::fs::File::create(&local)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    let other = root.path().join("other.json");
    std::fs::write(&other, "{}").unwrap();
    assert!(claude::read(&other).unwrap_err().contains("size limit"));
}
