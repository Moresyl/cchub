use super::*;
use bytes::Bytes;
use futures_util::StreamExt;

#[path = "keepalive_tests/failures.rs"]
mod failures;

const PING: &str = ": provider-alive\n\n";

fn frames(format: &str) -> (&'static str, &'static str) {
    match format {
        "openai_chat" => (
            "data: {\"id\":\"chat-fixture\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{\"content\":\"first\"}}],\"usage\":{\"prompt_tokens\":7}}\n\n",
            "data: {\"id\":\"chat-fixture\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":3}}\n\ndata: [DONE]\n\n",
        ),
        "openai_responses" => (
            "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-fixture\",\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":7}}}\n\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"first\"}\n\n",
            "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":7,\"output_tokens\":3}}}\n\n",
        ),
        "gemini_native" => (
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"first\"}]}}],\"usageMetadata\":{\"promptTokenCount\":7}}\n\n",
            "data: {\"candidates\":[{\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":7,\"candidatesTokenCount\":3}}\n\n",
        ),
        _ => panic!("unknown fixture format"),
    }
}

async fn active_server(first: &'static str, last: &'static str, pending: bool) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            let stream = async_stream::stream! {
                yield Ok::<_, std::io::Error>(Bytes::from_static(first.as_bytes()));
                for _ in 0..10 {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    yield Ok(Bytes::from_static(PING.as_bytes()));
                }
                if pending { std::future::pending::<()>().await; }
                yield Ok(Bytes::from_static(last.as_bytes()));
            };
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(Body::from_stream(stream))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Upstream { url, hits, task }
}

async fn proxy_server(app: AppHandle<MockRuntime>) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let app = app.clone();
        let observed = observed.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            forward_proxy_request_with_client(
                app,
                "claude".into(),
                "v1/messages".into(),
                request,
                Some(reqwest::Client::builder().no_proxy().build().unwrap()),
            )
            .await
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Upstream { url, hits, task }
}

async fn client(proxy: &Upstream) -> reqwest::Response {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .post(format!("{}/v1/messages", proxy.url))
        .json(&json!({"model":"fixture-model","stream":true,"messages":[]}))
        .send()
        .await
        .unwrap()
}

// Count only complete data events, as an SDK watchdog does; arbitrary HTTP
// bytes and SSE comments must not satisfy the client's idle deadline.
async fn read_with_watchdog(response: reqwest::Response) -> String {
    read_with_watchdog_budget(response, Duration::from_millis(1400)).await
}

async fn read_with_watchdog_budget(response: reqwest::Response, idle: Duration) -> String {
    assert_eq!(response.status(), StatusCode::OK);
    let mut stream = response.bytes_stream();
    let mut pending = String::new();
    let mut all = String::new();
    let mut deadline = tokio::time::Instant::now() + idle;
    while let Some(chunk) = tokio::time::timeout_at(deadline, stream.next())
        .await
        .expect("client timed out despite upstream activity")
    {
        let chunk = chunk.unwrap();
        let text = std::str::from_utf8(&chunk).unwrap();
        pending.push_str(text);
        all.push_str(text);
        while let Some(end) = pending.find("\n\n") {
            let frame = pending.drain(..end + 2).collect::<String>();
            if crate::provider_proxy_transform::stream_frames::event_data(&frame).is_some() {
                deadline = tokio::time::Instant::now() + idle;
            }
        }
    }
    all
}

#[tokio::test]
async fn translated_keepalive_liveness_reaches_an_actual_http_client() {
    for format in ["openai_chat", "openai_responses", "gemini_native"] {
        let (first, last) = frames(format);
        let upstream = active_server(first, last, false).await;
        let app = app(
            &[("p1", &upstream.url, vec![])],
            OptimizerConfig {
                streaming_idle_timeout: 1,
                ..Default::default()
            },
        );
        set_format(&app, "p1", format);
        open_profile(&app, "p1", true);
        let proxy = proxy_server(app.handle().clone()).await;
        let body = read_with_watchdog(client(&proxy).await).await;
        assert!(body.matches("event: ping").count() >= 2, "{format}: {body}");
        assert_eq!(
            body.matches("event: message_stop").count(),
            1,
            "{format}: {body}"
        );
        assert!(body.contains("first"));
        assert!(!body.contains("event: error"));
        assert_eq!(profile(&app, "p1").consecutive_successes, 1);
        assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 0);
        streaming_tests::assert_single_outcome(&app, 200, 1, 7);
        let db = app.state::<DbState>();
        let conn = db.0.lock().unwrap();
        let output: i64 = conn
            .query_row("SELECT output_tokens FROM proxy_request_logs", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(output, 3);
    }
}
