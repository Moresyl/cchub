use super::*;
use bytes::Bytes;
use serde_json::Value;

const COMPLETE: &str = r#"{"id":"chat_ok","model":"fixture-model","choices":[{"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":4}}"#;
const STREAM: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":4}}\n\ndata: [DONE]\n\n";
const SHORT_STREAM: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}],\"usage\":{\"prompt_tokens\":7}}\n\n";
const SOURCE: &str = r#" {"model":"fixture-model", "stream":false, "messages" : [{"role":"user","content":"hi"}, {"role":"assistant", "content":null, "reasoning_content":"thought", "reasoning":"alias", "reasoning_details":[{"text":"detail"}], "tool_calls":[{"id":"call_1","type":"function","function":{"name":"read","arguments":"{}"}}], "opaque":{"n":1.230000e+44}}, {"role":"tool","tool_call_id":"call_1","content":"ok"}], "temperature":0.10000000000001 } "#;

async fn strict_server(
    reject_fields: bool,
    final_status: StatusCode,
    final_body: &'static str,
    final_type: &'static str,
    delay: Duration,
) -> (Upstream, Arc<Mutex<Vec<Bytes>>>) {
    history_server(
        reject_fields,
        final_status,
        final_body,
        final_type,
        delay,
        "application/json",
    )
    .await
}

async fn history_server(
    reject_fields: bool,
    final_status: StatusCode,
    final_body: &'static str,
    final_type: &'static str,
    delay: Duration,
    error_type: &'static str,
) -> (Upstream, Arc<Mutex<Vec<Bytes>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (seen, saved) = (hits.clone(), requests.clone());
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let (seen, saved) = (seen.clone(), saved.clone());
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            let bytes = to_bytes(request.into_body(), 65536).await.unwrap();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            saved.lock().unwrap().push(bytes);
            tokio::time::sleep(delay).await;
            let rejected = reject_fields
                .then(|| {
                    ["reasoning_content", "reasoning", "reasoning_details"]
                        .into_iter()
                        .find(|key| value["messages"][1].get(key).is_some())
                })
                .flatten();
            let (status, body, content_type) = match rejected {
                Some(field) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    json!({"detail":[{
                        "type":"extra_forbidden", "loc":["body","messages",1,field]
                    }]})
                    .to_string(),
                    error_type,
                ),
                None => (final_status, final_body.into(), final_type),
            };
            Response::builder()
                .status(status)
                .header("content-type", content_type)
                .header("retry-after", "17")
                .body(Body::from(body))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (Upstream { url, hits, task }, requests)
}

async fn send_history(
    app: &App<MockRuntime>,
    path: &str,
    body: &str,
    client: Option<reqwest::Client>,
) -> Response<Body> {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/proxy/claude/{path}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(6),
        forward_proxy_request_with_client(
            app.handle().clone(),
            "claude".into(),
            path.into(),
            request,
            client.or_else(|| Some(reqwest::Client::builder().no_proxy().build().unwrap())),
        ),
    )
    .await
    .expect("history forwarding must terminate")
}

#[tokio::test]
async fn strict_history_recovers_three_aliases_and_learns_without_changing_tool_links() {
    let (upstream, saved) = strict_server(
        true,
        StatusCode::OK,
        COMPLETE,
        "application/json",
        Duration::ZERO,
    )
    .await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let response = send_history(&app, "v1/chat/completions", SOURCE, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 8192).await.unwrap(),
        COMPLETE
    );
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 4);
    let requests = saved.lock().unwrap();
    assert_eq!(requests[0], SOURCE);
    let next: Value = serde_json::from_slice(&requests[3]).unwrap();
    assert_eq!(next["messages"][1]["tool_calls"][0]["id"], "call_1");
    assert_eq!(next["messages"][2]["tool_call_id"], "call_1");
    assert!(std::str::from_utf8(&requests[3])
        .unwrap()
        .contains("1.230000e+44"));
    assert!(std::str::from_utf8(&requests[3])
        .unwrap()
        .contains("0.10000000000001"));
    drop(requests);
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
    assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 0);
    assert_eq!(
        send_history(&app, "v1/chat/completions", SOURCE, None)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 5);
    set_format(&app, "p1", "anthropic"); // A real snapshot revision invalidates its policy.
    assert_eq!(
        send_history(&app, "v1/chat/completions", SOURCE, None)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 9);
}

