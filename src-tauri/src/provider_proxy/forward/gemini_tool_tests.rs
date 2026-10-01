use super::*;
use serde_json::Value;

async fn observed_server(
    streaming: bool,
    wire: String,
) -> (Upstream, Arc<Mutex<Vec<(String, Value)>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = hits.clone();
    let collected = requests.clone();
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let observed = observed.clone();
        let collected = collected.clone();
        let wire = wire.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            let path = request.uri().to_string();
            let bytes = to_bytes(request.into_body(), 4 * 1024 * 1024)
                .await
                .unwrap();
            collected
                .lock()
                .unwrap()
                .push((path, serde_json::from_slice(&bytes).unwrap()));
            Response::builder()
                .header(
                    "content-type",
                    if streaming {
                        "text/event-stream"
                    } else {
                        "application/json"
                    },
                )
                .body(Body::from(wire))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (Upstream { url, hits, task }, requests)
}

async fn with_history(app: &App<MockRuntime>, streaming: bool, messages: Value) -> Response<Body> {
    let request = Request::builder()
        .method("POST")
        .uri("/proxy/claude/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"model":"fixture-model","stream":streaming,"messages":messages}).to_string(),
        ))
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        forward_proxy_request_with_client(
            app.handle().clone(),
            "claude".into(),
            "v1/messages".into(),
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        ),
    )
    .await
    .unwrap()
}

fn native_reply() -> Value {
    json!({"candidates":[{"content":{"parts":[
        {"functionCall":{"id":"native-a","name":"read_file","args":{"path":"你好.txt"}}},
        {"functionCall":{"id":"native-b","name":"read_file","args":{"path":"other.txt"}}}
    ]},"finishReason":"STOP"}],"modelVersion":"fixture-native",
    "usageMetadata":{"promptTokenCount":7,"candidatesTokenCount":3,"thoughtsTokenCount":2,"cachedContentTokenCount":1}})
}

fn assert_accounting(app: &App<MockRuntime>, status: u16, success: i64) {
    streaming_tests::assert_single_outcome(app, status, success, 7);
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let actual: (i64, i64, String) = conn
        .query_row(
            "SELECT output_tokens,cache_read_tokens,response_model FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(actual, (5, 1, "fixture-native".into()));
    let daily: (i64, i64) = conn
        .query_row(
            "SELECT total_output_tokens,total_cache_read_tokens FROM proxy_usage_daily_rollups",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(daily, (5, 1));
}

fn streamed_tools(bytes: &[u8]) -> Vec<Value> {
    let events: Vec<Value> = std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    events
        .iter()
        .filter(|event| event["type"] == "content_block_start")
        .map(|event| {
            let mut tool = event["content_block"].clone();
            let arguments: String = events
                .iter()
                .filter(|delta| {
                    delta["type"] == "content_block_delta" && delta["index"] == event["index"]
                })
                .filter_map(|delta| delta["delta"]["partial_json"].as_str())
                .collect();
            tool["input"] = serde_json::from_str(&arguments).unwrap();
            tool
        })
        .collect()
}

#[tokio::test]
async fn gemini_tool_whole_and_streamed_replies_continue_with_exact_ids_and_function_names() {
    for streaming in [false, true] {
        let reply = native_reply();
        let wire = if streaming {
            format!("data: {reply}\n\n")
        } else {
            reply.to_string()
        };
        let (upstream, requests) = observed_server(streaming, wire).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", "gemini_native");
        let response = with_history(&app, streaming, json!([])).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let content: Vec<Value> = if streaming {
            let output = std::str::from_utf8(&bytes).unwrap();
            assert!(output.contains("\"output_tokens\":5"));
            assert!(output.contains("\"cache_read_input_tokens\":1"));
            assert!(output.contains("\"input_tokens\":6"));
            streamed_tools(&bytes)
        } else {
            let body: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                body["usage"],
                json!({"input_tokens":6,"output_tokens":5,"cache_read_input_tokens":1})
            );
            body["content"].as_array().unwrap().clone()
        };
        assert_accounting(&app, 200, 1);
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["id"], "native-a");
        assert_eq!(content[1]["id"], "native-b");
        assert_eq!(content[0]["input"]["path"], "你好.txt");
        assert_eq!(content[1]["input"]["path"], "other.txt");
        let response = with_history(&app, streaming, json!([
            {"role":"assistant","content":content},
            {"role":"user","content":[
                {"type":"tool_result","tool_use_id":"native-b","content":"B"},
                {"type":"tool_result","tool_use_id":"native-a","content":"A failed","is_error":true}
            ]}
        ])).await;
        assert_eq!(response.status(), StatusCode::OK);
        to_bytes(response.into_body(), 65536).await.unwrap();
        let captured = requests.lock().unwrap();
        assert_eq!(captured.len(), 2);
        let (path, body) = &captured[1];
        assert!(path.contains("models/fixture-model:"));
        assert!(path.contains(if streaming {
            "streamGenerateContent"
        } else {
            "generateContent"
        }));
        if streaming {
            assert!(path.contains("alt=sse"));
        }
        assert!(body.get("_stream").is_none());
        assert!(body.get("stream").is_none());
        assert_eq!(
            body["contents"][0]["parts"][0]["functionCall"]["id"],
            "native-a"
        );
        assert_eq!(
            body["contents"][0]["parts"][1]["functionCall"]["id"],
            "native-b"
        );
        assert_eq!(
            body["contents"][0]["parts"][0]["functionCall"]["args"]["path"],
            "你好.txt"
        );
        assert_eq!(
            body["contents"][0]["parts"][1]["functionCall"]["args"]["path"],
            "other.txt"
        );
        assert_eq!(
            body["contents"][1]["parts"][0]["functionResponse"],
            json!({"id":"native-b","name":"read_file","response":{"result":"B"}})
        );
        assert_eq!(
            body["contents"][1]["parts"][1]["functionResponse"],
            json!({"id":"native-a","name":"read_file","response":{"error":"A failed"}})
        );
    }
}

