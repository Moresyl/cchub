use super::super::sources::{SourceRole, SourceSnapshot};
use super::*;

fn fixture() -> (tempfile::TempDir, Connection, Vec<SourceBinding>) {
    let root = tempfile::tempdir().unwrap();
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    let first = root.path().join("claude.json");
    let second = root.path().join("other.json");
    std::fs::write(
        &first,
        r#"{"mcpServers":{"same":{"command":"one","enabled":false}}}"#,
    )
    .unwrap();
    std::fs::write(&second, r#"{"mcpServers":{"same":{"command":"two"}}}"#).unwrap();
    let bindings = vec![
        SourceBinding {
            tool: "claude".into(),
            path: first,
            role: SourceRole::Primary,
        },
        SourceBinding {
            tool: "mcode".into(),
            path: second,
            role: SourceRole::Primary,
        },
    ];
    (root, conn, bindings)
}

fn refresh(conn: &Connection, bindings: &[SourceBinding]) -> Result<Vec<McpServer>, String> {
    catalog::prepare_snapshot(conn, SourceSnapshot::read_bindings(bindings)?)?.commit(conn)
}

#[test]
fn complete_reconciliation_preserves_independent_sources_and_missing_history() {
    let (_root, conn, bindings) = fixture();
    let first = refresh(&conn, &bindings).unwrap();
    assert_eq!(first.len(), 2);
    assert_ne!(first[0].id, first[1].id);
    let missing_id = first
        .iter()
        .find(|row| row.command.as_deref() == Some("one"))
        .unwrap()
        .id
        .clone();
    assert_eq!(
        first
            .iter()
            .find(|row| row.id == missing_id)
            .unwrap()
            .status,
        "disabled"
    );
    conn.execute(
        "INSERT INTO metrics(server_id,request_count) VALUES(?1,9)",
        [&missing_id],
    )
    .unwrap();
    std::fs::remove_file(&bindings[0].path).unwrap();
    let rows = refresh(&conn, &bindings).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter().find(|row| row.id == missing_id).unwrap().status,
        "missing"
    );
    assert_eq!(
        conn.query_row(
            "SELECT request_count FROM metrics WHERE server_id=?1",
            [&missing_id],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        9
    );
    assert!(CatalogState::load(&conn)
        .unwrap()
        .origins
        .contains_key(&missing_id));
}

#[test]
fn unique_legacy_source_migrates_metadata_foreign_keys_and_client_selection() {
    let (_root, conn, bindings) = fixture();
    conn.execute("INSERT INTO mcp_servers(id,name,command,args,env,transport,source,config_path,package_name,version,installed_at) VALUES('same','same','one','[]','{}','stdio','local',?1,'retained-package','retained-version','original-install-time')", [bindings[0].path.to_str().unwrap()]).unwrap();
    conn.execute_batch("INSERT INTO metrics(server_id,request_count) VALUES('same',9); INSERT INTO activity_logs(server_id) VALUES('same'); INSERT INTO mcp_clients(id,name,server_access) VALUES('client','client','{\"same\":false}'); INSERT INTO update_history(item_type,item_id) VALUES('mcp','same');").unwrap();
    let rows = refresh(&conn, &bindings).unwrap();
    assert_eq!(rows.len(), 2);
    let row = rows
        .iter()
        .find(|row| row.command.as_deref() == Some("one"))
        .unwrap();
    assert_ne!(row.id, "same");
    assert_eq!(row.package_name.as_deref(), Some("retained-package"));
    assert_eq!(row.version.as_deref(), Some("retained-version"));
    assert_eq!(row.installed_at.as_deref(), Some("original-install-time"));
    assert_eq!(
        conn.query_row("SELECT server_id FROM metrics", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        row.id
    );
    assert_eq!(
        conn.query_row("SELECT server_id FROM activity_logs", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        row.id
    );
    assert_eq!(
        conn.query_row("SELECT item_id FROM update_history", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        row.id
    );
    let access: String = conn
        .query_row("SELECT server_access FROM mcp_clients", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<BTreeMap<String, bool>>(&access).unwrap(),
        BTreeMap::from([(row.id.clone(), false)])
    );
}

#[test]
fn ambiguous_or_unmatched_legacy_rows_remain_recoverable_and_unowned() {
    let (_root, conn, bindings) = fixture();
    conn.execute("INSERT INTO mcp_servers(id,name,command,args,env,transport,source) VALUES('same','same','one','[]','{}','stdio','local')", []).unwrap();
    let rows = refresh(&conn, &bindings).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter().find(|row| row.id == "same").unwrap().status,
        "conflict"
    );
    assert!(!CatalogState::load(&conn)
        .unwrap()
        .origins
        .contains_key("same"));
}

#[test]
fn sqlite_failure_and_external_change_preserve_previous_catalog_state() {
    let (_root, conn, bindings) = fixture();
    let before = refresh(&conn, &bindings).unwrap();
    let old: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [STATE_KEY],
            |row| row.get(0),
        )
        .unwrap();
    let prepared =
        catalog::prepare_snapshot(&conn, SourceSnapshot::read_bindings(&bindings).unwrap())
            .unwrap();
    conn.execute_batch("CREATE TRIGGER fail_catalog BEFORE UPDATE OF value ON app_settings WHEN NEW.key='mcp_native_catalog_v1' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(prepared.commit(&conn).is_err());
    assert_eq!(
        rows(&conn)
            .unwrap()
            .iter()
            .map(|row| &row.id)
            .collect::<Vec<_>>(),
        before.iter().map(|row| &row.id).collect::<Vec<_>>()
    );
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [STATE_KEY],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        old
    );
    let prepared =
        catalog::prepare_snapshot(&conn, SourceSnapshot::read_bindings(&bindings).unwrap())
            .unwrap();
    std::fs::write(&bindings[0].path, "{}").unwrap();
    assert!(prepared.commit(&conn).is_err());
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [STATE_KEY],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        old
    );
}

#[test]
fn corrupted_catalog_json_and_conflicting_client_selection_refuse_before_commit() {
    let (_root, conn, bindings) = fixture();
    conn.execute("INSERT INTO mcp_servers(id,name,command,args,env) VALUES('bad','bad','private-fixture','[1]','{}')", []).unwrap();
    let error = refresh(&conn, &bindings).unwrap_err();
    assert!(!error.contains("private-fixture"));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM mcp_servers", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    conn.execute("DELETE FROM mcp_servers", []).unwrap();
    let snapshot = SourceSnapshot::read_bindings(&bindings).unwrap();
    let next = snapshot
        .origins
        .iter()
        .find(|origin| origin.connection.command == "one")
        .unwrap()
        .id
        .clone();
    conn.execute("INSERT INTO mcp_servers(id,name,command,args,env,transport,source,config_path) VALUES('same','same','one','[]','{}','stdio','local',?1)", [bindings[0].path.to_str().unwrap()]).unwrap();
    let access =
        serde_json::to_string(&BTreeMap::from([("same".to_owned(), false), (next, true)])).unwrap();
    conn.execute(
        "INSERT INTO mcp_clients(id,name,server_access) VALUES('client','client',?1)",
        [&access],
    )
    .unwrap();
    assert!(catalog::prepare_snapshot(&conn, snapshot)
        .unwrap()
        .commit(&conn)
        .is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM mcp_servers", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT server_access FROM mcp_clients", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        access
    );
}

#[test]
fn invalid_retained_scopes_and_native_formats_cannot_be_persisted_or_loaded() {
    let (_root, conn, bindings) = fixture();
    refresh(&conn, &bindings).unwrap();
    let original: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [STATE_KEY],
            |row| row.get(0),
        )
        .unwrap();
    let mut variants = Vec::new();
    let mut state = CatalogState::load(&conn).unwrap();
    state.origins.values_mut().next().unwrap().spec =
        NativeSpec::Yaml("command: one\nenabled: false\n".into());
    variants.push(state);
    for mutation in 0..5 {
        let mut state = CatalogState::load(&conn).unwrap();
        let origin = state.origins.values().next().unwrap().clone();
        let mut projection = Projection {
            source_id: origin.id.clone(),
            binding: bindings[0].clone(),
            canonical_path: origin.canonical_path.clone(),
            container: "mcpServers".into(),
            native_name: origin.native_name.clone(),
            spec: origin.spec.clone(),
        };
        match mutation {
            0 => projection.container = "wrong".into(),
            1 => projection.binding.path = "relative.json".into(),
            2 => projection.canonical_path = "relative.json".into(),
            3 => projection.source_id = "orphan".into(),
            _ => projection.binding.role = SourceRole::Plugin,
        }
        state.projections.push(projection);
        variants.push(state);
    }
    for state in variants {
        assert!(state.save(&conn).is_err());
        assert_eq!(
            conn.query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                [STATE_KEY],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            original
        );
        let invalid = serde_json::to_string(&state).unwrap();
        conn.execute(
            "UPDATE app_settings SET value=?1 WHERE key=?2",
            params![invalid, STATE_KEY],
        )
        .unwrap();
        assert!(CatalogState::load(&conn).is_err());
        conn.execute(
            "UPDATE app_settings SET value=?1 WHERE key=?2",
            params![original, STATE_KEY],
        )
        .unwrap();
    }
}
