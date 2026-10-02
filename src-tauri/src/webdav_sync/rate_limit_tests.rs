use super::*;
use crate::cloud_revision::tests::{fixture, reply};

fn limited(status: u16) -> Vec<u8> {
    format!("HTTP/1.1 {status} Limited\r\nRetry-After: 90\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
}

fn settings(url: String) -> WebDavSyncSettings {
    WebDavSyncSettings {
        enabled: true,
        base_url: url,
        username: "fixture-user".into(),
        password: "fixture-password".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn dav_limits_stop_manifest_fallback_directory_creation_and_conditional_writes() {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    for status in [429, 503] {
        for operation in ["manifest", "directories", "upload"] {
            let (url, worker) = fixture(vec![limited(status)]);
            let settings = settings(url);
            let result = match operation {
                "manifest" => fetch_manifest_with_fallback(&client, &settings)
                    .await
                    .map(|_| ()),
                "directories" => {
                    ensure_remote_directories(&client, &settings, WebDavRemoteLayout::Current).await
                }
                _ => upload_bytes(
                    &client,
                    &settings,
                    &format!("{}/snapshot", settings.base_url),
                    "application/octet-stream",
                    b"sealed".to_vec(),
                    &WriteCondition::Absent,
                )
                .await
                .map(|_| ()),
            };
            assert!(result.unwrap_err().contains(&format!("HTTP {status}")));
            assert!(crate::cloud_http::remaining(&credential_scope(&settings)).is_some());
            // Changing profiles within this account cannot bypass the wait.
            let mut another = settings.clone();
            another.profile = "other-profile".into();
            assert!(fetch_manifest_with_fallback(&client, &another)
                .await
                .unwrap_err()
                .contains("后重试"));
            let requests = worker.join().unwrap();
            assert_eq!(requests.len(), 1);
            let method = match operation {
                "manifest" => "GET",
                "directories" => "MKCOL",
                _ => "PUT",
            };
            assert!(requests[0].starts_with(method));
            if operation == "upload" {
                assert!(requests[0]
                    .to_ascii_lowercase()
                    .contains("if-none-match: *"));
            }
        }
    }
}

#[tokio::test]
async fn dav_connection_tests_share_limits_but_another_account_still_works() {
    let (url, worker) = fixture(vec![limited(429), reply(207, None, b"")]);
    let mut account = settings(url.clone());
    // Explicit local proxy keeps this fixture independent of system proxies.
    account.proxy_url = Some(url);
    assert!(test_connection(account.clone(), None, false)
        .await
        .unwrap_err()
        .contains("HTTP 429"));
    assert!(test_connection(account.clone(), None, false)
        .await
        .unwrap_err()
        .contains("后重试"));
    let mut another = account.clone();
    another.username = "different-user".into();
    test_connection(another.clone(), None, false).await.unwrap();
    assert!(crate::cloud_http::remaining(&credential_scope(&another)).is_none());
    assert!(crate::cloud_http::remaining(&credential_scope(&account)).is_some());
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|wire| wire.starts_with("PROPFIND ")));
}
