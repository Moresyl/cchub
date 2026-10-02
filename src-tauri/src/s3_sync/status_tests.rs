use super::*;
use crate::cloud_credentials::tests::MemoryStore;
use std::sync::Mutex;

fn connection() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .unwrap();
    conn
}

fn configured() -> S3SyncSettings {
    S3SyncSettings {
        enabled: true,
        endpoint: "https://s3.test/storage".into(),
        bucket: "backup".into(),
        access_key_id: "fixture-id".into(),
        secret_access_key: "fixture-secret".into(),
        backup_encryption: serde_json::from_value(
            serde_json::json!({"passphrase":"fixture-backup-password"}),
        )
        .unwrap(),
        ..Default::default()
    }
}

#[test]
fn delayed_errors_and_successes_cannot_modify_another_account_or_backup_location() {
    for field in ["endpoint", "account", "bucket", "region", "root", "profile"] {
        let conn = connection();
        let original = configured();
        let mut changed = original.clone();
        match field {
            "endpoint" => changed.endpoint = "https://different.test".into(),
            "account" => changed.access_key_id = "different-id".into(),
            "bucket" => changed.bucket = "different-bucket".into(),
            "region" => changed.region = "eu-west-1".into(),
            "root" => changed.remote_root = "another-root".into(),
            _ => changed.profile = "another-profile".into(),
        }
        changed.last_error = Some("current account error".into());
        changed.last_sync_at = Some("current account success".into());
        set_json_app_setting(&conn, SETTINGS_KEY, &changed.masked_for_frontend()).unwrap();
        let before: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                [SETTINGS_KEY],
                |row| row.get(0),
            )
            .unwrap();
        update_transfer_status(&conn, &original, None, Some("old request failed".into())).unwrap();
        update_upload_status(&conn, &original, "old request succeeded".into()).unwrap();
        let after: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                [SETTINGS_KEY],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(after, before, "changed {field}");
    }
}

#[test]
fn same_location_failure_preserves_success_time_concurrent_edits_and_rotated_credentials() {
    let conn = connection();
    let store = MemoryStore::default();
    let original = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    update_upload_status(&conn, &original, "last success".into()).unwrap();
    let mut edited = original.clone();
    edited.enabled = false;
    edited.auto_sync = true;
    edited.secret_access_key = "rotated-secret".into();
    edited.backup_encryption =
        serde_json::from_value(serde_json::json!({"passphrase":"rotated-backup-password"}))
            .unwrap();
    write_settings_with_store(&conn, edited, true, &store).unwrap();
    update_transfer_status(&conn, &original, None, Some("new transfer error".into())).unwrap();
    let current = read_settings_with_store(&conn, &store).unwrap();
    assert!(!current.enabled && current.auto_sync);
    assert_eq!(current.last_sync_at.as_deref(), Some("last success"));
    assert_eq!(current.last_error.as_deref(), Some("new transfer error"));
    assert_eq!(current.secret_access_key, "rotated-secret");
    assert_eq!(
        current.backup_encryption.passphrase.as_str(),
        "rotated-backup-password"
    );
    update_upload_status(&conn, &original, "recovered".into()).unwrap();
    let current = read_settings_with_store(&conn, &store).unwrap();
    assert_eq!(current.last_sync_at.as_deref(), Some("recovered"));
    assert!(current.last_error.is_none());
    assert!(!current.enabled && current.auto_sync);
}

#[test]
fn absent_settings_do_not_reappear_due_to_an_old_transfer() {
    let conn = connection();
    update_transfer_status(&conn, &configured(), None, Some("late error".into())).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM app_settings", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn finishing_a_failure_preserves_its_error_even_when_status_storage_fails() {
    let conn = connection();
    conn.execute_batch("DROP TABLE app_settings").unwrap();
    let db = DbState(Mutex::new(conn));
    assert_eq!(
        finish_transfer::<()>(&db, &configured(), Err("original failure".into())).unwrap_err(),
        "original failure"
    );
    assert_eq!(
        finish_transfer(&db, &configured(), Ok("completed")).unwrap(),
        "completed"
    );
}

#[tokio::test]
async fn actual_upload_and_restore_failures_record_only_their_original_location() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

    for upload in [true, false] {
        for changed in [true, false] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let (received, observed) = oneshot::channel();
            let (release, proceed) = oneshot::channel();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut wire = Vec::new();
                loop {
                    let mut chunk = [0; 2048];
                    let size = socket.read(&mut chunk).await.unwrap();
                    assert!(size > 0);
                    wire.extend_from_slice(&chunk[..size]);
                    assert!(wire.len() < 16 * 1024);
                    if wire.windows(4).any(|part| part == b"\r\n\r\n") {
                        break;
                    }
                }
                received.send(()).unwrap();
                proceed.await.unwrap();
                socket.write_all(b"HTTP/1.1 503 Limited\r\nRetry-After: 120\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                String::from_utf8(wire).unwrap()
            });
            let mut original = configured();
            original.endpoint = url.clone();
            original.proxy_url = Some(url);
            let conn = connection();
            set_json_app_setting(&conn, SETTINGS_KEY, &original.masked_for_frontend()).unwrap();
            let app = tauri::test::mock_builder()
                .manage(DbState(Mutex::new(conn)))
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .unwrap();
            let db = app.state::<DbState>();
            let operation = async {
                let result = if upload {
                    upload_inner(&db, &original, None).await.map(|_| ())
                } else {
                    download_inner(&db, &original, false).await.map(|_| ())
                };
                finish_transfer(&db, &original, result)
            };
            let edit = async {
                observed.await.unwrap();
                if changed {
                    let mut next = original.clone();
                    next.access_key_id = "another-account".into();
                    let conn = db.0.lock().unwrap();
                    set_json_app_setting(&conn, SETTINGS_KEY, &next.masked_for_frontend()).unwrap();
                }
                release.send(()).unwrap();
            };
            let (result, ()) = tokio::time::timeout(Duration::from_secs(10), async {
                tokio::join!(operation, edit)
            })
            .await
            .unwrap();
            assert!(result.unwrap_err().contains("HTTP 503"));
            let current: S3SyncSettings = get_json_app_setting(&db.0.lock().unwrap(), SETTINGS_KEY)
                .unwrap()
                .unwrap();
            assert_eq!(
                current.last_error.is_some(),
                !changed,
                "upload={upload}, changed={changed}"
            );
            if changed {
                assert_eq!(current.access_key_id, "another-account");
            }
            let wire = server.await.unwrap();
            assert!(wire.starts_with("GET "));
            assert!(wire.to_ascii_lowercase().contains("aws4-hmac-sha256"));
        }
    }
}
