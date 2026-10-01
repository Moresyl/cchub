use super::*;
use axum::{body::Body, http::StatusCode, routing::post, Router};
use bytes::Bytes;

async fn verify(input: &str, sse: bool) -> Result<(), &'static str> {
    let chunks = input
        .as_bytes()
        .iter()
        .map(|byte| Ok::<_, std::io::Error>(Bytes::copy_from_slice(&[*byte])))
        .collect::<Vec<_>>();
    verify_response(futures_util::stream::iter(chunks), sse).await
}

#[tokio::test]
async fn accepts_complete_replies_from_all_four_model_protocols() {
    let fixtures = [
        r#"{"type":"message","id":"fixture","stop_reason":"end_turn","content":[{"type":"text","text":"你好"}]}"#,
        r#"{"choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}]}"#,
        r#"{"type":"response","id":"fixture","status":"completed","output":[]}"#,
        r#"{"candidates":[{"content":{"parts":[{"text":"OK"}]},"finishReason":"STOP"}]}"#,
    ];
    for fixture in fixtures {
        assert_eq!(verify(fixture, false).await, Ok(()), "{fixture}");
    }
    for fixture in [
        "event: message_start\r\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"fixture\"}}\r\n\r\nevent: message_stop\r\ndata: {\"type\":\"message_stop\"}\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"你好🦀\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n",
        "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"fixture\",\"status\":\"completed\",\"output\":[]}}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"OK\"}]},\"finishReason\":\"STOP\"}]}\n\n",
    ] {
        assert_eq!(verify(fixture, true).await, Ok(()));
    }
}

#[tokio::test]
async fn heartbeats_empty_chunks_or_partial_output_never_prove_model_health() {
    for fixture in [
        "",
        ": heartbeat\n\n",
        "data: [DONE]\n\n",
        "data: {}\n\n",
        "data: {\"choices\":[{\"delta\":{}}]}\n\ndata: [DONE]\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"unfinished\"}}]}\n\n",
    ] {
        assert!(verify(fixture, true).await.is_err(), "{fixture}");
    }
    for fixture in [
        "",
        "<html>Login required</html>",
        "{}",
        "{\"status\":\"ok\"}",
        "{\"choices\":[]}",
    ] {
        assert!(verify(fixture, false).await.is_err());
    }
}

#[tokio::test]
async fn errors_after_a_heartbeat_or_model_delta_are_failed_without_echoing_private_data() {
    for failure in [
        "event: error\ndata: {\"error\":\"private-credential\"}\n\n",
        "data: {\"error\":{\"message\":\"private-credential\"}}\n\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"private-credential\"}}}\n\n",
    ] {
        let input = format!(": heartbeat\n\ndata: {{\"choices\":[{{\"delta\":{{\"content\":\"OK\"}}}}]}}\n\n{failure}");
        let error = verify(&input, true).await.unwrap_err();
        assert!(!error.contains("private-credential"));
    }
    assert!(verify(r#"{"type":"response","id":"fixture","status":"completed","output":[],"error":{"message":"private-credential"}}"#, false).await.is_err());
}

#[tokio::test]
async fn malformed_unfinished_invalid_utf8_and_oversized_replies_fail() {
    for fixture in ["data: not json\n\n", "data: {\"choices\":[]}"] {
        assert!(verify(fixture, true).await.is_err());
    }
    let bytes = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(
        b"data: \xff\n\n",
    ))]);
    assert!(verify_response(bytes, true).await.is_err());
    for sse in [true, false] {
        let large = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from(vec![
            b'x';
            MAX_RESPONSE_BYTES
                + 1
        ]))]);
        assert!(verify_response(large, sse).await.is_err());
    }
}

#[tokio::test]
async fn actual_http_rejects_authentication_errors_and_preserves_request_credentials() {
    let route = post(
        |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| async move {
            assert_eq!(
                headers.get("authorization").unwrap(),
                "Bearer draft-fixture-key"
            );
            assert_eq!(headers.get("x-fixture").unwrap(), "draft-header");
            assert_eq!(body["model"], "draft-model");
            (
                StatusCode::UNAUTHORIZED,
                "private-credential returned by upstream",
            )
        },
    );
    let (url, task) = server(route).await;
    let outcome = execute(client(), request(url)).await;
    task.abort();
    assert_eq!(outcome.status, "error");
    assert_eq!(outcome.http_status, Some(401));
    assert!(!outcome.message.contains("private-credential"));
    assert!(outcome.message.contains("authentication"));
}

#[tokio::test]
async fn actual_http_checks_model_completion_instead_of_the_first_network_chunk() {
    for (body, expected) in [
        (": heartbeat\n\nevent: error\ndata: {\"error\":\"private-credential\"}\n\n", "error"),
        (": heartbeat\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\ndata: [DONE]\n\n", "healthy"),
    ] {
        let route = post(move || async move {
            let chunks = body.as_bytes().chunks(3).map(|bytes| Ok::<_, std::io::Error>(Bytes::copy_from_slice(bytes))).collect::<Vec<_>>();
            ([("content-type", "text/event-stream")], Body::from_stream(futures_util::stream::iter(chunks)))
        });
        let (url, task) = server(route).await;
        let outcome = execute(client(), request(url)).await;
        task.abort();
        assert_eq!(outcome.http_status, Some(200));
        assert_eq!(outcome.status, expected);
        assert!(outcome.latency_ms.is_some());
    }
}

#[tokio::test]
async fn actual_http_json_html_and_quota_responses_are_classified_correctly() {
    for (code, body, expected) in [
        (
            StatusCode::OK,
            r#"{"choices":[{"message":{"content":"OK"},"finish_reason":"stop"}]}"#,
            "healthy",
        ),
        (StatusCode::OK, "<html>please log in</html>", "error"),
        (StatusCode::TOO_MANY_REQUESTS, "private-credential", "error"),
    ] {
        let (url, task) = server(post(move || async move {
            (code, [("content-type", "application/json")], body)
        }))
        .await;
        let outcome = execute(client(), request(url)).await;
        task.abort();
        assert_eq!(outcome.http_status, Some(code.as_u16()));
        assert_eq!(outcome.status, expected);
        assert!(!outcome.message.contains("private-credential"));
    }
}

#[tokio::test]
async fn actual_http_transport_timeout_is_an_error() {
    let (url, task) = server(post(|| async {
        std::future::pending::<()>().await;
        "unreachable"
    }))
    .await;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    let outcome = execute(client, request(url)).await;
    task.abort();
    assert_eq!(outcome.status, "error");
    assert!(outcome.message.contains("timed out"));
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

fn request(endpoint: String) -> StreamCheckRequestSpec {
    StreamCheckRequestSpec {
        endpoint,
        headers: vec![
            ("authorization".into(), "Bearer draft-fixture-key".into()),
            ("x-fixture".into(), "draft-header".into()),
        ],
        body: serde_json::json!({"model":"draft-model","messages":[{"role":"user","content":"Reply with OK."}],"stream":true}),
    }
}

async fn server(route: axum::routing::MethodRouter) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/probe", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/probe", route))
            .await
            .unwrap();
    });
    (url, task)
}
