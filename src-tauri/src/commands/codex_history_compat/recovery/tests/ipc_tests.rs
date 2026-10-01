use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::Manager;

use crate::db::DbState;

#[test]
fn recovery_commands_report_configuration_failure_through_actual_camel_case_ipc() {
    let app = mock_builder()
        .manage(DbState(Mutex::new(
            rusqlite::Connection::open_in_memory().unwrap(),
        )))
        .invoke_handler(tauri::generate_handler![
            crate::commands::codex_history_compat::list_codex_history_migration_backups,
            crate::commands::codex_history_compat::preview_codex_history_restore,
            crate::commands::codex_history_compat::restore_codex_history_migration
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let state = app.state::<DbState>();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _guard = state.0.lock().unwrap();
        panic!("fixture configuration connection failure");
    }))
    .is_err());
    let window = tauri::WebviewWindowBuilder::new(&app, "fixture", Default::default())
        .build()
        .unwrap();
    for (command, body) in [
        ("list_codex_history_migration_backups", json!({})),
        (
            "preview_codex_history_restore",
            json!({"backupKey":"generation"}),
        ),
        (
            "restore_codex_history_migration",
            json!({"backupKey":"generation","expectedRevision":"checked","selectedKeys":["session"]}),
        ),
    ] {
        let result = tauri::test::get_ipc_response(
            &window,
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        );
        assert_eq!(
            result.err(),
            Some(json!("配置数据库当前不可用")),
            "{command}"
        );
    }
    // The failure precedes backup-directory resolution and file work.
    assert!(state.0.is_poisoned());
}
