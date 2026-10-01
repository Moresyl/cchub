use super::*;

fn snapshot() -> String {
    serde_json::json!({
        "auth": {"OPENAI_API_KEY": "fixture-key"},
        "config": "model = 'new-model'\nmodel_reasoning_effort = 'high'\n[model_providers.custom]\nbase_url = 'https://fixture.test'\n"
    }).to_string()
}

#[test]
fn startup_overlay_preserves_omitted_effort_and_explicit_custom_levels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let original = snapshot();
    assert_eq!(
        overlay_codex_user_fields_into_snapshot(&original, &path),
        original
    );
    for effort in [None, Some("max"), Some("none"), Some("future-effort")] {
        let current = effort
            .map(|level| format!("model_reasoning_effort = '{level}'\n"))
            .unwrap_or_default();
        std::fs::write(&path, format!("model = 'old-model'\n{current}")).unwrap();
        let result = overlay_codex_user_fields_into_snapshot(&original, &path);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let doc = parsed["config"]
            .as_str()
            .unwrap()
            .parse::<toml_edit::DocumentMut>()
            .unwrap();
        assert_eq!(
            doc.get("model_reasoning_effort")
                .and_then(toml_edit::Item::as_str),
            effort
        );
        assert_eq!(doc["model"].as_str(), Some("new-model"));
        assert_eq!(parsed["auth"]["OPENAI_API_KEY"], "fixture-key");
    }
}

#[test]
fn actual_startup_reapply_and_explicit_switch_honor_different_effort_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE custom_paths (tool_id TEXT PRIMARY KEY, config_dir TEXT, mcp_config_path TEXT); CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT);").unwrap();
    conn.execute(
        "INSERT INTO custom_paths (tool_id, config_dir) VALUES ('codex', ?1)",
        [dir.path().to_str().unwrap()],
    )
    .unwrap();
    let path = dir.path().join("config.toml");
    let original = snapshot();
    std::fs::write(&path, "model = 'old-model'\n").unwrap();
    apply_tool_snapshot_with_options(&conn, "codex", &original, true).unwrap();
    let after_startup = std::fs::read_to_string(&path)
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    assert!(after_startup.get("model_reasoning_effort").is_none());
    assert_eq!(after_startup["model"].as_str(), Some("new-model"));
    apply_tool_snapshot_with_options(&conn, "codex", &original, false).unwrap();
    let switched = std::fs::read_to_string(&path)
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    assert_eq!(switched["model_reasoning_effort"].as_str(), Some("high"));
    let mut cleared: serde_json::Value = serde_json::from_str(&original).unwrap();
    cleared["config"] = "model = 'new-model'\n".into();
    apply_tool_snapshot_with_options(&conn, "codex", &cleared.to_string(), false).unwrap();
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap()
        .get("model_reasoning_effort")
        .is_none());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(dir.path().join("auth.json")).unwrap()
        )
        .unwrap()["OPENAI_API_KEY"],
        "fixture-key"
    );
}
