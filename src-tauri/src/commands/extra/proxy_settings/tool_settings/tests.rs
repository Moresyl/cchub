use super::{codex, storage};
use crate::db::DbState;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{path::Path, sync::Mutex};
use tauri::{
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    Manager,
};

fn conn(directory: &Path) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE custom_paths (tool_id TEXT PRIMARY KEY, config_dir TEXT, mcp_config_path TEXT);").unwrap();
    conn.execute(
        "INSERT INTO custom_paths VALUES ('codex', ?1, NULL)",
        [directory.to_str().unwrap()],
    )
    .unwrap();
    conn
}

fn app(directory: &Path) -> tauri::App<MockRuntime> {
    mock_builder()
        .manage(DbState(Mutex::new(conn(directory))))
        .invoke_handler(tauri::generate_handler![
            super::get_codex_settings,
            super::set_codex_setting
        ])
        .build(mock_context(noop_assets()))
        .unwrap()
}

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

#[test]
fn ipc_uses_configured_directory_and_returns_acknowledged_revision() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let initial = ipc(&window, "get_codex_settings", json!({})).unwrap();
    assert_eq!(initial["approval_mode"], "default");
    let changed = ipc(&window, "set_codex_setting", json!({
        "key": "approval_mode", "value": "workspace-write", "expectedRevision": initial["config_revision"],
    })).unwrap();
    assert_eq!(changed["approval_mode"], "workspace-write");
    assert_ne!(changed["config_revision"], initial["config_revision"]);
    let source = std::fs::read_to_string(root.path().join("config.toml")).unwrap();
    assert!(source.contains("approval_policy = \"on-request\""));
    assert!(source.contains("sandbox_mode = \"workspace-write\""));
    assert!(!source.contains("personality"));
    assert!(ipc(&window, "set_codex_setting", json!({
        "key": "context_window_1m", "value": "true", "expectedRevision": initial["config_revision"],
    })).is_err());
    assert_eq!(
        std::fs::read_to_string(root.path().join("config.toml")).unwrap(),
        source
    );
}

#[test]
fn ipc_rejects_old_location_even_when_both_files_have_identical_bytes() {
    let root = tempfile::tempdir().unwrap();
    let other = root.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let source = "model = 'mine'\n";
    std::fs::write(root.path().join("config.toml"), source).unwrap();
    std::fs::write(other.join("config.toml"), source).unwrap();
    let app = app(root.path());
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let initial = ipc(&window, "get_codex_settings", json!({})).unwrap();
    app.state::<DbState>()
        .0
        .lock()
        .unwrap()
        .execute(
            "UPDATE custom_paths SET config_dir = ?1",
            [other.to_str().unwrap()],
        )
        .unwrap();
    assert!(ipc(&window, "set_codex_setting", json!({
        "key": "reasoning_effort", "value": "high", "expectedRevision": initial["config_revision"],
    })).is_err());
    for file in [root.path().join("config.toml"), other.join("config.toml")] {
        assert_eq!(std::fs::read_to_string(file).unwrap(), source);
    }
}

#[test]
fn permission_presets_preserve_personality_and_opaque_configuration() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let tail = "\n# provider token stays byte-identical\n[model_providers.mine]\nexperimental_bearer_token = '''keep-secret''' # keep\n[permissions.custom.filesystem]\n\"/own/path\" = 'read'\n[sandbox_workspace_write]\nnetwork_access = false\n";
    for (mode, policy) in [
        ("read-only", "on-request"),
        ("workspace-write", "on-request"),
        ("danger-full-access", "never"),
    ] {
        std::fs::write(&path, format!("personality = 'friendly' # keep style\ndefault_permissions = 'custom'\napproval_policy = {{ granular = {{ rules = false }} }}\n{tail}")).unwrap();
        let result = codex::write(&path, "approval_mode", mode, None).unwrap();
        assert_eq!(result.approval_mode, mode);
        assert_eq!(result.approval_policy, policy);
        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(updated.contains("personality = 'friendly' # keep style"));
        assert!(updated.ends_with(tail));
        assert!(!updated.contains("default_permissions"));
    }
}

#[test]
fn custom_and_inherited_permissions_never_get_reported_as_a_preset() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    for (source, mode) in [
        ("", "default"),
        ("personality = 'full-auto'\n", "default"),
        (
            "approval_policy = 'never'\nsandbox_mode = 'read-only'\n",
            "custom",
        ),
        (
            "approval_policy = {granular = {rules = false}}\nsandbox_mode = 'workspace-write'\n",
            "custom",
        ),
        (
            "approval_policy = 'on-request'\ndefault_permissions = ':workspace'\n",
            "workspace-write",
        ),
        (
            "approval_policy = 'on-request'\ndefault_permissions = 'mine'\n",
            "custom",
        ),
        (
            "approval_policy = 'on-request'\nsandbox_mode = 'workspace-write'\nprofile = 'mine'\n",
            "custom",
        ),
    ] {
        std::fs::write(&path, source).unwrap();
        assert_eq!(codex::read(&path).unwrap().approval_mode, mode);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
}

