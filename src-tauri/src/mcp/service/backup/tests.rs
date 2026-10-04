use super::*;

struct Fixture {
    root: tempfile::TempDir,
    conn: Connection,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::run_migrations(&conn).unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
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
            conn.execute(
                "INSERT INTO custom_paths(tool_id,mcp_config_path,config_dir) VALUES(?1,?2,?3)",
                params![
                    tool,
                    root.path().join(tool).join("native.conf").to_str().unwrap(),
                    root.path().join(tool).join("settings").to_str().unwrap()
                ],
            )
            .unwrap();
        }
        Self { root, conn }
    }
    fn path(&self, tool: &str) -> std::path::PathBuf {
        native::configured_binding(&self.conn, tool).unwrap().path
    }
    fn install(&self, name: &str) -> String {
        let text = serde_json::json!({"mcpServers":{name:{"command":"node", "args":["entry.js"], "timeout":42, "disabled":true,"custom":{"retained":true}}}}).to_string();
        import_document(&self.conn, "claude", &text, vec!["gemini".into()]).unwrap()[0]
            .server
            .id
            .clone()
    }
    fn history(&self, id: &str, count: i64) {
        self.conn
            .execute(
                "INSERT INTO metrics(server_id,request_count) VALUES(?1,?2)",
                params![id, count],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO update_history(item_type,item_id,new_version) VALUES('mcp',?1,?2)",
                params![id, count.to_string()],
            )
            .unwrap();
        self.conn.execute("INSERT INTO mcp_clients(id,name,config_path,server_access) VALUES('client','Client',?1,?2)",params![self.root.path().join("client.json").to_str().unwrap(),serde_json::json!({id:true}).to_string()]).unwrap();
    }
}

