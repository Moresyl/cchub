use super::*;

#[test]
fn native_document_import_accepts_comments_and_flat_maps_but_rejects_duplicate_or_broken_siblings()
{
    for text in [
        r#"{"mcpServers":{"one":{"command":"node"},"one":{"command":"other"}}}"#,
        r#"{"one":{"command":"node"},"two":{"command":"python","env":{"KEY":42}}}"#,
        r#"{"mcpServers":[]}"#,
    ] {
        let f = Fixture::new();
        let before = f.files();
        assert!(import_document(&f.conn, "claude", text, vec!["codex".into()]).is_err());
        assert_eq!(f.files(), before);
        assert!(list(&f.conn).unwrap().is_empty());
        assert!(!f.path("claude").parent().unwrap().exists());
    }
    for text in [
        "{// comment\n\"mcpServers\":{\"one\":{\"command\":\"node\",\"timeout\":42},}}",
        r#"{"one":{"command":"node","timeout":42}}"#,
    ] {
        let f = Fixture::new();
        let result = import_document(&f.conn, "claude", text, vec![]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &export(&f.conn, &result[0].server.id).unwrap()
            )
            .unwrap()["timeout"],
            42
        );
    }
}

fn entries() -> Vec<(String, NativeSpec)> {
    vec![
        ("first".into(), NativeSpec::Json(r#"{"command":"node","args":["one"],"enabled":false,"timeout":91,"extension":{"keep":true}}"#.into())),
        ("second".into(), NativeSpec::Json(r#"{"command":"python","args":["two"],"env":{"TOKEN":"fixture"}}"#.into())),
    ]
}

#[test]
fn batch_composes_two_servers_across_all_tools_and_retains_native_policy() {
    let f = Fixture::new();
    f.write(
        "claude",
        "{// retained comment\n\"mcpServers\":{\"unrelated\":{\"command\":\"keep\"}}}\n",
    );
    let installed = install_batch(
        &f.conn,
        "claude",
        entries(),
        TOOLS.iter().map(|tool| (*tool).into()).collect(),
    )
    .unwrap();
    assert_eq!(installed.len(), 2);
    assert_ne!(installed[0].server.id, installed[1].server.id);
    assert_eq!(installed[0].server.status, "disabled");
    assert_eq!(CatalogState::load(&f.conn).unwrap().projections.len(), 14);
    for tool in TOOLS {
        let view = crate::mcp::native_read::read_config(&f.conn, tool).unwrap();
        assert!(view.servers.contains_key("second"));
        assert_eq!(view.servers["first"]["timeout"], 91);
        assert_eq!(view.servers["first"]["extension"]["keep"], true);
    }
    assert!(std::fs::read_to_string(f.path("claude"))
        .unwrap()
        .contains("retained comment"));
    f.scan();
    assert_eq!(list(&f.conn).unwrap().len(), 3);
}

#[test]
fn late_invalid_duplicate_or_existing_entries_leave_every_file_and_row_untouched() {
    for case in 0..4 {
        let f = Fixture::new();
        f.write(
            "claude",
            r#"{"mcpServers":{"existing":{"command":"keep"}}}"#,
        );
        f.scan();
        let before = f.files();
        let catalog = f.catalog();
        let mut batch = entries();
        match case {
            0 => batch[1].1 = NativeSpec::Json(r#"{"command":"python","args":[42]}"#.into()),
            1 => batch[1].0 = "first".into(),
            2 => batch[1].0 = "existing".into(),
            _ => batch.clear(),
        }
        assert!(install_batch(&f.conn, "claude", batch, vec!["codex".into()]).is_err());
        assert_eq!(f.files(), before);
        assert_eq!(f.catalog(), catalog);
        assert_eq!(list(&f.conn).unwrap().len(), 1);
        assert!(!f.path("codex").parent().unwrap().exists());
    }
}

#[test]
fn actual_finalizer_failure_restores_the_entire_import_and_has_no_phantom_activity() {
    let f = Fixture::new();
    f.write("claude", "{// original\n\"mcpServers\":{}}\n");
    f.scan();
    let before = f.files();
    let catalog = f.catalog();
    let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let written = observed.clone();
    let paths: Vec<_> = TOOLS.iter().map(|tool| f.path(tool)).collect();
    f.conn.commit_hook(Some(move || {
        written.store(
            paths.iter().all(|path| {
                std::fs::read_to_string(path)
                    .is_ok_and(|text| text.contains("first") && text.contains("second"))
            }),
            std::sync::atomic::Ordering::SeqCst,
        );
        true
    }));
    assert!(install_batch(
        &f.conn,
        "claude",
        entries(),
        TOOLS.iter().map(|tool| (*tool).into()).collect()
    )
    .is_err());
    f.conn.commit_hook(None::<fn() -> bool>);
    assert!(observed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(f.files(), before);
    assert_eq!(f.catalog(), catalog);
    assert!(list(&f.conn).unwrap().is_empty());
    assert_eq!(
        f.conn
            .query_row("SELECT COUNT(*) FROM activity_logs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    for tool in &TOOLS[1..] {
        assert!(!f.path(tool).parent().unwrap().exists());
    }
}

#[test]
fn an_opencode_native_source_keeps_its_command_array_and_extensions() {
    let f = Fixture::new();
    let batch = vec![("same".into(), NativeSpec::Json(r#"{"type":"local","command":["node","server.js"],"environment":{"TOKEN":"fixture"},"enabled":false,"timeout":42}"#.into()))];
    let result = install_batch(&f.conn, "opencode", batch, vec!["claude".into()]).unwrap();
    assert_eq!(result[0].server.source, "opencode");
    let source = crate::mcp::native_read::read_config(&f.conn, "opencode").unwrap();
    assert_eq!(
        source.servers["same"]["command"],
        serde_json::json!(["node", "server.js"])
    );
    assert_eq!(source.servers["same"]["timeout"], 42);
    let projected = crate::mcp::native_read::read_config(&f.conn, "claude").unwrap();
    assert_eq!(projected.servers["same"]["command"], "node");
    assert_eq!(
        projected.servers["same"]["args"],
        serde_json::json!(["server.js"])
    );
}
