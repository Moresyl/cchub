use super::*;
use crate::mcp::operations;

#[test]
fn production_commands_keep_same_name_origins_independent() {
    let f = Fixture::new();
    f.write(
        "claude",
        r#"{"mcpServers":{"same":{"command":"node","timeout":42}}}"#,
    );
    f.write("gemini", r#"{"mcpServers":{"same":{"command":"python"}}}"#);
    operations::refresh(&f.conn).unwrap();
    let source = f.source("claude");
    let other = std::fs::read(f.path("gemini")).unwrap();
    let revision = view::revision(&source).unwrap();
    assert!(operations::remove(&f.conn, "same", None).is_err());
    operations::update(
        &f.conn,
        &source.id,
        "updated".into(),
        vec![],
        HashMap::new(),
        Some(&revision),
    )
    .unwrap();
    assert!(operations::update(
        &f.conn,
        &source.id,
        "stale".into(),
        vec![],
        HashMap::new(),
        Some(&revision)
    )
    .is_err());
    let exported: serde_json::Value =
        serde_json::from_str(&operations::export(&f.conn, &source.id).unwrap()).unwrap();
    assert_eq!(exported["timeout"], 42);
    operations::remove(&f.conn, &source.id, None).unwrap();
    assert_eq!(std::fs::read(f.path("gemini")).unwrap(), other);
}

#[test]
fn installing_into_selected_tool_does_not_touch_claude() {
    let f = Fixture::new();
    operations::install_for_tool(&f.conn, "gemini", "same".into(), config()).unwrap();
    assert!(!f.path("claude").exists());
    assert!(f.path("gemini").exists());
    let source = f.source("gemini");
    let statuses = operations::statuses(&f.conn, &[source.id.clone()]).unwrap();
    assert_eq!(statuses[&source.id]["gemini"].state, "source");
    assert_eq!(statuses[&source.id]["claude"].state, "missing");
}

#[test]
fn target_import_is_atomic_and_preserves_native_extensions() {
    let f = Fixture::new();
    let rows = operations::import_targets(
        &f.conn,
        "claude",
        r#"{"mcpServers":{"same":{"command":"node","timeout":42}}}"#,
        vec!["gemini".into(), "claude-desktop".into()],
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!f.path("claude").exists());
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.path("gemini")).unwrap()).unwrap();
    assert_eq!(value["mcpServers"]["same"]["timeout"], 42);
    let files = f.files();
    assert!(operations::import_targets(
        &f.conn,
        "claude",
        r#"{"mcpServers":{"new":{"command":"node"},"bad":{"command":42}}}"#,
        vec!["gemini".into(), "codex".into()]
    )
    .is_err());
    assert_eq!(f.files(), files);
}
