use super::*;
use crate::mcp::sources::SourceSnapshot;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TOOLS: [&str; 8] = [
    "claude",
    "claude-desktop",
    "codex",
    "gemini",
    "grokbuild",
    "opencode",
    "hermes",
    "mcode",
];

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
        for tool in TOOLS {
            let file = root.path().join(tool).join("native.conf");
            let dir = root.path().join(tool).join("settings");
            conn.execute(
                "INSERT INTO custom_paths(tool_id,mcp_config_path,config_dir) VALUES(?1,?2,?3)",
                params![tool, file.to_str().unwrap(), dir.to_str().unwrap()],
            )
            .unwrap();
        }
        Self { root, conn }
    }

    fn path(&self, tool: &str) -> PathBuf {
        native::configured_binding(&self.conn, tool).unwrap().path
    }

    fn write(&self, tool: &str, text: &str) {
        let path = self.path(tool);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn scan(&self) {
        prepare_refresh(&self.conn)
            .unwrap()
            .commit(&self.conn)
            .unwrap();
    }

    fn source(&self, tool: &str) -> NativeOrigin {
        CatalogState::load(&self.conn)
            .unwrap()
            .origins
            .into_values()
            .find(|origin| {
                origin.native_name == "same"
                    && origin.bindings.iter().any(|binding| binding.tool == tool)
            })
            .unwrap()
    }

    fn catalog(&self) -> String {
        self.conn
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                [STATE_KEY],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn files(&self) -> Vec<Option<Vec<u8>>> {
        TOOLS
            .iter()
            .map(|tool| crate::config_write::read(&self.path(tool)).unwrap())
            .collect()
    }

    fn alias(&self, tool: &str, path: &Path) {
        self.conn
            .execute(
                "UPDATE custom_paths SET mcp_config_path=?1 WHERE tool_id=?2",
                params![path.to_str().unwrap(), tool],
            )
            .unwrap();
    }
}

fn config() -> McpServerConfig {
    McpServerConfig {
        command: "node".into(),
        args: vec!["server.js".into()],
        env: HashMap::from([("TOKEN".into(), "fixture".into())]),
        transport_type: Some("stdio".into()),
    }
}

#[test]
fn independent_names_update_one_origin_and_delete_only_its_owned_copies() {
    let f = Fixture::new();
    f.write("claude", "\u{feff}{// source comment\r\n\"mcpServers\":{\"same\":{\"command\":\"old\",\"enabled\":false,\"timeout\":91,\"extension\":{\"keep\":true}},\"other\":{\"command\":\"stay\"}}}\r\n");
    f.write(
        "mcode",
        r#"{"mcpServers":{"same":{"command":"independent"}}}"#,
    );
    f.scan();
    let origin = f.source("claude");
    let independent = f.source("mcode");
    let other_bytes = std::fs::read(f.path("mcode")).unwrap();
    let revision = view::revision(&origin).unwrap();
    sync(&f.conn, &origin.id, "codex").unwrap();
    update(
        &f.conn,
        &origin.id,
        "new".into(),
        vec!["--mcp".into()],
        HashMap::new(),
        Some(&revision),
    )
    .unwrap();
    let current = f.source("claude");
    assert!(current.disabled);
    let exported: serde_json::Value =
        serde_json::from_str(&export(&f.conn, &origin.id).unwrap()).unwrap();
    assert_eq!(exported["command"], "new");
    assert_eq!(exported["timeout"], 91);
    assert_eq!(exported["extension"]["keep"], true);
    assert!(std::fs::read_to_string(f.path("claude"))
        .unwrap()
        .contains("// source comment"));
    assert_eq!(
        status(&f.conn, &origin.id).unwrap()["codex"].state,
        "linked"
    );
    assert_eq!(std::fs::read(f.path("mcode")).unwrap(), other_bytes);
    f.conn
        .execute(
            "INSERT INTO metrics(server_id,request_count) VALUES(?1,7)",
            [&origin.id],
        )
        .unwrap();
    uninstall(
        &f.conn,
        &origin.id,
        Some(&view::revision(&current).unwrap()),
    )
    .unwrap();
    assert_eq!(std::fs::read(f.path("mcode")).unwrap(), other_bytes);
    let remaining = list(&f.conn).unwrap();
    assert!(remaining.iter().any(|row| row.server.id == independent.id));
    assert!(!remaining.iter().any(|row| row.server.id == origin.id));
    assert_eq!(
        f.conn
            .query_row(
                "SELECT request_count FROM metrics WHERE server_id=?1",
                [&origin.id],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        7
    );
    assert!(SourceSnapshot::read_configured(&f.conn)
        .unwrap()
        .origins
        .iter()
        .any(|source| source.native_name == "other"));
}

#[test]
fn equal_unowned_target_is_never_adopted_or_removed() {
    let f = Fixture::new();
    for tool in ["claude", "mcode"] {
        f.write(tool, r#"{"mcpServers":{"same":{"command":"node"}}}"#);
    }
    f.scan();
    let origin = f.source("claude");
    let before = f.files();
    let catalog = f.catalog();
    assert_eq!(
        status(&f.conn, &origin.id).unwrap()["mcode"].state,
        "unowned"
    );
    assert!(sync(&f.conn, &origin.id, "mcode").is_err());
    assert!(unsync(&f.conn, &origin.id, "mcode").is_err());
    assert_eq!(f.catalog(), catalog);
    assert_eq!(f.files(), before);
    uninstall(&f.conn, &origin.id, None).unwrap();
    assert_eq!(
        crate::config_write::read(&f.path("mcode")).unwrap(),
        before[7]
    );
}

#[test]
fn modified_owned_copy_blocks_edits_and_delete_without_partial_changes() {
    let f = Fixture::new();
    let server = install(
        &f.conn,
        "same".into(),
        config(),
        vec!["gemini".into(), "codex".into()],
    )
    .unwrap();
    f.write(
        "gemini",
        r#"{"mcpServers":{"same":{"command":"external","env":{"PRIVATE":"fixture"}}}}"#,
    );
    let before = f.files();
    let catalog = f.catalog();
    assert_eq!(
        status(&f.conn, &server.server.id).unwrap()["gemini"].state,
        "conflict"
    );
    for error in [
        update(
            &f.conn,
            &server.server.id,
            "changed".into(),
            vec![],
            HashMap::new(),
            None,
        )
        .unwrap_err(),
        uninstall(&f.conn, &server.server.id, None).unwrap_err(),
        sync(&f.conn, &server.server.id, "gemini").unwrap_err(),
        unsync(&f.conn, &server.server.id, "gemini").unwrap_err(),
    ] {
        assert!(!error.contains("PRIVATE"));
    }
    assert_eq!(f.files(), before);
    assert_eq!(f.catalog(), catalog);
    f.scan();
    assert_eq!(CatalogState::load(&f.conn).unwrap().origins.len(), 1);
    assert_eq!(
        status(&f.conn, &server.server.id).unwrap()["gemini"].state,
        "conflict"
    );
}

#[test]
fn missing_owned_copy_requires_explicit_restore_or_unlink_before_edit() {
    let f = Fixture::new();
    let server = install(&f.conn, "same".into(), config(), vec!["codex".into()]).unwrap();
    std::fs::remove_file(f.path("codex")).unwrap();
    let before = f.files();
    assert!(update(
        &f.conn,
        &server.server.id,
        "changed".into(),
        vec![],
        HashMap::new(),
        None
    )
    .is_err());
    assert_eq!(f.files(), before);
    sync(&f.conn, &server.server.id, "codex").unwrap();
    assert_eq!(
        status(&f.conn, &server.server.id).unwrap()["codex"].state,
        "linked"
    );
    std::fs::remove_file(f.path("codex")).unwrap();
    unsync(&f.conn, &server.server.id, "codex").unwrap();
    update(
        &f.conn,
        &server.server.id,
        "changed".into(),
        vec![],
        HashMap::new(),
        None,
    )
    .unwrap();
    assert!(!f.path("codex").exists());
}

#[test]
fn all_targets_install_together_and_malformed_last_target_creates_nothing() {
    let f = Fixture::new();
    f.write("mcode", "{malformed");
    let before = f.files();
    assert!(install(
        &f.conn,
        "same".into(),
        config(),
        TOOLS.iter().map(|tool| (*tool).into()).collect()
    )
    .is_err());
    assert_eq!(f.files(), before);
    assert_eq!(
        f.conn
            .query_row("SELECT COUNT(*) FROM mcp_servers", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(!f.path("claude").parent().unwrap().exists());
    f.write("mcode", "{}");
    let result = install(
        &f.conn,
        "same".into(),
        config(),
        TOOLS.iter().map(|tool| (*tool).into()).collect(),
    )
    .unwrap();
    let statuses = status(&f.conn, &result.server.id).unwrap();
    assert_eq!(statuses["claude"].state, "source");
    assert!(TOOLS[1..]
        .iter()
        .all(|tool| statuses[*tool].state == "linked"));
    assert_eq!(CatalogState::load(&f.conn).unwrap().projections.len(), 7);
    f.scan();
    assert_eq!(list(&f.conn).unwrap().len(), 1);
}

#[test]
fn actual_sqlite_final_commit_failure_rolls_back_native_files_and_catalog() {
    let f = Fixture::new();
    f.write("claude", "{// preserve exact bytes\n\"mcpServers\":{}}\n");
    f.scan();
    let before = f.files();
    let catalog = f.catalog();
    let written = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = written.clone();
    let targets: Vec<_> = TOOLS.iter().map(|tool| f.path(tool)).collect();
    f.conn.commit_hook(Some(move || {
        observed.store(
            targets
                .iter()
                .all(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains("same"))),
            std::sync::atomic::Ordering::SeqCst,
        );
        true
    }));
    assert!(install(
        &f.conn,
        "same".into(),
        config(),
        TOOLS.iter().map(|tool| (*tool).into()).collect()
    )
    .is_err());
    f.conn.commit_hook(None::<fn() -> bool>);
    assert!(
        written.load(std::sync::atomic::Ordering::SeqCst),
        "the failing finalizer must see all native writes"
    );
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
    for tool in TOOLS[1..].iter() {
        assert!(
            !f.root.path().join(tool).exists(),
            "created parent remained for {tool}"
        );
    }
}

#[test]
fn source_remove_keeps_history_and_restore_is_explicit() {
    let f = Fixture::new();
    let server = install(&f.conn, "same".into(), config(), vec![]).unwrap();
    unsync(&f.conn, &server.server.id, "claude").unwrap();
    assert_eq!(list(&f.conn).unwrap()[0].server.status, "missing");
    assert_eq!(
        status(&f.conn, &server.server.id).unwrap()["claude"].state,
        "missing"
    );
    assert!(update(
        &f.conn,
        &server.server.id,
        "changed".into(),
        vec![],
        HashMap::new(),
        None
    )
    .is_err());
    sync(&f.conn, &server.server.id, "claude").unwrap();
    assert_eq!(
        status(&f.conn, &server.server.id).unwrap()["claude"].state,
        "source"
    );
    assert_eq!(list(&f.conn).unwrap()[0].server.status, "active");
}

#[test]
fn stale_form_revision_refuses_before_any_native_write() {
    let f = Fixture::new();
    let server = install(&f.conn, "same".into(), config(), vec!["codex".into()]).unwrap();
    let revision = server.origin.unwrap().revision;
    update(
        &f.conn,
        &server.server.id,
        "new".into(),
        vec![],
        HashMap::new(),
        Some(&revision),
    )
    .unwrap();
    let before = f.files();
    let catalog = f.catalog();
    assert!(update(
        &f.conn,
        &server.server.id,
        "stale".into(),
        vec![],
        HashMap::new(),
        Some(&revision)
    )
    .is_err());
    assert!(uninstall(&f.conn, &server.server.id, Some(&revision)).is_err());
    assert_eq!(f.files(), before);
    assert_eq!(f.catalog(), catalog);
}

#[test]
fn shared_target_aliases_are_linked_once_physically_and_unlinked_together() {
    let f = Fixture::new();
    let shared = f.path("mcode");
    f.alias("claude-desktop", &shared);
    let server = install(
        &f.conn,
        "same".into(),
        config(),
        vec!["mcode".into(), "claude-desktop".into()],
    )
    .unwrap();
    let statuses = status(&f.conn, &server.server.id).unwrap();
    assert_eq!(statuses["mcode"].state, "linked");
    assert_eq!(statuses["claude-desktop"].state, "linked");
    update(
        &f.conn,
        &server.server.id,
        "new".into(),
        vec![],
        HashMap::new(),
        None,
    )
    .unwrap();
    f.scan();
    assert_eq!(list(&f.conn).unwrap().len(), 1);
    unsync(&f.conn, &server.server.id, "mcode").unwrap();
    assert!(CatalogState::load(&f.conn).unwrap().projections.is_empty());
    assert_eq!(
        status(&f.conn, &server.server.id).unwrap()["claude-desktop"].state,
        "missing"
    );
}

#[test]
fn incompatible_source_alias_leaves_original_unchanged() {
    let f = Fixture::new();
    f.write(
        "claude",
        r#"{"mcpServers":{"same":{"url":"https://fixture.invalid/mcp"}}}"#,
    );
    f.scan();
    let origin = f.source("claude");
    f.alias("gemini", &f.path("claude"));
    let before = f.files();
    let catalog = f.catalog();
    assert!(sync(&f.conn, &origin.id, "gemini").is_err());
    assert_eq!(f.files(), before);
    assert_eq!(f.catalog(), catalog);
}

#[test]
fn uninstall_revokes_only_selected_source_access_and_history_remains() {
    let f = Fixture::new();
    let first = install(&f.conn, "same".into(), config(), vec![]).unwrap();
    let second = install(&f.conn, "other".into(), config(), vec![]).unwrap();
    let access = serde_json::to_string(&BTreeMap::from([
        (first.server.id.clone(), true),
        (second.server.id.clone(), false),
    ]))
    .unwrap();
    f.conn
        .execute(
            "INSERT INTO mcp_clients(id,name,server_access) VALUES('client','client',?1)",
            [&access],
        )
        .unwrap();
    uninstall(&f.conn, &first.server.id, None).unwrap();
    let access: String = f
        .conn
        .query_row("SELECT server_access FROM mcp_clients", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<BTreeMap<String, bool>>(&access).unwrap(),
        BTreeMap::from([(second.server.id, false)])
    );
    assert_eq!(
        f.conn
            .query_row(
                "SELECT COUNT(*) FROM activity_logs WHERE server_id=?1",
                [&first.server.id],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
}

#[test]
fn missing_source_cannot_be_reinstalled_over_existing_projection_ownership() {
    let f = Fixture::new();
    let first = install(&f.conn, "same".into(), config(), vec!["codex".into()]).unwrap();
    unsync(&f.conn, &first.server.id, "claude").unwrap();
    let before = f.files();
    let catalog = f.catalog();
    assert!(install(
        &f.conn,
        "same".into(),
        McpServerConfig {
            command: "replacement".into(),
            ..config()
        },
        vec![]
    )
    .is_err());
    assert_eq!(f.files(), before);
    assert_eq!(f.catalog(), catalog);
}

#[test]
fn malformed_client_access_stops_uninstall_before_files_change() {
    let f = Fixture::new();
    let server = install(&f.conn, "same".into(), config(), vec!["hermes".into()]).unwrap();
    f.conn
        .execute(
            "INSERT INTO mcp_clients(id,name,server_access) VALUES('client','client','{invalid')",
            [],
        )
        .unwrap();
    let before = f.files();
    let catalog = f.catalog();
    assert!(uninstall(&f.conn, &server.server.id, None).is_err());
    assert_eq!(f.files(), before);
    assert_eq!(f.catalog(), catalog);
    assert_eq!(list(&f.conn).unwrap().len(), 1);
}

#[test]
fn changed_shared_alias_blocks_physical_copy_removal_and_retains_links() {
    let f = Fixture::new();
    let shared = f.path("mcode");
    f.alias("claude-desktop", &shared);
    let server = install(
        &f.conn,
        "same".into(),
        config(),
        vec!["mcode".into(), "claude-desktop".into()],
    )
    .unwrap();
    let other = f.root.path().join("different-native.conf");
    std::fs::write(&other, std::fs::read(&shared).unwrap()).unwrap();
    f.alias("claude-desktop", &other);
    let before = f.files();
    let catalog = f.catalog();
    assert!(unsync(&f.conn, &server.server.id, "mcode").is_err());
    assert_eq!(f.files(), before);
    assert_eq!(f.catalog(), catalog);
    assert_eq!(CatalogState::load(&f.conn).unwrap().projections.len(), 2);
}
