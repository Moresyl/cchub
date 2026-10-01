use super::*;
use futures_util::StreamExt;

const START: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":7}}}\n\n";

async fn dynamic_server(body: String) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        let body = body.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(Body::from(body))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Upstream { url, hits, task }
}

async fn forward_path(app: &App<MockRuntime>, path: &str) -> Response<Body> {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/proxy/claude/{path}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"model":"fixture-model","stream":true,"messages":[]}).to_string(),
        ))
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        forward_proxy_request_with_client(
            app.handle().clone(),
            "claude".into(),
            path.into(),
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        ),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn native_protocol_short_streams_emit_errors_and_preserve_partial_usage_once() {
    for (path, start, marker) in [
        ("v1/messages", START, "event: error"),
        ("v1/responses", "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"r1\",\"usage\":{\"input_tokens\":7}}}\n\n", "event: response.failed"),
        ("v1/chat/completions", "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":null}],\"usage\":{\"prompt_tokens\":7}}\n\n", "api_error"),
        ("v1beta/models/gemini:streamGenerateContent", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hello\"}]}}],\"usageMetadata\":{\"promptTokenCount\":7}}\n\n", "UNAVAILABLE"),
    ] {
        let upstream = server(StatusCode::OK, "text/event-stream", start).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        open_profile(&app, "p1", true);
        let response = forward_path(&app, path).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 8192).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.starts_with(start), "{path}: {text}");
        assert!(text.contains(marker), "{path}: {text}");
        assert!(text.contains("p1:"));
        assert!(text.contains("connection lost"));
        assert!(!text.contains("event: message_stop"));
        assert!(!text.contains("event: response.completed"));
        assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 1);
        assert_eq!(profile(&app, "p1").consecutive_successes, 0);
        streaming_tests::assert_single_outcome(&app, 502, 0, 7);
    }
}

#[tokio::test]
async fn huge_terminal_events_are_unchanged_and_only_whole_streams_recover() {
    for complete in [true, false] {
        let data = "A".repeat(2 * 1024 * 1024);
        let tail = if complete { "\"}}\n\n" } else { "" };
        let body = format!("{START}event: message_stop\ndata: {{\"type\":\"message_stop\",\"image\":{{\"data\":\"{data}{tail}");
        let upstream = dynamic_server(body.clone()).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        open_profile(&app, "p1", true);
        let response = forward(app.handle().clone(), true).await;
        let bytes = tokio::time::timeout(
            Duration::from_secs(5),
            to_bytes(response.into_body(), 3 * 1024 * 1024),
        )
        .await
        .unwrap()
        .unwrap();
        if complete {
            assert_eq!(bytes.as_ref(), body.as_bytes());
            assert_eq!(profile(&app, "p1").consecutive_successes, 1);
            streaming_tests::assert_single_outcome(&app, 200, 1, 7);
        } else {
            let text = String::from_utf8(bytes.to_vec()).unwrap();
            assert!(text.starts_with(&body));
            assert!(text.contains("event: error"));
            assert_eq!(profile(&app, "p1").consecutive_successes, 0);
            streaming_tests::assert_single_outcome(&app, 502, 0, 7);
        }
    }
}

#[tokio::test]
async fn actual_transport_cut_after_received_usage_returns_one_error_and_no_success() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let release = Arc::new(Notify::new());
    let gate = release.clone();
    let router = Router::new().fallback(any(move || {
        let gate = gate.clone();
        async move {
            let body = async_stream::stream! {
                yield Ok::<_, std::io::Error>(bytes::Bytes::from_static(START.as_bytes()));
                gate.notified().await;
                yield Err(std::io::Error::other("private transport secret"));
            };
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(Body::from_stream(body))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let upstream = Upstream {
        url,
        hits: Arc::new(AtomicUsize::new(0)),
        task,
    };
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let response = forward(app.handle().clone(), true).await;
    let mut stream = response.into_body().into_data_stream();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !bytes.ends_with(b"\n\n") {
            bytes.extend_from_slice(&stream.next().await.unwrap().unwrap());
        }
    })
    .await
    .unwrap();
    assert_eq!(bytes, START.as_bytes());
    release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.unwrap());
        }
    })
    .await
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.matches("event: error").count(), 1);
    assert!(text.contains("connection lost"));
    assert!(!text.contains("private transport secret"));
    assert!(!text.contains("message_stop"));
    assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 1);
    streaming_tests::assert_single_outcome(&app, 502, 0, 7);
}
