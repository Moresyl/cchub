use super::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

#[path = "ipc_tests.rs"]
mod ipc_tests;

fn conn(root: &Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    for tool in ["codex", "claude"] {
        conn.execute(
            "INSERT INTO custom_paths(tool_id,config_dir) VALUES(?1,?2)",
            rusqlite::params![tool, root.to_string_lossy()],
        )
        .unwrap();
    }
    conn
}

fn target(path: &Path) -> SessionDeleteTarget {
    SessionDeleteTarget {
        tool_id: "codex".into(),
        session_id: "same".into(),
        source_path: path.to_string_lossy().into(),
        source_backend: "jsonl".into(),
    }
}

#[test]
fn worker_resolves_current_twin_and_preserves_ownership_and_backend_restrictions() {
    let area = tempfile::tempdir().unwrap();
    let root = area.path().join("codex");
    std::fs::create_dir(&root).unwrap();
    let plain = root.join("rollout.jsonl");
    let packed = archive::twin(&plain).unwrap();
    let bytes = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"same\"}}\n";
    std::fs::write(&plain, bytes).unwrap();
    let conn = conn(&root);
    let plan = SessionDeletePlan::prepare(&conn, target(&plain)).unwrap();
    std::fs::write(&packed, zstd::stream::encode_all(&bytes[..], 0).unwrap()).unwrap();
    std::fs::remove_file(&plain).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&packed)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(1700000000))
        .unwrap();
    let trash = area.path().join("recovery");
    plan.execute_with_trash(|| Ok(trash.clone())).unwrap();
    assert!(!packed.exists());
    let saved = session_trash::list(&trash).unwrap();
    session_trash::restore(&trash, &saved[0].key, &[root.clone()]).unwrap();
    let mut unsupported = target(&packed);
    unsupported.source_backend = "unknown".into();
    assert!(SessionDeletePlan::prepare(&conn, unsupported)
        .unwrap()
        .execute()
        .unwrap_err()
        .contains("does not support"));
    let mut readonly = target(&packed);
    readonly.source_backend = "mcode_sqlite".into();
    assert!(SessionDeletePlan::prepare(&conn, readonly)
        .unwrap()
        .execute()
        .unwrap_err()
        .contains("read-only"));
    let outside = tempfile::NamedTempFile::new().unwrap();
    assert!(SessionDeletePlan::prepare(&conn, target(outside.path()))
        .unwrap()
        .execute()
        .unwrap_err()
        .contains("Invalid"));
    assert!(packed.exists() && outside.path().exists());
}

#[test]
fn owned_scan_captures_candidates_without_requiring_files_to_already_exist() {
    let area = tempfile::tempdir().unwrap();
    let root = area.path().join("later");
    let conn = conn(&root);
    let plan = super::super::session_scanners::prepare_session_scan(
        &conn,
        Some("codex".into()),
        None,
        None,
    )
    .unwrap();
    drop(conn);
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("session.jsonl"), concat!(
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"same\"}}\n",
        "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"owned scan\"}]}}\n"
    )).unwrap();
    let sessions = super::super::session_scanners::execute_session_scan(plan).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "same");
}

struct Release(Option<std::sync::mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[tokio::test]
async fn cancelled_delete_wait_keeps_worker_serialized_and_application_db_available() {
    let area = tempfile::tempdir().unwrap();
    let path = area.path().join("rollout.jsonl");
    std::fs::write(&path, "fixture").unwrap();
    let db = Arc::new(crate::db::DbState(Mutex::new(conn(area.path()))));
    let permit = mutation_permit().await;
    let plan = {
        let conn = db.0.lock().unwrap();
        let mut target = target(&path);
        target.tool_id = "claude".into();
        SessionDeletePlan::prepare(&conn, target).unwrap()
    };
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, hold) = std::sync::mpsc::channel();
    let release = Release(Some(release));
    let (finished, done) = tokio::sync::oneshot::channel();
    let job = tokio::spawn(mutate(permit, move || {
        let _ = entered.send(());
        hold.recv_timeout(Duration::from_secs(10))
            .map_err(|e| e.to_string())?;
        let result = plan.execute();
        let _ = finished.send(());
        result
    }));
    tokio::time::timeout(Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    assert!(
        db.0.try_lock().is_ok(),
        "file IO must not hold the configuration DB"
    );
    job.abort();
    assert!(job.await.unwrap_err().is_cancelled());
    assert!(
        MUTATIONS.try_lock().is_err(),
        "the live worker must retain serialization after cancellation"
    );
    let next = tokio::spawn(async {
        let permit = mutation_permit().await;
        mutate(permit, || Ok(())).await
    });
    assert!(!next.is_finished());
    drop(release);
    tokio::time::timeout(Duration::from_secs(5), done)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), next)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn cancelled_mutation_waiter_never_executes_and_worker_errors_release_the_gate() {
    let permit = mutation_permit().await;
    let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = ran.clone();
    let waiter = tokio::spawn(async move {
        let permit = mutation_permit().await;
        mutate(permit, move || {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await
    });
    tokio::task::yield_now().await;
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert_eq!(
        mutate(permit, || Err::<(), _>("fixture error".into()))
            .await
            .unwrap_err(),
        "fixture error"
    );
    assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
    let permit = mutation_permit().await;
    let error = mutate(permit, || -> Result<(), String> { panic!("fixture panic") })
        .await
        .unwrap_err();
    assert!(error.contains("Session file task failed"));
    assert!(
        !error.contains("fixture panic"),
        "panic payloads must not reach the UI"
    );
    let permit = mutation_permit().await;
    assert!(mutate(permit, || Ok(())).await.is_ok());
}

#[test]
fn configured_project_candidates_include_missing_paths_and_deduplicate_all_sources() {
    let area = tempfile::tempdir().unwrap();
    let missing = area.path().join("later-project");
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE workspaces(base_path TEXT); CREATE TABLE hooks(project_path TEXT); CREATE TABLE app_settings(key TEXT,value TEXT); CREATE TABLE custom_paths(tool_id TEXT,config_dir TEXT,mcp_config_path TEXT);").unwrap();
    let spelling = missing.to_string_lossy();
    conn.execute("INSERT INTO workspaces VALUES(?1)", [spelling.as_ref()])
        .unwrap();
    conn.execute("INSERT INTO hooks VALUES(?1)", [spelling.as_ref()])
        .unwrap();
    conn.execute(
        "INSERT INTO app_settings VALUES('known_project_roots',?1)",
        [serde_json::json!([spelling.as_ref(), "  "]).to_string()],
    )
    .unwrap();
    let configured = crate::commands::extra_commands::configured_project_roots(&conn);
    assert_eq!(configured, vec![missing.clone()]);
    assert!(crate::commands::extra_commands::discover_project_roots(&conn).is_empty());
    let candidates = session_root_candidates_for_tool(&conn, "codex").unwrap();
    assert!(candidates.contains(&missing.join(".codex")));
    std::fs::create_dir(&missing).unwrap();
    assert_eq!(
        crate::commands::extra_commands::discover_project_roots(&conn),
        vec![missing]
    );
}
