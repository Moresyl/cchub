use super::*;
use crate::cloud_revision::tests::{fixture, reply};

fn limited(status: u16) -> Vec<u8> {
    format!("HTTP/1.1 {status} Limited\r\nRetry-After: 120\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
}

fn settings(url: String) -> S3SyncSettings {
    S3SyncSettings {
        enabled: true,
        endpoint: url,
        bucket: "fixture-bucket".into(),
        access_key_id: "fixture-id".into(),
        secret_access_key: "fixture-secret".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn s3_limits_stop_reads_and_signed_conditional_writes_without_replaying() {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    for status in [429, 503] {
        for operation in ["download", "upload"] {
            let (url, worker) = fixture(vec![limited(status)]);
            let settings = settings(url);
            let key = object_key(&settings, MANIFEST_NAME);
            let result = if operation == "download" {
                get_object_with_headers_using(&client, &settings, &key)
                    .await
                    .map(|_| ())
            } else {
                put_object_using(
                    &client,
                    &settings,
                    &key,
                    b"sealed".to_vec(),
                    &WriteCondition::Absent,
                )
                .await
                .map(|_| ())
            };
            assert!(result.unwrap_err().contains(&format!("HTTP {status}")));
            let mut another = settings.clone();
            another.bucket = "another-bucket".into();
            another.profile = "other-profile".into();
            assert!(get_object_with_headers_using(
                &client,
                &another,
                &object_key(&another, MANIFEST_NAME)
            )
            .await
            .unwrap_err()
            .contains("后重试"));
            let requests = worker.join().unwrap();
            assert_eq!(requests.len(), 1);
            let headers = requests[0].to_ascii_lowercase();
            assert!(headers.contains("aws4-hmac-sha256"));
            if operation == "upload" {
                assert!(headers.contains("if-none-match: *"));
                assert!(headers
                    .contains("signedheaders=host;if-none-match;x-amz-content-sha256;x-amz-date"));
            }
        }
    }
}

#[tokio::test]
async fn s3_connection_tests_share_limits_but_another_access_key_still_works() {
    let (url, worker) = fixture(vec![limited(503), reply(404, None, b"")]);
    let mut account = settings(url.clone());
    account.proxy_url = Some(url);
    assert!(test_connection(account.clone(), None, false)
        .await
        .unwrap_err()
        .contains("HTTP 503"));
    assert!(test_connection(account.clone(), None, false)
        .await
        .unwrap_err()
        .contains("后重试"));
    let mut another = account.clone();
    another.access_key_id = "another-id".into();
    test_connection(another.clone(), None, false).await.unwrap();
    assert!(crate::cloud_http::remaining(&credential_scope(&another)).is_none());
    assert!(crate::cloud_http::remaining(&credential_scope(&account)).is_some());
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|wire| wire.starts_with("HEAD ")));
}
