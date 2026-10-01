use super::{log, old};
use crate::commands::codex_history_compat::preview_codex_history_migration;
use crate::db::DbState;
use serde_json::{json, Value};
use std::{
    future::Future,
    pin::pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
};
use tauri::{
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    Manager,
};

fn app(root: &std::path::Path) -> tauri::App<MockRuntime> {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO custom_paths(tool_id,config_dir) VALUES('codex',?1)",
        [root.to_string_lossy().as_ref()],
    )
    .unwrap();
    mock_builder()
        .manage(DbState(Mutex::new(conn)))
        .invoke_handler(tauri::generate_handler![
            crate::commands::codex_history_compat::preview_codex_history_migration
        ])
        .build(mock_context(noop_assets()))
        .unwrap()
}

#[test]
fn preview_ipc_returns_camel_case_counts_and_never_modifies_native_history() {
    let root = tempfile::tempdir().unwrap();
    let path = log(root.path(), "rollout.jsonl.zst");
    let before = std::fs::read(&path).unwrap();
    std::fs::write(
        root.path().join("config.toml"),
        "[model_providers.legacy]\nname='old'",
    )
    .unwrap();
    let app = app(root.path());
    let window = tauri::WebviewWindowBuilder::new(&app, "fixture", Default::default())
        .build()
        .unwrap();
    let result: Value = tauri::test::get_ipc_response(
        &window,
        tauri::webview::InvokeRequest {
            cmd: "preview_codex_history_migration".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(json!({"targetProviderId":"destination"})),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(result["sourceProviderIds"], json!(["legacy"]));
    assert_eq!(result["targetProviderId"], "destination");
    assert_eq!(result["jsonlFiles"], 1);
    assert_eq!(result["compressedFiles"], 1);
    assert_eq!(result["stateRows"], 0);
    assert_eq!(result["revision"].as_str().unwrap().len(), 64);
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
}

struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}

#[tokio::test]
async fn queued_preview_releases_configuration_lock_and_uses_current_root() {
    let previous = tempfile::tempdir().unwrap();
    log(previous.path(), "previous.jsonl");
    let current = tempfile::tempdir().unwrap();
    let path = log(current.path(), "current.jsonl");
    std::fs::write(
        &path,
        "{\"type\":\"session_meta\",\"payload\":{\"model_provider\":\"other\"}}\n",
    )
    .unwrap();
    old(&path);
    let app = app(previous.path());
    let state = app.state::<DbState>();
    let permit = crate::commands::extra_commands::session_file_tasks::mutation_permit().await;
    let mut waiting = pin!(preview_codex_history_migration(
        Some(vec!["legacy".into()]),
        None,
        state.clone()
    ));
    let waker = Waker::from(Arc::new(Noop));
    assert!(matches!(
        waiting.as_mut().poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    ));
    state
        .0
        .try_lock()
        .unwrap()
        .execute(
            "UPDATE custom_paths SET config_dir=?1 WHERE tool_id='codex'",
            [current.path().to_string_lossy().as_ref()],
        )
        .unwrap();
    drop(permit);
    let result = waiting.await.unwrap();
    assert_eq!(
        result.jsonl_files, 0,
        "a queued preview must not retain authority for the previous configured root"
    );
    assert!(previous.path().join("sessions/previous.jsonl").exists());
}
