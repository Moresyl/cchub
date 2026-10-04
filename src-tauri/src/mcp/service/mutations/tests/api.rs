use super::*;

#[test]
fn name_compatibility_never_chooses_between_distinct_sources() {
    let f = Fixture::new();
    f.write("claude", r#"{"mcpServers":{"same":{"command":"node"}}}"#);
    f.scan();
    let first = f.source("claude");
    assert_eq!(resolve_id(&f.conn, "same").unwrap(), first.id);
    f.write("gemini", r#"{"mcpServers":{"same":{"command":"python"}}}"#);
    f.scan();
    let second = f.source("gemini");
    let files = f.files();
    let state = f.catalog();
    assert!(resolve_id(&f.conn, "same")
        .unwrap_err()
        .contains("Multiple"));
    for source in [first, second] {
        assert_eq!(resolve_id(&f.conn, &source.id).unwrap(), source.id);
    }
    assert_eq!(f.files(), files);
    assert_eq!(f.catalog(), state);
}

#[test]
fn retained_unresolved_identity_is_not_reinterpreted_as_a_name() {
    let f = Fixture::new();
    f.write("claude", r#"{"mcpServers":{"same":{"command":"node"}}}"#);
    f.scan();
    f.conn.execute("INSERT INTO mcp_servers(id,name,command,status) VALUES('same','same','different','conflict')", []).unwrap();
    assert!(resolve_id(&f.conn, "same")
        .unwrap_err()
        .contains("unresolved"));
    assert!(resolve_id(&f.conn, "absent").is_err());
    let source = f.source("claude");
    assert_eq!(resolve_id(&f.conn, &source.id).unwrap(), source.id);
}

#[test]
fn batch_status_distinguishes_owned_unowned_disabled_missing_and_conflicting_entries() {
    let f = Fixture::new();
    f.write(
        "claude",
        r#"{"mcpServers":{"same":{"command":"node","enabled":false}}}"#,
    );
    f.write("gemini", r#"{"mcpServers":{"same":{"command":"python"}}}"#);
    f.scan();
    let first = f.source("claude");
    let second = f.source("gemini");
    sync(&f.conn, &first.id, "codex").unwrap();
    let files = f.files();
    let state = f.catalog();
    let all = statuses(
        &f.conn,
        &[first.id.clone(), second.id.clone(), first.id.clone()],
    )
    .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[&first.id].len(), 8);
    assert_eq!(all[&first.id]["claude"].state, "source");
    assert!(all[&first.id]["claude"].disabled);
    assert_eq!(all[&first.id]["codex"].state, "linked");
    assert!(all[&first.id]["codex"].disabled);
    assert_eq!(all[&first.id]["gemini"].state, "unowned");
    assert_eq!(all[&first.id]["hermes"].state, "missing");
    assert_eq!(all[&second.id]["gemini"].state, "source");
    assert_eq!(all[&second.id]["codex"].state, "unowned");
    assert_eq!(f.files(), files);
    assert_eq!(f.catalog(), state);
    f.write("codex", "[mcp_servers.same]\ncommand='external'\n");
    let changed = statuses(&f.conn, &[first.id.clone(), second.id.clone()]).unwrap();
    assert_eq!(changed[&first.id]["codex"].state, "conflict");
    assert_eq!(changed[&second.id]["codex"].state, "unowned");
    assert_eq!(changed[&second.id]["gemini"].state, "source");
}

#[test]
fn failed_batch_reads_never_return_false_states_or_mutate_catalog() {
    let f = Fixture::new();
    f.write("claude", r#"{"mcpServers":{"same":{"command":"node"}}}"#);
    f.scan();
    let origin = f.source("claude");
    let state = f.catalog();
    f.write("hermes", "mcp_servers: [broken]");
    let before = f.files();
    assert!(statuses(&f.conn, &[origin.id.clone()]).is_err());
    assert!(statuses(&f.conn, &[origin.id, "unknown-source".into()]).is_err());
    assert_eq!(f.files(), before);
    assert_eq!(f.catalog(), state);
    assert!(statuses(&f.conn, &[]).unwrap().is_empty());
}
