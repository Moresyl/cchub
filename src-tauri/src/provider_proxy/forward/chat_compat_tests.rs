use super::*;
use futures_util::StreamExt;
use serde_json::Value;

async fn chunked_server(wire: String, content_type: &'static str, pending: bool) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        let wire = wire.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            let stream = async_stream::stream! {
                for bytes in wire.as_bytes().chunks(13) {
                    tokio::task::yield_now().await;
                    yield Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(bytes));
                }
                if pending { std::future::pending::<()>().await; }
            };
            Response::builder()
                .header("content-type", content_type)
                .body(Body::from_stream(stream))
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

fn event(delta: Value, finish: Value, usage: Value) -> String {
    format!(
        "data: {}\r\n\r\n",
        json!({"id":"fixture","model":"fixture-model","choices":[{"index":0,"delta":delta,"finish_reason":finish}],"usage":usage})
    )
}

#[tokio::test]
async fn raw_chat_stream_repairs_empty_fields_and_preserves_tools_usage_and_recovery() {
    let mut wire = event(
        json!({"reasoning_content":"先思考","tool_calls":[]}),
        json!(""),
        json!({"prompt_tokens":7}),
    );
    wire += &event(
        json!({"content":"你好🦀","reasoning_content":"","tool_calls":[]}),
        json!(""),
        json!(null),
    );
    wire += &event(
        json!({"tool_calls":[{"index":0,"id":"call_one","function":{"name":"grep","arguments":"{\"path\":\""}}]}),
        json!(""),
        json!(null),
    );
    wire += &event(
        json!({"tool_calls":[{"index":0,"function":{"name":"","arguments":"文件\"}"}}]}),
        json!("tool_calls"),
        json!({"prompt_tokens":7,"completion_tokens":4}),
    );
    wire += "data: [DONE]\r\n\r\n";
    let upstream = chunked_server(wire, "text/event-stream", false).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let response = forward_path(&app, "v1/chat/completions").await;
    assert_eq!(response.status(), StatusCode::OK);
    let output = to_bytes(response.into_body(), 65536).await.unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.ends_with("data: [DONE]\n\n"));
    assert!(
        !text.contains("\"name\":\"\"")
            && !text.contains("\"reasoning_content\":\"\"")
            && !text.contains("\"tool_calls\":[]")
    );
    assert!(
        text.contains("\"reasoning_content\":\"先思考\"")
            && text.contains("\"content\":\"你好🦀\"")
    );
    let mut name = String::new();
    let mut arguments = String::new();
    for data in text.lines().filter_map(|line| line.strip_prefix("data: ")) {
        if data == "[DONE]" {
            continue;
        }
        let value: Value = serde_json::from_str(data).unwrap();
        assert!(
            value["choices"][0]["finish_reason"].is_null()
                || value["choices"][0]["finish_reason"] == "tool_calls"
        );
        if let Some(calls) = value
            .pointer("/choices/0/delta/tool_calls")
            .and_then(Value::as_array)
        {
            let function = &calls[0]["function"];
            if let Some(next) = function["name"].as_str() {
                name = next.to_owned();
            }
            arguments.push_str(function["arguments"].as_str().unwrap());
        }
    }
    assert_eq!(name, "grep");
    assert_eq!(arguments, r#"{"path":"文件"}"#);
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
    assert_eq!(profile(&app, "p1").consecutive_successes, 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
    let db = app.state::<DbState>();
    let output_tokens: i64 =
        db.0.lock()
            .unwrap()
            .query_row("SELECT output_tokens FROM proxy_request_logs", [], |row| {
                row.get(0)
            })
            .unwrap();
    assert_eq!(output_tokens, 4);
}

#[tokio::test]
async fn translated_vendor_text_has_one_block_and_one_recorded_outcome() {
    let mut wire = event(
        json!({"reasoning_content":"先思考","tool_calls":[]}),
        json!(""),
        json!({"prompt_tokens":7}),
    );
    for text in ["我来", "看", "一下🦀"] {
        wire += &event(
            json!({"content":text,"reasoning_content":"","tool_calls":[]}),
            json!(""),
            json!(null),
        );
    }
    wire += &event(
        json!({"tool_calls":[]}),
        json!("stop"),
        json!({"prompt_tokens":7,"completion_tokens":4}),
    );
    wire += "data: [DONE]\r\n\r\n";
    let upstream = chunked_server(wire, "text/event-stream", false).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "openai_chat");
    open_profile(&app, "p1", true);
    let response = forward(app.handle().clone(), true).await;
    let output = to_bytes(response.into_body(), 65536).await.unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    let events: Vec<Value> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();
    let starts: Vec<_> = events
        .iter()
        .filter(|event| event["type"] == "content_block_start")
        .collect();
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0]["content_block"]["type"], "thinking");
    assert_eq!(starts[1]["content_block"]["type"], "text");
    assert_eq!(
        events
            .iter()
            .filter_map(|event| event.pointer("/delta/text").and_then(Value::as_str))
            .collect::<String>(),
        "我来看一下🦀"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "message_delta")
            .count(),
        1
    );
    assert_eq!(events.last().unwrap()["type"], "message_stop");
    assert_eq!(profile(&app, "p1").consecutive_successes, 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
}