#[test]
fn foreign_backup_keeps_full_library_but_only_local_ownership_clients_and_history() {
    let live = Fixture::new();
    let remote = Fixture::new();
    let local_id = live.install("local");
    let remote_id = remote.install("remote");
    live.history(&local_id, 7);
    remote.history(&remote_id, 11);
    let foreign_bytes = std::fs::read(remote.path("claude")).unwrap();
    let local_bytes = std::fs::read(live.path("claude")).unwrap();
    let library = prepare_backup_library(&remote.conn).unwrap();
    // Cloud preparation has already retained local custom paths/settings.
    remote.conn.execute("DELETE FROM custom_paths", []).unwrap();
    copy_rows(
        &live.conn,
        &remote.conn,
        "custom_paths",
        "tool_id,config_dir,mcp_config_path,skills_dir",
        true,
        "",
    )
    .unwrap();
    restore_backup_library(&live.conn, &remote.conn, library).unwrap();
    let state = CatalogState::load(&remote.conn).unwrap();
    assert_eq!(state.origins.len(), 1);
    assert!(state.origins.contains_key(&local_id));
    assert_eq!(state.projections.len(), 1);
    assert_eq!(state.projections[0].source_id, local_id);
    let archived_id = state.archived.keys().next().unwrap().clone();
    assert_eq!(state.archived.len(), 1);
    assert_eq!(
        state.archived[&archived_id].spec.to_json().unwrap()["timeout"],
        42
    );
    assert_eq!(
        remote
            .conn
            .query_row(
                "SELECT server_id FROM metrics WHERE request_count=11",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        archived_id
    );
    assert_eq!(
        remote
            .conn
            .query_row(
                "SELECT server_id FROM metrics WHERE request_count=7",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        local_id
    );
    assert_eq!(
        remote
            .conn
            .query_row("SELECT COUNT(*) FROM update_history", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    let access: String = remote
        .conn
        .query_row("SELECT server_access FROM mcp_clients", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&access).unwrap(),
        serde_json::json!({local_id:true})
    );
    assert_eq!(std::fs::read(live.path("claude")).unwrap(), local_bytes);
    let foreign_path = remote.root.path().join("claude").join("native.conf");
    assert_eq!(std::fs::read(&foreign_path).unwrap(), foreign_bytes);
    prepare_refresh(&remote.conn)
        .unwrap()
        .commit(&remote.conn)
        .unwrap();
    assert_eq!(
        list(&remote.conn)
            .unwrap()
            .iter()
            .find(|row| row.server.id == archived_id)
            .unwrap()
            .server
            .status,
        "archived"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&export(&remote.conn, &archived_id).unwrap())
            .unwrap()["custom"]["retained"],
        true
    );
    assert!(sync(&remote.conn, &archived_id, "codex").is_err());
    sync(&remote.conn, &archived_id, "claude").unwrap();
    let restored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(live.path("claude")).unwrap()).unwrap();
    assert_eq!(restored["mcpServers"]["remote"]["timeout"], 42);
    assert_eq!(restored["mcpServers"]["remote"]["disabled"], true);
    assert!(restored["mcpServers"].get("local").is_some());
    assert!(CatalogState::load(&remote.conn)
        .unwrap()
        .archived
        .is_empty());
    let migrated: String = remote
        .conn
        .query_row(
            "SELECT server_id FROM metrics WHERE request_count=11",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(migrated, archived_id);
    assert_ne!(migrated, remote_id);
    assert_eq!(std::fs::read(foreign_path).unwrap(), foreign_bytes);
}

#[test]
fn library_restore_refuses_existing_entry_and_rolls_back_native_files_on_database_failure() {
    let live = Fixture::new();
    let remote = Fixture::new();
    remote.install("remote");
    let library = prepare_backup_library(&remote.conn).unwrap();
    remote.conn.execute("DELETE FROM custom_paths", []).unwrap();
    copy_rows(
        &live.conn,
        &remote.conn,
        "custom_paths",
        "tool_id,config_dir,mcp_config_path,skills_dir",
        true,
        "",
    )
    .unwrap();
    restore_backup_library(&live.conn, &remote.conn, library).unwrap();
    let id = CatalogState::load(&remote.conn)
        .unwrap()
        .archived
        .keys()
        .next()
        .unwrap()
        .clone();
    let path = live.path("claude");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = br#"{"mcpServers":{"remote":{"command":"independent"}}}"#;
    std::fs::write(&path, bytes).unwrap();
    assert!(sync(&remote.conn, &id, "claude").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::write(&path, b"{\"mcpServers\":{}}").unwrap();
    let original = std::fs::read(&path).unwrap();
    remote.conn.commit_hook(Some(|| true));
    assert!(sync(&remote.conn, &id, "claude").is_err());
    remote.conn.commit_hook(None::<fn() -> bool>);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(CatalogState::load(&remote.conn)
        .unwrap()
        .archived
        .contains_key(&id));
    uninstall(&remote.conn, &id, None).unwrap();
    assert!(CatalogState::load(&remote.conn)
        .unwrap()
        .archived
        .is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn portable_library_retains_native_toml_and_yaml_extension_types() {
    for (tool, spec) in [
        (
            "codex",
            NativeSpec::Toml(
                "command = 'node'\nstamp = 1979-05-27T07:32:00Z\ncustom_float = nan\n".into(),
            ),
        ),
        (
            "hermes",
            NativeSpec::Yaml(
                "command: node\ncustom_float: .nan\ncustom_tag: !retained value\n".into(),
            ),
        ),
    ] {
        let live = Fixture::new();
        let remote = Fixture::new();
        install_batch(
            &remote.conn,
            tool,
            vec![("typed".into(), spec.clone())],
            vec![],
        )
        .unwrap();
        let library = prepare_backup_library(&remote.conn).unwrap();
        remote.conn.execute("DELETE FROM custom_paths", []).unwrap();
        copy_rows(
            &live.conn,
            &remote.conn,
            "custom_paths",
            "tool_id,config_dir,mcp_config_path,skills_dir",
            true,
            "",
        )
        .unwrap();
        restore_backup_library(&live.conn, &remote.conn, library).unwrap();
        let state = CatalogState::load(&remote.conn).unwrap();
        let id = state.archived.keys().next().unwrap();
        assert!(state.archived[id].spec.same(&spec).unwrap());
        sync(&remote.conn, id, tool).unwrap();
        let state = CatalogState::load(&remote.conn).unwrap();
        assert!(state.archived.is_empty());
        assert!(state
            .origins
            .values()
            .next()
            .unwrap()
            .spec
            .same(&spec)
            .unwrap());
    }
}

#[test]
fn malformed_backup_catalog_is_rejected_before_replacing_any_local_state() {
    let live = Fixture::new();
    let remote = Fixture::new();
    let id = live.install("local");
    let before = std::fs::read(live.path("claude")).unwrap();
    remote
        .conn
        .execute(
            "INSERT INTO app_settings(key,value) VALUES(?1,'{broken')",
            [STATE_KEY],
        )
        .unwrap();
    assert!(prepare_backup_library(&remote.conn).is_err());
    assert!(CatalogState::load(&live.conn)
        .unwrap()
        .origins
        .contains_key(&id));
    assert_eq!(std::fs::read(live.path("claude")).unwrap(), before);
}

#[test]
fn archived_display_rows_are_rebuilt_from_the_complete_definition() {
    let live = Fixture::new();
    let remote = Fixture::new();
    let id = "mcp-archive-fixture";
    let mut state = CatalogState::default();
    state.archived.insert(
        id.into(),
        ArchivedSource {
            name: "retained".into(),
            tool: "claude".into(),
            spec: NativeSpec::Json(r#"{"command":"node","args":["actual.js"]}"#.into()),
        },
    );
    state.save(&remote.conn).unwrap();
    remote.conn.execute("INSERT INTO mcp_servers(id,name,command,source,config_path) VALUES(?1,'stale','wrong','other','foreign')",[id]).unwrap();
    let library = prepare_backup_library(&remote.conn).unwrap();
    restore_backup_library(&live.conn, &remote.conn, library).unwrap();
    let item = list(&remote.conn).unwrap().pop().unwrap();
    assert_eq!(item.server.name, "retained");
    assert_eq!(item.server.command.as_deref(), Some("node"));
    assert_eq!(item.server.args, r#"["actual.js"]"#);
    assert_eq!(item.server.source, "claude");
    assert!(item.server.config_path.is_none());
    assert!(item.origin.unwrap().bindings.is_empty());
}
