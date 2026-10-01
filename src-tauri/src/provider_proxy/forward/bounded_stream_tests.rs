use super::*;

async fn dynamic_server(wire: String) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let wire = Arc::new(wire);
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        let wire = wire.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(Body::from(wire.as_str().to_owned()))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Upstream { url, hits, task }
}

#[tokio::test]
async fn bounded_decoder_failure_retains_usage_and_records_one_failed_request_for_every_adapter() {
    for (format, prefix, terminal) in [
        ("openai_chat", "data: {\"id\":\"x\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{}}],\"usage\":{\"prompt_tokens\":7}}\n\n", "data: [DONE]\n\n"),
        ("openai_responses", "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_x\",\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":7}}}\n\n", "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n"),
        ("gemini_native", "data: {\"usageMetadata\":{\"promptTokenCount\":7}}\n\n", "data: {\"candidates\":[{\"finishReason\":\"STOP\"}]}\n\n"),
    ] {
        let wire = format!("{prefix}data: private-secret{}\n\n{terminal}", "x".repeat(8 * 1024 * 1024));
        let upstream = dynamic_server(wire).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        let response = forward(app.handle().clone(), true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let output = to_bytes(response.into_body(), 65536).await.unwrap();
        let output = String::from_utf8(output.to_vec()).unwrap();
        assert_eq!(output.matches("event: error\n").count(), 1, "{format}: {output}");
        assert!(output.contains("8 MiB limit"));
        assert!(!output.contains("message_stop"));
        assert!(!output.contains("private-secret"));
        streaming_tests::assert_single_outcome(&app, 502, 0, 7);
        assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn valid_chat_finish_reason_without_done_is_delivered_and_counted_once() {
    let upstream = server(StatusCode::OK, "text/event-stream", "data: {\"id\":\"x\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{\"content\":\"answer\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":3}}\n\n").await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "openai_chat");
    let response = forward(app.handle().clone(), true).await;
    let output = to_bytes(response.into_body(), 65536).await.unwrap();
    let output = String::from_utf8(output.to_vec()).unwrap();
    assert_eq!(output.matches("event: message_stop\n").count(), 1);
    assert!(!output.contains("event: error\n"));
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
}
