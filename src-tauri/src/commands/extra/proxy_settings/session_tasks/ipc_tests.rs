use super::{conn, target};
use crate::commands::extra_commands::{
    delete_session, get_session_detail, get_session_messages, get_sessions,
};
use crate::db::DbState;
use serde_json::{json, Value};
use std::{
    future::Future,
    pin::pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
    time::Duration,
};
use tauri::{
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    Manager,
};

fn app(root: &std::path::Path) -> tauri::App<MockRuntime> {
    mock_builder()
        .manage(DbState(Mutex::new(conn(root))))
        .invoke_handler(tauri::generate_handler![
            crate::commands::extra_commands::get_custom_paths,
            crate::commands::extra_commands::get_sessions,
            crate::commands::compat_commands::list_sessions,
            crate::commands::extra_commands::get_session_detail,
            crate::commands::extra_commands::get_session_messages,
            crate::commands::extra_commands::delete_session,
            crate::commands::extra_commands::delete_sessions,
            crate::commands::extra_commands::delete_sessions_checked
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

struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}

#[tokio::test]
async fn waiting_session_commands_release_db_and_configuration_ipc_remains_responsive() {
    let area = tempfile::tempdir().unwrap();
    let path = area.path().join("session.jsonl");
    std::fs::write(&path, "{}\n").unwrap();
    let app = app(area.path());
    let window = tauri::WebviewWindowBuilder::new(&app, "fixture", Default::default())
        .build()
        .unwrap();
    let mut permits = Vec::new();
    for _ in 0..4 {
        permits.push(super::super::READERS.acquire().await.unwrap());
    }
    let state = app.state::<DbState>();
    let mut list = pin!(get_sessions(
        Some("claude".into()),
        None,
        None,
        state.clone()
    ));
    let mut messages = pin!(get_session_messages(
        "claude".into(),
        path.to_string_lossy().into(),
        state.clone()
    ));
    let mut detail = pin!(get_session_detail(
        "claude".into(),
        "same".into(),
        path.to_string_lossy().into(),
        "jsonl".into(),
        "jsonl".into(),
        None,
        "Fixture".into(),
        "".into(),
        None,
        None,
        0,
        None,
        None,
        None,
        false,
        false,
        state.clone()
    ));
    let waker = Waker::from(Arc::new(Noop));
    let mut cx = Context::from_waker(&waker);
    assert!(matches!(list.as_mut().poll(&mut cx), Poll::Pending));
    assert!(matches!(messages.as_mut().poll(&mut cx), Poll::Pending));
    assert!(matches!(detail.as_mut().poll(&mut cx), Poll::Pending));
    assert!(state.0.try_lock().is_ok());
    let response = tokio::task::spawn_blocking(move || ipc(&window, "get_custom_paths", json!({})));
    let value = tokio::time::timeout(Duration::from_secs(5), response)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(value.as_array().unwrap().len(), 2);
    drop(permits);
    assert!(list.await.is_ok());
    assert!(messages.await.is_ok());
    assert!(detail.await.is_ok());
}

#[test]
fn async_session_ipc_preserves_formats_partial_failures_and_legacy_stop_on_error() {
    let area = tempfile::tempdir().unwrap();
    let path = area.path().join("session.jsonl");
    std::fs::write(
        &path,
        "{\"message\":{\"role\":\"user\",\"content\":\"Fixture message\"}}\n",
    )
    .unwrap();
    let app = app(area.path());
    let window = tauri::WebviewWindowBuilder::new(&app, "fixture", Default::default())
        .build()
        .unwrap();
    let listing = json!({"toolId":"claude","query":null,"limit":200});
    let sessions = ipc(&window, "get_sessions", listing.clone()).unwrap();
    assert_eq!(sessions.as_array().unwrap().len(), 1);
    assert_eq!(ipc(&window, "list_sessions", listing).unwrap(), sessions);
    let detail = ipc(
        &window,
        "get_session_detail",
        json!({
            "toolId":"claude", "sessionId":"same", "sourcePath":path,
            "sourceKind":"claude_jsonl", "sourceBackend":"jsonl", "cwd":null,
            "title":"Fixture", "preview":"", "createdAt":null, "updatedAt":null,
            "messageCount":0, "inputTokens":null, "outputTokens":null, "tokensUsed":null,
            "canResume":false, "canDelete":true
        }),
    )
    .unwrap();
    assert_eq!(
        detail["session"]["source_path"],
        path.to_string_lossy().as_ref()
    );
    assert!(!detail["entries"].as_array().unwrap().is_empty());
    let messages = ipc(
        &window,
        "get_session_messages",
        json!({"providerId":"claude","sourcePath":path}),
    )
    .unwrap();
    assert!(!messages.as_array().unwrap().is_empty());
    assert!(ipc(
        &window,
        "get_session_messages",
        json!({"providerId":"claude","sourcePath":area.path().join("../outside.jsonl")})
    )
    .unwrap_err()
    .as_str()
    .unwrap()
    .contains("Invalid"));
    let mut valid = target(&path);
    valid.tool_id = "claude".into();
    let mut missing = valid.clone();
    missing.source_path = area.path().join("missing.jsonl").to_string_lossy().into();
    let result = ipc(
        &window,
        "delete_sessions_checked",
        json!({"sessions":[missing.clone(), valid.clone(), valid.clone()]}),
    )
    .unwrap();
    assert_eq!(result["deleted"].as_array().unwrap().len(), 1);
    assert_eq!(result["failed"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["failed"][0]["target"]["sourcePath"],
        missing.source_path
    );
    assert!(!path.exists());
    std::fs::write(&path, "fixture").unwrap();
    assert!(ipc(
        &window,
        "delete_sessions",
        json!({"sessions":[missing, valid.clone()]})
    )
    .is_err());
    assert!(
        path.exists(),
        "the legacy endpoint still stops on its first failure"
    );
    assert_eq!(
        ipc(
            &window,
            "delete_session",
            serde_json::to_value(&valid).unwrap()
        )
        .unwrap(),
        Value::Null
    );
    assert!(!path.exists());
    std::fs::write(&path, "fixture").unwrap();
    assert_eq!(
        ipc(&window, "delete_sessions", json!({"sessions":[valid]})).unwrap(),
        1
    );
}

#[tokio::test]
async fn busy_recovery_worker_allows_configuration_ipc_and_cancelled_delete_does_not_run() {
    let area = tempfile::tempdir().unwrap();
    let root = area.path().join("codex");
    std::fs::create_dir(&root).unwrap();
    let path = root.join("rollout.jsonl");
    let bytes = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"same\"}}\n";
    std::fs::write(&path, bytes).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1700000000))
        .unwrap();
    let trash = area.path().join("recovery");
    let saved =
        super::super::session_trash::delete(&target(&path), &[root.clone()], &trash).unwrap();
    let app = app(&root);
    let window = tauri::WebviewWindowBuilder::new(&app, "fixture", Default::default())
        .build()
        .unwrap();
    let state = app.state::<DbState>();
    let permit = super::super::mutation_permit().await;
    let plan = {
        let conn = state.0.lock().unwrap();
        super::super::SessionRestorePlan::prepare(&conn, saved.key).unwrap()
    };
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, hold) = std::sync::mpsc::channel();
    let release = super::Release(Some(release));
    let job = tokio::spawn(super::super::mutate(permit, move || {
        let _ = entered.send(());
        hold.recv_timeout(Duration::from_secs(10))
            .map_err(|e| e.to_string())?;
        plan.execute(&trash)
    }));
    tokio::time::timeout(Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    {
        let mut waiting = pin!(delete_session(
            "codex".into(),
            "same".into(),
            path.to_string_lossy().into(),
            "jsonl".into(),
            state.clone()
        ));
        let waker = Waker::from(Arc::new(Noop));
        assert!(matches!(
            waiting.as_mut().poll(&mut Context::from_waker(&waker)),
            Poll::Pending
        ));
    }
    assert!(state.0.try_lock().is_ok());
    let response = tokio::task::spawn_blocking(move || ipc(&window, "get_custom_paths", json!({})));
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), response)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(!path.exists());
    drop(release);
    tokio::time::timeout(Duration::from_secs(5), job)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[tokio::test]
async fn queued_deletion_uses_configuration_at_its_turn_and_cannot_keep_old_root_authority() {
    let area = tempfile::tempdir().unwrap();
    let path = area.path().join("session.jsonl");
    std::fs::write(&path, "fixture").unwrap();
    let new_root = tempfile::tempdir().unwrap();
    let app = app(area.path());
    let state = app.state::<DbState>();
    let permit = super::super::mutation_permit().await;
    let mut waiting = pin!(delete_session(
        "claude".into(),
        "same".into(),
        path.to_string_lossy().into(),
        "jsonl".into(),
        state.clone()
    ));
    let waker = Waker::from(Arc::new(Noop));
    assert!(matches!(
        waiting.as_mut().poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    ));
    state
        .0
        .lock()
        .unwrap()
        .execute(
            "UPDATE custom_paths SET config_dir=?1 WHERE tool_id='claude'",
            [new_root.path().to_string_lossy().as_ref()],
        )
        .unwrap();
    drop(permit);
    assert!(waiting
        .await
        .unwrap_err()
        .contains("Invalid session source path"));
    assert!(path.exists());
}
