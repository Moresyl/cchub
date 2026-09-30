use super::*;
use std::time::Duration;
use tokio::{io::AsyncWriteExt, net::TcpListener};

async fn server(wire: String) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/usage?private=secret",
        listener.local_addr().unwrap()
    );
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = test_support::read_headers(&mut socket).await;
        let _ = socket.write_all(wire.as_bytes()).await;
        request
    });
    (url, handle)
}

async fn query(wire: String) -> Result<Result<Value, ResponseFailure>, String> {
    let (url, task) = server(wire).await;
    let result = request_json(
        &client().unwrap(),
        &url,
        "test-key",
        true,
        Instant::now() + Duration::from_secs(2),
    )
    .await;
    task.await.unwrap();
    result
}

#[test]
fn finite_numbers_and_official_origins_are_strict() {
    assert_eq!(
        finite_number(Some(&serde_json::json!(" 12.50 "))),
        Some(12.5)
    );
    for text in ["NaN", "inf", "-inf", "1e999", ""] {
        assert_eq!(finite_number(Some(&serde_json::json!(text))), None);
    }
    for url in [
        "http://api.deepseek.com",
        "https://user@api.deepseek.com",
        "https://api.deepseek.com:8443",
        "not a url",
    ] {
        assert!(official_url(url).is_none());
    }
    assert_eq!(
        official_url("https://API.DEEPSEEK.COM:443/v1")
            .unwrap()
            .host_str(),
        Some("api.deepseek.com")
    );
}

#[tokio::test]
async fn reads_json_and_preserves_auth_modes() {
    let wire = "HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\n{\"balance\":1}";
    let (url, task) = server(wire.into()).await;
    let result = request_json(
        &client().unwrap(),
        &url,
        "raw-key",
        false,
        Instant::now() + Duration::from_secs(2),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result["balance"], 1);
    let request = task.await.unwrap().to_ascii_lowercase();
    assert!(request.contains("authorization: raw-key"));
}

#[tokio::test]
async fn distinguishes_invalid_json_from_read_interruptions() {
    let invalid =
        query("HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad".into())
            .await
            .unwrap()
            .unwrap_err();
    assert_eq!(invalid.message, "Usage API returned invalid JSON");
    let truncated =
        query("HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{}".into())
            .await
            .unwrap_err();
    assert!(truncated.contains("Failed to read"));
    assert!(!truncated.contains("private=secret"));
}

#[tokio::test]
async fn rejects_oversized_declared_and_chunked_responses() {
    let declared = query(format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        MAX_USAGE_BYTES + 1
    ))
    .await
    .unwrap()
    .unwrap_err();
    assert!(declared.message.contains("2 MiB"));
    let body = "x".repeat(MAX_USAGE_BYTES + 1);
    let chunked = query(format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n", body.len(), body)).await.unwrap().unwrap_err();
    assert!(chunked.message.contains("2 MiB"));
}

#[tokio::test]
async fn classifies_status_before_reading_error_bodies() {
    for status in [401, 403, 404, 302] {
        let error = query(format!(
            "HTTP/1.1 {status} Error\r\nContent-Length: 999999999\r\nConnection: close\r\n\r\n"
        ))
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(error.message, format!("Usage API returned HTTP {status}"));
        assert_eq!(
            error.kind,
            FailureKind::Http(reqwest::StatusCode::from_u16(status).unwrap())
        );
    }
    for status in [429, 500, 503] {
        let error = query(format!(
            "HTTP/1.1 {status} Error\r\nContent-Length: 999999999\r\nConnection: close\r\n\r\n"
        ))
        .await
        .unwrap_err();
        assert_eq!(error, format!("Usage API returned HTTP {status}"));
    }
}

#[tokio::test]
async fn deadline_covers_a_stalled_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        test_support::read_headers(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n")
            .await
            .unwrap();
        std::future::pending::<()>().await;
    });
    let error = request_json(
        &client().unwrap(),
        &url,
        "key",
        true,
        Instant::now() + Duration::from_millis(100),
    )
    .await
    .unwrap_err();
    assert_eq!(error, "Usage query timed out");
    task.abort();
}

#[tokio::test]
async fn never_follows_another_origin_with_credentials() {
    let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (url, task) = server(format!("HTTP/1.1 302 Found\r\nLocation: http://{}/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", destination.local_addr().unwrap())).await;
    let error = request_json(
        &client().unwrap(),
        &url,
        "private-key",
        true,
        Instant::now() + Duration::from_secs(2),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(error.message, "Usage API returned HTTP 302");
    assert!(task.await.unwrap().contains("private-key"));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), destination.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn invalid_header_errors_do_not_echo_credentials_or_url_parameters() {
    let error = request_json(
        &client().unwrap(),
        "http://127.0.0.1:1/?secret=query-secret",
        "key-secret\ninvalid",
        true,
        Instant::now() + Duration::from_secs(1),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(!error.message.contains("key-secret"));
    assert!(!error.message.contains("query-secret"));
    assert_eq!(error.kind, FailureKind::InvalidRequest);
}
