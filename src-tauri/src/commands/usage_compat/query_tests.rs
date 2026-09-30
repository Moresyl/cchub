use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

async fn finish_response(socket: &mut TcpStream) {
    socket.shutdown().await.unwrap();
    let mut tail = [0u8; 64];
    let count = tokio::time::timeout(std::time::Duration::from_secs(2), socket.read(&mut tail))
        .await
        .expect("fixture peer did not close after Connection: close")
        .unwrap();
    assert_eq!(
        count, 0,
        "fixture received unexpected bytes after complete GET headers"
    );
}

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
    // Repeated real connections expose Windows close races that a single run misses.
    for _ in 0..20 {
        fallback_case().await;
    }
}

async fn fallback_case() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1?key=secret", listener.local_addr().unwrap());
    let trace = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let server_trace = trace.clone();
    let mut task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for body in [
            "{}",
            "{\"data\":{\"balance\":\"3.5\",\"currency\":\"USD\"}}",
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = crate::shared::usage_http::test_support::read_headers(&mut socket).await;
            server_trace.lock().unwrap().push(request.clone());
            requests.push(request);
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
            finish_response(&mut socket).await;
        }
        requests
    });
    let result = query_usage(&base, "key", &["usage", "quota"]).await;
    let requests = match tokio::time::timeout(std::time::Duration::from_secs(3), &mut task).await {
        Ok(Ok(requests)) => requests,
        outcome => {
            task.abort();
            panic!("fixture failed to finish: {outcome:?}; query result: {result:?}; accepted requests: {:?}", trace.lock().unwrap());
        }
    };
    let result =
        result.unwrap_or_else(|error| panic!("{error}; accepted fixture requests: {requests:?}"));
    assert_eq!(result["success"], true);
    assert_eq!(result["data"][0]["remaining"], 3.5);
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
            finish_response(&mut socket).await;
        }
    });
    let error = query_usage(&base, "key", &["usage", "quota"])
        .await
        .unwrap_err();
    assert_eq!(error, "Usage API returned HTTP 503");
    task.await.unwrap();
}
