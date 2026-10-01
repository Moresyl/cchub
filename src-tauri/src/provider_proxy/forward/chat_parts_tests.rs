use super::*;
use serde_json::Value;

const TYPED_DELTA: &str = r#"{"content":[{"type":"thinking","thinking":[{"type":"text","text":"先确认🦀"}],"closed":true},{"type":"text","text":"答案"}]}"#;

fn typed_wire() -> String {
    format!("data: {{\"id\":\"fixture\",\"model\":\"fixture-model\",\"choices\":[{{\"index\":0,\"delta\":{TYPED_DELTA},\"finish_reason\":null}}],\"usage\":{{\"prompt_tokens\":7}}}}\r\n\r\ndata: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}],\"usage\":{{\"prompt_tokens\":7,\"completion_tokens\":9}}}}\r\n\r\ndata: [DONE]\r\n\r\n")
}

async fn upstream(wire: String, content_type: &'static str) -> Upstream {
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
                for byte in wire.as_bytes() {
                    yield Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(&[*byte]));
                }
            };
            Response::builder()
                .header("content-type", content_type)
                .header("etag", "fixture-original")
                .header("x-request-id", "fixture-original")
                .body(Body::from_stream(stream))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Upstream { url, hits, task }
}

#[tokio::test]
async fn typed_chat_stream_retains_thinking_text_and_usage_over_http() {
    let source = upstream(typed_wire(), "text/event-stream").await;
    let app = app(&[("p1", &source.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "openai_chat");
    open_profile(&app, "p1", true);
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let wire = std::str::from_utf8(&bytes).unwrap();
    let events: Vec<Value> = wire
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();
    let thinking = events
        .iter()
        .filter_map(|e| e.pointer("/delta/thinking").and_then(Value::as_str))
        .collect::<String>();
    let text = events
        .iter()
        .filter_map(|e| e.pointer("/delta/text").and_then(Value::as_str))
        .collect::<String>();
    assert_eq!(thinking, "先确认🦀");
    assert_eq!(text, "答案");
    assert_eq!(events.last().unwrap()["type"], "message_stop");
    assert_eq!(source.hits.load(Ordering::SeqCst), 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
}

async fn raw_forward(app: &App<MockRuntime>, streaming: bool) -> Response<Body> {
    let path = "v1/chat/completions";
    let request = Request::builder()
        .method("POST")
        .uri(format!("/proxy/claude/{path}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"model":"fixture-model","stream":streaming,"messages":[]}).to_string(),
        ))
        .unwrap();
    forward_proxy_request_with_client(
        app.handle().clone(),
        "claude".into(),
        path.into(),
        request,
        Some(reqwest::Client::builder().no_proxy().build().unwrap()),
    )
    .await
}

#[tokio::test]
async fn raw_typed_chat_stream_has_string_content_and_preserves_accounting() {
    let source = upstream(typed_wire(), "text/event-stream").await;
    let app = app(&[("p1", &source.url, vec![])], OptimizerConfig::default());
    let response = raw_forward(&app, true).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("etag"));
    assert_eq!(response.headers()["x-request-id"], "fixture-original");
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let events: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: ").filter(|data| *data != "[DONE]"))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();
    assert_eq!(events[0]["choices"][0]["delta"]["content"], "答案");
    assert_eq!(
        events[0]["choices"][0]["delta"]["reasoning_content"],
        "先确认🦀"
    );
    assert!(bytes.ends_with(b"data: [DONE]\n\n"));
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
}

#[tokio::test]
async fn typed_chat_whole_response_is_supported_for_native_and_messages_clients() {
    let wire = format!(
        r#"{{"id":"fixture","model":"fixture-model","choices":[{{"message":{TYPED_DELTA},"finish_reason":"stop"}}],"usage":{{"prompt_tokens":7,"completion_tokens":9}}}}"#
    );
    for translated in [false, true] {
        let source = upstream(wire.clone(), "application/json; charset=utf-8").await;
        let app = app(&[("p1", &source.url, vec![])], OptimizerConfig::default());
        if translated {
            set_format(&app, "p1", "openai_chat");
        }
        let response = if translated {
            forward(app.handle().clone(), false).await
        } else {
            raw_forward(&app, false).await
        };
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        if translated {
            assert_eq!(
                body["content"],
                json!([{"type":"thinking","thinking":"先确认🦀"},{"type":"text","text":"答案"}])
            );
            assert_eq!(body["usage"]["input_tokens"], 7);
        } else {
            assert_eq!(body["choices"][0]["message"]["content"], "答案");
            assert_eq!(
                body["choices"][0]["message"]["reasoning_content"],
                "先确认🦀"
            );
            assert_eq!(body["usage"]["completion_tokens"], 9);
        }
        assert_eq!(source.hits.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn unsupported_parts_after_output_fail_visibly_without_replaying_the_request() {
    let mut wire = typed_wire()
        .split("data: {\"choices\"")
        .next()
        .unwrap()
        .to_owned();
    wire += "data: {\"choices\":[{\"delta\":{\"content\":[{\"type\":\"thinking\",\"thinking\":\"secret-thought\",\"signature\":\"secret-signature\"}]},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
    let source = upstream(wire, "text/event-stream").await;
    let fallback = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[("p1", &source.url, vec![]), ("p2", &fallback.url, vec![])],
        OptimizerConfig::default(),
    );
    set_format(&app, "p1", "openai_chat");
    let response = forward(app.handle().clone(), true).await;
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let wire = std::str::from_utf8(&bytes).unwrap();
    assert!(wire.contains("答案") && wire.contains("unsupported or invalid parts"));
    assert!(!wire.contains("secret") && !wire.contains("message_stop"));
    assert_eq!(source.hits.load(Ordering::SeqCst), 1);
    assert_eq!(fallback.hits.load(Ordering::SeqCst), 0);
    streaming_tests::assert_single_outcome(&app, 502, 0, 7);
}

#[tokio::test]
async fn mixed_typed_blocks_tools_and_trailing_usage_keep_order_and_one_outcome() {
    let wire = concat!(
        "data: {\"id\":\"fixture\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{\"reasoning_content\":\"预先\",\"content\":[{\"type\":\"text\",\"text\":\"第一段\"},{\"type\":\"thinking\",\"thinking\":\"再思考\"},{\"type\":\"refusal\",\"refusal\":\"无法协助\"}],\"tool_calls\":[{\"index\":0,\"id\":\"call_owned\",\"function\":{\"name\":\"inspect\",\"arguments\":\"{\\\"x\\\":\"}}]},\"finish_reason\":null}],\"usage\":{\"prompt_tokens\":7}}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":9}}\n\n",
        "data: [DONE]\n\n"
    );
    let source = upstream(wire.into(), "text/event-stream").await;
    let app = app(&[("p1", &source.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "openai_chat");
    let response = forward(app.handle().clone(), true).await;
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let events: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();
    let starts: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "content_block_start")
        .collect();
    let types: Vec<&str> = starts
        .iter()
        .map(|e| e["content_block"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(types, ["thinking", "text", "thinking", "text", "tool_use"]);
    for (index, event) in starts.iter().enumerate() {
        assert_eq!(event["index"], index);
    }
    assert_eq!(starts[4]["content_block"]["id"], "call_owned");
    assert_eq!(starts[4]["content_block"]["name"], "inspect");
    let args = events
        .iter()
        .filter_map(|e| e.pointer("/delta/partial_json").and_then(Value::as_str))
        .collect::<String>();
    assert_eq!(args, r#"{"x":1}"#);
    let usage = events
        .iter()
        .rev()
        .find(|e| e["type"] == "message_delta")
        .unwrap();
    assert_eq!(usage["usage"]["input_tokens"], 7);
    assert_eq!(usage["usage"]["output_tokens"], 9);
    assert_eq!(events.last().unwrap()["type"], "message_stop");
    assert_eq!(source.hits.load(Ordering::SeqCst), 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
}

#[tokio::test]
async fn native_whole_json_keeps_unknown_parts_precision_escapes_and_layout_over_http() {
    for content in [
        r#""ordinary\u6587本""#,
        r#"[{"type":"thinking","thinking":"signed","signature":"YWJj==\/\u003d"}]"#,
        r#"[{"type":"image_url","image_url":{"url":"opaque://image"}}]"#,
    ] {
        let wire = format!(
            r#"{{ "choices" : [{{ "message":{{"content":{content}}},"finish_reason":"stop" }}], "opaque":{{"n":999999999999999999999999999999999999,"e":1.2300e+999,"x":1,"x":2}},"signature":"YWJj==\/\u003d" }}"#
        );
        let source = upstream(wire.clone(), "application/json").await;
        let app = app(&[("p1", &source.url, vec![])], OptimizerConfig::default());
        let response = raw_forward(&app, false).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["etag"], "fixture-original");
        assert_eq!(to_bytes(response.into_body(), 65536).await.unwrap(), wire);
        assert_eq!(source.hits.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn native_large_opaque_error_fields_fail_over_without_secret_disclosure() {
    for wire in [
        r#"{"error":{"message":"secret-token"},"opaque":1.2300e+999}"#,
        r#"{"error":{"message":"secret-token"},"error":null}"#,
    ] {
        let source = upstream(wire.into(), "application/json").await;
        let fallback = upstream(
            r#"{"choices":[{"message":{"content":"recovered"},"finish_reason":"stop"}]}"#.into(),
            "application/json",
        )
        .await;
        let app = app(
            &[("p1", &source.url, vec![]), ("p2", &fallback.url, vec![])],
            OptimizerConfig::default(),
        );
        let response = raw_forward(&app, false).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        assert!(std::str::from_utf8(&bytes).unwrap().contains("recovered"));
        assert_eq!(source.hits.load(Ordering::SeqCst), 1);
        assert_eq!(fallback.hits.load(Ordering::SeqCst), 1);
        let db = app.state::<DbState>();
        let error: Option<String> =
            db.0.lock()
                .unwrap()
                .query_row("SELECT error_message FROM proxy_request_logs", [], |row| {
                    row.get(0)
                })
                .unwrap();
        assert!(error.is_none());
    }
}
