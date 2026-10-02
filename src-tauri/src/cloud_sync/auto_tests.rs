use super::workflow_lock;
use crate::commands::extra_commands::{get_json_app_setting, set_json_app_setting};
use crate::db::DbState;
use crate::s3_sync::S3SyncSettings;
use crate::webdav_sync::WebDavSyncSettings;
use std::sync::Mutex;
use std::time::Duration;
use tauri::Manager;

async fn queued_decision(s3: bool, change: &str) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let account = uuid::Uuid::new_v4().to_string();
    let scope = crate::cloud_credentials::scope(
        if s3 { "s3_secret" } else { "webdav_password" },
        &url,
        &account,
    );
    let key = if s3 {
        "s3_sync_settings"
    } else {
        "webdav_sync_settings"
    };
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .unwrap();
    if s3 {
        let settings = S3SyncSettings {
            enabled: true,
            auto_sync: true,
            endpoint: url.clone(),
            bucket: "backup".into(),
            access_key_id: account.clone(),
            credential_scope: Some(scope.clone()),
            ..Default::default()
        };
        set_json_app_setting(&conn, key, &settings).unwrap();
    } else {
        let settings = WebDavSyncSettings {
            enabled: true,
            auto_sync: true,
            base_url: url.clone(),
            username: account.clone(),
            credential_scope: Some(scope.clone()),
            ..Default::default()
        };
        set_json_app_setting(&conn, key, &settings).unwrap();
    }
    let app = tauri::test::mock_builder()
        .manage(DbState(Mutex::new(conn)))
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let db = app.state::<DbState>();

    let mut quiet_listener = Some(listener);
    let server = if change == "limit" {
        let listener = quiet_listener.take().unwrap();
        Some(tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut wire = Vec::new();
            loop {
                let mut buffer = [0; 1024];
                let size = stream.read(&mut buffer).await.unwrap();
                assert!(size > 0);
                wire.extend_from_slice(&buffer[..size]);
                assert!(wire.len() < 16 * 1024);
                if wire.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            stream.write_all(b"HTTP/1.1 429 Limited\r\nRetry-After: 120\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
            String::from_utf8(wire).unwrap()
        }))
    } else {
        None
    };

    // The automatic task is polled first and queues behind an existing cloud
    // operation. Its decision must use changes made before that operation ends.
    let held = workflow_lock().lock().await;
    let queued = async {
        if s3 {
            crate::s3_sync::auto_upload(&db)
                .await
                .map(|info| info.is_some())
        } else {
            crate::webdav_sync::auto_upload(&db)
                .await
                .map(|info| info.is_some())
        }
    };
    let edit = async {
        if change == "limit" {
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let error = crate::cloud_http::send(client.get(&url), &scope)
                .await
                .unwrap_err();
            assert!(error.contains("HTTP 429"));
        } else {
            let conn = db.0.lock().unwrap();
            if s3 {
                let mut settings: S3SyncSettings =
                    get_json_app_setting(&conn, key).unwrap().unwrap();
                settings.auto_sync = false;
                if change == "account" {
                    settings.access_key_id = "new-account".into();
                }
                set_json_app_setting(&conn, key, &settings).unwrap();
            } else {
                let mut settings: WebDavSyncSettings =
                    get_json_app_setting(&conn, key).unwrap().unwrap();
                settings.auto_sync = false;
                if change == "account" {
                    settings.username = "new-account".into();
                }
                set_json_app_setting(&conn, key, &settings).unwrap();
            }
        }
        drop(held);
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(biased; queued, edit)
    })
    .await
    .unwrap();
    assert!(!result.unwrap(), "s3={s3}, change={change}");
    if let Some(listener) = quiet_listener {
        assert_eq!(
            listener.into_std().unwrap().accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    if let Some(server) = server {
        let wire = server.await.unwrap();
        assert!(wire.starts_with("GET "));
        assert!(crate::cloud_http::remaining(&scope).is_some());
    }
    let conn = db.0.lock().unwrap();
    let (enabled, auto_sync, last_error, last_sync_at, current_account) = if s3 {
        let current: S3SyncSettings = get_json_app_setting(&conn, key).unwrap().unwrap();
        (
            current.enabled,
            current.auto_sync,
            current.last_error,
            current.last_sync_at,
            current.access_key_id,
        )
    } else {
        let current: WebDavSyncSettings = get_json_app_setting(&conn, key).unwrap().unwrap();
        (
            current.enabled,
            current.auto_sync,
            current.last_error,
            current.last_sync_at,
            current.username,
        )
    };
    assert!(last_error.is_none());
    assert!(last_sync_at.is_none());
    assert!(enabled);
    assert_eq!(auto_sync, change == "limit");
    if change == "account" {
        assert_eq!(current_account, "new-account");
    } else {
        assert_eq!(current_account, account);
    }
}

#[tokio::test]
async fn queued_automatic_uploads_recheck_the_disabled_switch() {
    for s3 in [true, false] {
        queued_decision(s3, "disable").await;
    }
}

#[tokio::test]
async fn queued_automatic_uploads_use_the_current_accounts_switch() {
    for s3 in [true, false] {
        queued_decision(s3, "account").await;
    }
}

#[tokio::test]
async fn newly_rate_limited_automatic_tasks_do_not_send_requests_or_record_wait_errors() {
    for s3 in [true, false] {
        queued_decision(s3, "limit").await;
    }
}