#[tokio::test]
async fn gemini_tool_orphan_results_stop_locally_before_vendor_io_or_circuit_penalty() {
    let (upstream, requests) = observed_server(false, native_reply().to_string()).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "gemini_native");
    let response = with_history(&app, false, json!([
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"private-id","content":"private-result"}]}
    ])).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["content-type"], "application/json");
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let output = std::str::from_utf8(&bytes).unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert!(output.contains("preceding unresolved call"));
    assert!(!output.contains("private-id"));
    assert!(!output.contains("private-result"));
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 0);
    assert!(requests.lock().unwrap().is_empty());
    assert!(app
        .state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .profile_circuits
        .is_empty());
}

#[tokio::test]
async fn gemini_tool_duplicate_reply_ids_fail_accounting_instead_of_crediting_vendor_completion() {
    for streaming in [false, true] {
        let mut reply = native_reply();
        reply["candidates"][0]["content"]["parts"][1]["functionCall"]["id"] = json!("native-a");
        let wire = if streaming {
            format!("data: {reply}\n\n")
        } else {
            reply.to_string()
        };
        let (upstream, _) = observed_server(streaming, wire).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", "gemini_native");
        let response = with_history(&app, streaming, json!([])).await;
        assert_eq!(
            response.status(),
            if streaming {
                StatusCode::OK
            } else {
                StatusCode::BAD_GATEWAY
            }
        );
        if !streaming {
            assert_eq!(response.headers()["content-type"], "application/json");
        }
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let output = std::str::from_utf8(&bytes).unwrap();
        if streaming {
            assert_eq!(output.matches("event: content_block_start").count(), 1);
            assert_eq!(output.matches("event: error").count(), 1);
            assert!(!output.contains("event: message_stop"));
        } else {
            let body: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(body["type"], "error");
        }
        assert_accounting(&app, 502, 0);
    }
}

#[tokio::test]
async fn gemini_tool_invalid_whole_reply_can_fail_over_without_double_counting() {
    for malformed in [true, false] {
        let mut invalid = native_reply();
        invalid["candidates"][0]["content"]["parts"][1]["functionCall"]["id"] = json!("native-a");
        let (primary, _) = observed_server(
            false,
            if malformed {
                "invalid JSON".into()
            } else {
                invalid.to_string()
            },
        )
        .await;
        let (alternate, _) = observed_server(false, native_reply().to_string()).await;
        let app = app(
            &[("p1", &primary.url, vec![alternate.url.clone()])],
            OptimizerConfig::default(),
        );
        set_format(&app, "p1", "gemini_native");
        let response = with_history(&app, false, json!([])).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["content"][1]["id"], "native-b");
        assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
        assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
        assert_accounting(&app, 200, 1);
    }
}

#[tokio::test]
async fn gemini_tool_native_route_preserves_the_original_reply() {
    for streaming in [false, true] {
        let original = native_reply();
        let wire = if streaming {
            format!("data: {original}\n\n")
        } else {
            original.to_string()
        };
        let (upstream, _) = observed_server(streaming, wire.clone()).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let path = if streaming {
            "v1beta/models/fixture:streamGenerateContent"
        } else {
            "v1beta/models/fixture:generateContent"
        };
        let request = Request::builder()
            .method("POST")
            .uri(format!("/proxy/claude/{path}"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"contents":[],"stream":streaming}).to_string(),
            ))
            .unwrap();
        let response = tokio::time::timeout(
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
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        if streaming {
            assert_eq!(bytes.as_ref(), wire.as_bytes());
        } else {
            assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), original);
        }
        assert_accounting(&app, 200, 1);
    }
}
