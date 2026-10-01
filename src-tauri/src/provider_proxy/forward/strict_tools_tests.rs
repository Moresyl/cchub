use super::*;
use serde_json::Value;

async fn observed_server(reply: Value) -> (Upstream, Arc<Mutex<Vec<(String, Value)>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = hits.clone();
    let collected = requests.clone();
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let observed = observed.clone();
        let collected = collected.clone();
        let reply = reply.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            let path = request.uri().to_string();
            let bytes = to_bytes(request.into_body(), 65536).await.unwrap();
            collected
                .lock()
                .unwrap()
                .push((path, serde_json::from_slice(&bytes).unwrap()));
            Response::builder()
                .header("content-type", "application/json")
                .body(Body::from(reply.to_string()))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (Upstream { url, hits, task }, requests)
}

async fn send(app: &App<MockRuntime>, path: &str, body: Value) -> Response<Body> {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/proxy/claude/{path}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
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
async fn translated_tools_send_loose_or_explicit_strictness_without_rewriting_schema() {
    let schema = json!({"type":"object","properties":{
        "path":{"type":"string","pattern":"^(?!.*\\.\\.).+$"},
        "optional":{"type":"string"}
    },"required":["path"]});
    for (api, path, reply) in [
        (
            "openai_responses",
            "/v1/responses",
            json!({"id":"resp_1","model":"vendor-model","status":"completed","output":[],"usage":{"input_tokens":7,"output_tokens":5}}),
        ),
        (
            "openai_chat",
            "/v1/chat/completions",
            json!({"id":"chatcmpl-1","model":"vendor-model","choices":[{"message":{"content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":5}}),
        ),
    ] {
        for strict in [None, Some(false), Some(true)] {
            let (upstream, captured) = observed_server(reply.clone()).await;
            let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
            set_format(&app, "p1", api);
            let mut tool = json!({"name":"read_file","input_schema":schema});
            if let Some(strict) = strict {
                tool["strict"] = json!(strict);
            }
            let response = send(
                &app,
                "v1/messages",
                json!({"model":"fixture-model","messages":[],"tools":[tool]}),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            to_bytes(response.into_body(), 65536).await.unwrap();
            let requests = captured.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].0, path);
            let function = if api == "openai_responses" {
                &requests[0].1["tools"][0]
            } else {
                &requests[0].1["tools"][0]["function"]
            };
            assert_eq!(function["parameters"], schema);
            assert_eq!(
                function.get("strict").and_then(Value::as_bool),
                if api == "openai_responses" {
                    Some(strict.unwrap_or(false))
                } else {
                    strict
                }
            );
            streaming_tests::assert_single_outcome(&app, 200, 1, 7);
        }
    }
}

#[tokio::test]
async fn native_tools_keep_original_strict_fields_and_schema() {
    for (path, body, reply) in [
        (
            "v1/messages",
            json!({"model":"fixture-model","messages":[],"tools":[{"name":"run","strict":true,"input_schema":{"type":"object","properties":{},"additionalProperties":false,"required":[]}}]}),
            json!({"id":"native-1","content":[],"usage":{"input_tokens":7,"output_tokens":5}}),
        ),
        (
            "v1/responses",
            json!({"model":"fixture-model","input":"hello","tools":[{"type":"function","name":"run","strict":null,"parameters":{"type":"object","properties":{"optional":{"type":"string"}}}}]}),
            json!({"id":"resp_native","status":"completed","output":[],"usage":{"input_tokens":7,"output_tokens":5}}),
        ),
        (
            "v1/chat/completions",
            json!({"model":"fixture-model","messages":[],"tools":[{"type":"function","function":{"name":"run","strict":false,"parameters":{"type":"object","properties":{"optional":{"type":"string"}}}}}]}),
            json!({"id":"chatcmpl-native","choices":[],"usage":{"prompt_tokens":7,"completion_tokens":5}}),
        ),
    ] {
        let (upstream, captured) = observed_server(reply.clone()).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let response = send(&app, path, body.clone()).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), reply);
        let requests = captured.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, format!("/{path}"));
        assert_eq!(requests[0].1["tools"], body["tools"]);
        streaming_tests::assert_single_outcome(&app, 200, 1, 7);
    }
}
