use super::*;

fn format(app: &App<MockRuntime>, api: &str) {
    let state = app.state::<DbState>();
    let conn = state.0.lock().unwrap();
    let raw: String = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id='p1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    value["env"]["ANTHROPIC_API_FORMAT"] = json!(api);
    conn.execute(
        "UPDATE config_profiles SET config_snapshot=?1 WHERE id='p1'",
        [value.to_string()],
    )
    .unwrap();
}

#[tokio::test]
async fn converted_proxy_messages_use_anthropic_ids_and_native_replies_remain_untouched() {
    for (api, wire, expected) in [
        (
            "openai_chat",
            r#"{"id":"chatcmpl-abc","model":"fixture-model","choices":[{"message":{"content":"hi"},"finish_reason":"stop"}]}"#,
            "msg_abc",
        ),
        (
            "openai_responses",
            r#"{"id":"resp_abc","model":"fixture-model","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"hi"}]}]}"#,
            "msg_abc",
        ),
        (
            "anthropic",
            r#"{"id":"vendor-native-id","model":"fixture-model","content":[]}"#,
            "vendor-native-id",
        ),
    ] {
        let upstream = server(StatusCode::OK, "application/json", wire).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        format(&app, api);
        let response = forward(app.handle().clone(), false).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 32 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["id"], expected);
        if api == "anthropic" {
            assert_eq!(
                value,
                serde_json::from_str::<serde_json::Value>(wire).unwrap()
            );
        }
    }
}

#[tokio::test]
async fn streamed_proxy_replies_start_once_with_an_anthropic_id() {
    for (api, wire, expected) in [
        ("openai_chat", "data: {\"id\":\"chatcmpl-abc\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{\"content\":\"hi\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n", Some("msg_abc")),
        ("openai_responses", "event: response.output_text.delta\ndata: {\"delta\":\"hi\"}\n\nevent: response.completed\ndata: {\"response\":{\"id\":\"resp_late\",\"status\":\"completed\"}}\n\n", None),
        ("anthropic", "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"native-id\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n", Some("native-id")),
    ] {
        let upstream = server(StatusCode::OK, "text/event-stream", wire).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        format(&app, api);
        let response = forward(app.handle().clone(), true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = tokio::time::timeout(Duration::from_secs(3), to_bytes(response.into_body(), 32 * 1024)).await.unwrap().unwrap();
        let output = String::from_utf8(bytes.to_vec()).unwrap();
        let starts = output.lines().filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|event| event["type"] == "message_start").collect::<Vec<_>>();
        assert_eq!(starts.len(), 1, "{output}");
        let id = starts[0]["message"]["id"].as_str().unwrap();
        if let Some(expected) = expected { assert_eq!(id, expected); }
        else { assert!(uuid::Uuid::parse_str(id.strip_prefix("msg_").unwrap()).is_ok()); }
        if api == "anthropic" { assert_eq!(output, wire); }
    }
}