#[test]
fn repair_only_removes_invalid_legacy_style_on_explicit_permission_change() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    for style in ["suggest", "auto-edit", "full-auto"] {
        std::fs::write(&path, format!("personality = '{style}'\n")).unwrap();
        assert!(codex::read(&path).unwrap().legacy_personality);
        codex::write(&path, "reasoning_effort", "high", None).unwrap();
        assert!(codex::read(&path).unwrap().legacy_personality);
        let result = codex::write(&path, "approval_mode", "read-only", None).unwrap();
        assert!(!result.legacy_personality);
        assert!(!std::fs::read_to_string(&path)
            .unwrap()
            .contains("personality"));
    }
}

#[test]
fn context_toggle_preserves_other_limits_and_unchanged_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    assert!(codex::write(&path, "context_window_1m", "false", None)
        .unwrap()
        .context_window
        .is_none());
    assert!(!path.exists());
    let source = "model_context_window = 512_000 # user's choice\nmodel_auto_compact_token_limit = 480_000 # preserve\n";
    std::fs::write(&path, source).unwrap();
    let initial = codex::read(&path).unwrap();
    let unchanged = codex::write(
        &path,
        "context_window_1m",
        "false",
        Some(&initial.config_revision),
    )
    .unwrap();
    assert_eq!(initial.config_revision, unchanged.config_revision);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert!(
        codex::write(&path, "context_window_1m", "true", None)
            .unwrap()
            .context_window_1m
    );
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("model_auto_compact_token_limit = 480_000 # preserve"));
    assert!(codex::write(&path, "context_window_1m", "false", None)
        .unwrap()
        .context_window
        .is_none());
}

#[test]
fn invalid_values_fail_without_touching_or_creating_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    for (key, value) in [
        ("approval_mode", "full-auto"),
        ("reasoning_effort", "invented"),
        ("disable_response_storage", "TRUE"),
        ("context_window_1m", "yes"),
        ("keep-secret", "x"),
    ] {
        let error = codex::write(&path, key, value, None).unwrap_err();
        assert!(!error.contains("keep-secret"));
        assert!(!path.exists());
    }
}

#[test]
fn malformed_and_credential_bearing_errors_are_masked_and_preserved() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    for source in [
        "token = 'keep-secret",
        "model_context_window = 'keep-secret'\n",
        "approval_policy = 42\n",
        "disable_response_storage = 'keep-secret'\n",
        "sandbox_mode = []\n",
    ] {
        std::fs::write(&path, source).unwrap();
        let error = codex::read(&path).unwrap_err();
        assert!(!error.contains("keep-secret"));
        assert!(codex::write(&path, "reasoning_effort", "high", None).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
    std::fs::write(&path, [0xff]).unwrap();
    assert!(codex::read(&path).unwrap_err().contains("UTF-8"));
}

#[test]
fn selected_profile_cannot_be_silently_overridden_by_a_global_preset() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let source = "profile = 'mine'\npersonality = 'pragmatic'\n";
    std::fs::write(&path, source).unwrap();
    assert!(codex::write(&path, "approval_mode", "danger-full-access", None).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn path_resolution_is_strict_and_distinguishes_claude_mcp_location() {
    let root = tempfile::tempdir().unwrap();
    let conn = conn(root.path());
    assert_eq!(
        storage::config_path(&conn, "codex").unwrap(),
        root.path().join("config.toml")
    );
    conn.execute(
        "UPDATE custom_paths SET config_dir = NULL, mcp_config_path = ?1",
        [root.path().join("elsewhere.json").to_str().unwrap()],
    )
    .unwrap();
    assert_eq!(
        storage::config_path(&conn, "codex").unwrap(),
        root.path().join("config.toml")
    );
    conn.execute(
        "INSERT INTO custom_paths VALUES ('claude', ?1, ?2)",
        rusqlite::params![
            root.path().join("claude").to_str().unwrap(),
            root.path().join("mcp.json").to_str().unwrap()
        ],
    )
    .unwrap();
    assert_eq!(
        storage::config_path(&conn, "claude").unwrap(),
        root.path().join("claude/settings.json")
    );
    conn.execute_batch("DROP TABLE custom_paths").unwrap();
    assert!(storage::config_path(&conn, "codex").is_err());
}

#[test]
fn unchanged_windows_configuration_keeps_exact_bytes_and_timestamp() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.toml");
    let source = "model_reasoning_effort = 'high' # keep\r\n\r\n[model_providers.mine]\r\nexperimental_bearer_token = '''keep-secret'''\r\n";
    std::fs::write(&path, source).unwrap();
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let initial = codex::read(&path).unwrap();
    let saved = codex::write(
        &path,
        "reasoning_effort",
        "high",
        Some(&initial.config_revision),
    )
    .unwrap();
    assert_eq!(saved.config_revision, initial.config_revision);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
    let updated = codex::write(
        &path,
        "reasoning_effort",
        "low",
        Some(&saved.config_revision),
    )
    .unwrap();
    assert_eq!(updated.reasoning_effort, "low");
    let changed = std::fs::read_to_string(&path).unwrap();
    assert!(changed.contains("# keep\r\n"));
    assert!(changed.ends_with(
        "\r\n[model_providers.mine]\r\nexperimental_bearer_token = '''keep-secret'''\r\n"
    ));
    assert!(!changed.replace("\r\n", "").contains('\n'));
}

#[test]
fn non_regular_and_oversized_targets_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    assert!(codex::read(root.path()).is_err());
    let path = root.path().join("config.toml");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    assert!(codex::read(&path).unwrap_err().contains("size limit"));
}