#[tokio::test]
async fn mistral_host_is_converted_before_first_socket_request() {
    let (upstream, saved) = strict_server(
        true,
        StatusCode::OK,
        COMPLETE,
        "application/json",
        Duration::ZERO,
    )
    .await;
    let address: std::net::SocketAddr = upstream
        .url
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap();
    let base = format!("http://api.mistral.ai:{}", address.port());
    let app = app(&[("p1", &base, vec![])], OptimizerConfig::default());
    let client = reqwest::Client::builder()
        .no_proxy()
        .resolve("api.mistral.ai", address)
        .build()
        .unwrap();
    let response = send_history(&app, "v1/chat/completions", SOURCE, Some(client)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
    let value: Value = serde_json::from_slice(&saved.lock().unwrap()[0]).unwrap();
    assert_eq!(
        value["messages"][1]["content"][0]["thinking"][0]["text"],
        "thought\nalias\ndetail"
    );
    assert_eq!(value["messages"][1]["tool_calls"][0]["id"], "call_1");
    assert_eq!(value["messages"][2]["tool_call_id"], "call_1");
}

#[tokio::test]
async fn failed_repairs_do_not_learn_or_remove_history_from_next_endpoint() {
    let (primary, primary_saved) = strict_server(
        true,
        StatusCode::SERVICE_UNAVAILABLE,
        r#"{"error":"offline"}"#,
        "application/json",
        Duration::ZERO,
    )
    .await;
    let (alternate, alternate_saved) = strict_server(
        false,
        StatusCode::OK,
        COMPLETE,
        "application/json",
        Duration::ZERO,
    )
    .await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    for round in 1..=2 {
        let response = send_history(&app, "v1/chat/completions", SOURCE, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(primary.hits.load(Ordering::SeqCst), 4 * round);
        assert_eq!(alternate.hits.load(Ordering::SeqCst), round);
        assert_eq!(alternate_saved.lock().unwrap().last().unwrap(), SOURCE);
        assert_eq!(
            primary_saved.lock().unwrap()[(4 * (round - 1)) as usize],
            SOURCE
        );
        app.state::<LocalProviderProxyRuntime>()
            .0
            .lock()
            .unwrap()
            .preferred_base_urls
            .clear();
    }
}

#[tokio::test]
async fn unrelated_errors_keep_vendor_status_headers_and_original_bytes_without_retry() {
    const ERROR: &str = r#" {"error":{"message":"reasoning_content: quota exhausted"}} "#;
    let (upstream, saved) = strict_server(
        false,
        StatusCode::BAD_REQUEST,
        ERROR,
        "application/json",
        Duration::ZERO,
    )
    .await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    let response = send_history(&app, "v1/chat/completions", SOURCE, None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["retry-after"], "17");
    assert_eq!(to_bytes(response.into_body(), 8192).await.unwrap(), ERROR);
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
    assert_eq!(saved.lock().unwrap()[0], SOURCE);
}

#[tokio::test]
async fn repair_retries_share_deadline_and_fail_over_before_response_commit() {
    for streaming in [false, true] {
        let (primary, _) = strict_server(
            true,
            StatusCode::OK,
            COMPLETE,
            "application/json",
            Duration::from_millis(650),
        )
        .await;
        let (alternate, saved) = strict_server(
            false,
            StatusCode::OK,
            COMPLETE,
            "application/json",
            Duration::ZERO,
        )
        .await;
        let app = app(
            &[("p1", &primary.url, vec![alternate.url.clone()])],
            OptimizerConfig {
                non_streaming_timeout: 1,
                streaming_first_byte_timeout: 1,
                ..Default::default()
            },
        );
        let source = SOURCE.replace("\"stream\":false", &format!("\"stream\":{streaming}"));
        let response = send_history(&app, "v1/chat/completions", &source, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(primary.hits.load(Ordering::SeqCst), 2);
        assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
        assert_eq!(saved.lock().unwrap()[0], source);
        assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
    }
}

#[tokio::test]
async fn only_completed_streams_learn_history_policy() {
    for (wire, complete, expected_hits) in [(STREAM, true, 5), (SHORT_STREAM, false, 8)] {
        let (upstream, _) = strict_server(
            true,
            StatusCode::OK,
            wire,
            "text/event-stream",
            Duration::ZERO,
        )
        .await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let source = SOURCE.replace("\"stream\":false", "\"stream\":true");
        for _ in 0..2 {
            let response = send_history(&app, "v1/chat/completions", &source, None).await;
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = to_bytes(response.into_body(), 8192).await.unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            assert_eq!(text.contains("data: [DONE]"), complete);
            assert_eq!(text.contains("api_error"), !complete);
        }
        assert_eq!(upstream.hits.load(Ordering::SeqCst), expected_hits);
    }
}

#[tokio::test]
async fn malformed_or_unusable_json_success_cannot_cache_repairs() {
    for reply in [
        "broken JSON",
        "{}",
        r#"{"choices":[{"message":{},"finish_reason":null}]}"#,
    ] {
        let (upstream, _) = strict_server(
            true,
            StatusCode::OK,
            reply,
            "application/json",
            Duration::ZERO,
        )
        .await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        for _ in 0..2 {
            let _ = send_history(&app, "v1/chat/completions", SOURCE, None).await;
        }
        assert_eq!(upstream.hits.load(Ordering::SeqCst), 8, "{reply}");
    }
}

#[tokio::test]
async fn structured_validation_without_json_content_type_is_recovered_for_streaming_clients() {
    for (streaming, error_type) in [
        (false, "text/plain"),
        (true, ""),
        (true, "application/problem+json"),
    ] {
        let (upstream, _) = history_server(
            true,
            StatusCode::OK,
            COMPLETE,
            "application/json",
            Duration::ZERO,
            error_type,
        )
        .await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let source = SOURCE.replace("\"stream\":false", &format!("\"stream\":{streaming}"));
        let response = send_history(&app, "v1/chat/completions", &source, None).await;
        assert_eq!(response.status(), StatusCode::OK, "{error_type}");
        assert_eq!(upstream.hits.load(Ordering::SeqCst), 4);
    }
}

#[tokio::test]
async fn native_messages_does_not_treat_chat_history_error_as_a_repair_instruction() {
    let (upstream, saved) = strict_server(
        true,
        StatusCode::OK,
        COMPLETE,
        "application/json",
        Duration::ZERO,
    )
    .await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    let response = send_history(&app, "v1/messages", SOURCE, None).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
    assert_eq!(saved.lock().unwrap()[0], SOURCE);
}
