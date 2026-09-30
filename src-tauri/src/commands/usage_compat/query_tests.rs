use super::*;
use tokio::{io::AsyncWriteExt, net::TcpListener};

#[test]
fn rejects_embedded_credentials_and_removes_query_secrets_from_candidates() {
    assert!(validate_base_url("https://user:secret@relay.test/v1").is_err());
    let base = validate_base_url("https://relay.test/v1?key=secret#private").unwrap();
    let urls = endpoint_candidates(&base, &["usage", "quota", "usage"]);
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0].as_str(), "https://relay.test/v1/usage");
}

#[tokio::test]
async fn probes_fallbacks_after_complete_unrecognized_responses() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1?key=secret", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for body in [
            "{}",
            "{\"data\":{\"balance\":\"3.5\",\"currency\":\"USD\"}}",
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            requests.push(crate::shared::usage_http::test_support::read_headers(&mut socket).await);
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
        requests
    });
    let result = query_usage(&base, "key", &["usage", "quota"])
        .await
        .unwrap();
    assert_eq!(result["success"], true);
    assert_eq!(result["data"][0]["remaining"], 3.5);
    let requests = task.await.unwrap();
    assert!(requests[0].starts_with("GET /v1/usage "));
    assert!(requests[1].starts_with("GET /v1/quota "));
    assert!(requests
        .iter()
        .all(|request| !request.contains("key=secret")));
}

#[tokio::test]
async fn preserves_transient_failure_if_no_fallback_succeeds() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        for status in [503, 404] {
            let (mut socket, _) = listener.accept().await.unwrap();
            crate::shared::usage_http::test_support::read_headers(&mut socket).await;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 {status} Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
    });
    let error = query_usage(&base, "key", &["usage", "quota"])
        .await
        .unwrap_err();
    assert_eq!(error, "Usage API returned HTTP 503");
    task.await.unwrap();
}