#[tokio::test]
async fn other_protocols_and_non_sse_chat_content_keep_empty_fields_unchanged() {
    let fixture = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"\",\"tool_calls\":[]},\"finish_reason\":\"\"}]}\n\ndata: [DONE]\n\n";
    for (path, content_type) in [
        ("v1/messages", "text/event-stream"),
        ("v1/responses", "text/event-stream"),
        (
            "v1beta/models/gemini:streamGenerateContent",
            "text/event-stream",
        ),
        ("v1/chat/completions", "text/plain"),
    ] {
        let upstream = chunked_server(fixture.into(), content_type, false).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let response = forward_path(&app, path).await;
        let output = to_bytes(response.into_body(), 65536).await.unwrap();
        assert_eq!(output, fixture, "{path}: {content_type}");
    }
}

#[tokio::test]
async fn incomplete_and_error_streams_remain_failed_with_partial_usage_once() {
    for ending in [
        "data: {\"choices\":[{\"delta\":{\"content\":\"cut", // truncated event
        "data: {\"error\":{\"type\":\"api_error\",\"message\":\"fixture failed\"}}\n\n",
    ] {
        let mut wire = event(
            json!({"content":"partial","reasoning_content":"","tool_calls":[]}),
            json!(""),
            json!({"prompt_tokens":7}),
        );
        wire += ending;
        let upstream = chunked_server(wire, "text/event-stream", false).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        open_profile(&app, "p1", true);
        let response = forward_path(&app, "v1/chat/completions").await;
        let output = to_bytes(response.into_body(), 65536).await.unwrap();
        let text = std::str::from_utf8(&output).unwrap();
        assert!(text.contains("api_error"));
        assert!(text.contains(ending));
        assert!(!text.contains("[DONE]"));
        assert_eq!(profile(&app, "p1").consecutive_successes, 0);
        assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 1);
        streaming_tests::assert_single_outcome(&app, 502, 0, 7);
    }
}

#[tokio::test]
async fn cancelling_after_a_repaired_event_keeps_usage_without_counting_success() {
    let wire = event(
        json!({"content":"partial","tool_calls":[]}),
        json!(""),
        json!({"prompt_tokens":7}),
    );
    let upstream = chunked_server(wire, "text/event-stream", true).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    let response = forward_path(&app, "v1/chat/completions").await;
    let mut body = response.into_body().into_data_stream();
    let first = tokio::time::timeout(Duration::from_secs(5), body.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(first.ends_with(b"\n\n"));
    assert!(!std::str::from_utf8(&first)
        .unwrap()
        .contains("\"tool_calls\":[]"));
    drop(body);
    streaming_tests::assert_single_outcome(&app, 499, 0, 7);
}
