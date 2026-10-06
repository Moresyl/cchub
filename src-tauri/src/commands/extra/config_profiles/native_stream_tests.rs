use super::*;
use axum::{
    body::{to_bytes, Body},
    http::Request,
    routing::any,
    Router,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn native_stream_request_uses_selected_alias_variant_credentials_and_real_protocol() {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let collected = observed.clone();
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let collected = collected.clone();
        async move {
            let path = request.uri().path().to_string();
            let auth = request.headers().get("authorization").or_else(|| request.headers().get("x-api-key")).or_else(|| request.headers().get("x-goog-api-key")).unwrap().to_str().unwrap().to_string();
            let body: Value = serde_json::from_slice(&to_bytes(request.into_body(), 65536).await.unwrap()).unwrap();
            collected.lock().unwrap().push((path.clone(), auth, body));
            let response = if path.ends_with("responses") { json!({"id":"resp_fixture","type":"response","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"OK"}]}]}) }
                else if path.ends_with("messages") { json!({"id":"msg_fixture","type":"message","content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn"}) }
                else if path.contains("streamGenerateContent") { json!({"candidates":[{"content":{"parts":[{"text":"OK"}]},"finishReason":"STOP"}]}) }
                else { json!({"id":"chatcmpl_fixture","choices":[{"message":{"content":"OK"},"finish_reason":"stop"}]}) };
            axum::Json(response)
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    for (package, path, auth) in [
        (
            "@opencode/ai/openai-compatible",
            "/v1/chat/completions",
            "Bearer selected",
        ),
        (
            "@opencode/ai/openai-compatible/responses",
            "/v1/responses",
            "Bearer selected",
        ),
        (
            "@opencode/ai/anthropic-compatible",
            "/v1/messages",
            "selected",
        ),
        (
            "@opencode/ai/google",
            "/v1/models/vendor/model:streamGenerateContent",
            "selected",
        ),
    ] {
        let snapshot = json!({"package":package,"settings":{"apiKey":"provider","baseURL":format!("{base}/v1")},"body":{"temperature":0.2,"max_tokens":1000000},"models":{"first":{"settings":{"apiKey":"unrelated"}},"alias":{"modelID":"vendor/model","variants":[{"id":"fast","settings":{"apiKey":"selected"}}]}},"metadata":{"nativeFormat":"providers","nativeProviderId":"local","nativeModelId":"alias#fast"}});
        let profile = ConfigProfile {
            id: "fixture".into(),
            name: "Fixture".into(),
            tool_id: "opencode".into(),
            config_snapshot: snapshot.to_string(),
            sort_order: 0,
            source_type: None,
            source_key: None,
            created_at: None,
            updated_at: None,
        };
        let request = extract_stream_check_request(app.handle(), &profile)
            .await
            .unwrap();
        assert_eq!(reqwest::Url::parse(&request.endpoint).unwrap().path(), path);
        let result = stream_probe::execute(client.clone(), request).await;
        assert_eq!(result.status, "healthy", "{package}: {}", result.message);
        let collected = observed.lock().unwrap();
        let (actual_path, actual_auth, body) = collected.last().unwrap();
        assert_eq!(actual_path, path);
        assert_eq!(actual_auth, auth);
        assert_eq!(body["temperature"], 0.2);
        if body.get("model").is_some() {
            assert_eq!(body["model"], "vendor/model");
        }
        assert!(body
            .get("max_tokens")
            .is_none_or(|value| value == &json!(16)));
    }
    task.abort();
}
