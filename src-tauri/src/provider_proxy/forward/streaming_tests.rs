use super::*;

pub(super) async fn split_server(frame: &'static str) -> Upstream {
    split_server_with_pending(frame, false).await
}

pub(super) async fn split_server_with_pending(frame: &'static str, pending: bool) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            let stream = async_stream::stream! {
                for byte in frame.as_bytes() {
                    tokio::task::yield_now().await;
                    yield Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(&[*byte]));
                }
                if pending { std::future::pending::<()>().await; }
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

pub(super) fn assert_single_outcome(app: &App<MockRuntime>, status: u16, success: i64, input: i64) {
    drain_accounting();
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM proxy_request_logs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    let actual: (u16, i64) = conn
        .query_row(
            "SELECT status_code,input_tokens FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(actual, (status, input));
    let daily: (i64,i64,i64) = conn.query_row("SELECT total_requests,success_requests,total_input_tokens FROM proxy_usage_daily_rollups", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
    assert_eq!(daily, (1, success, input));
}

#[tokio::test]
async fn cancellation_after_usage_preserves_partial_tokens_without_counting_success() {
    use futures_util::StreamExt;
    let upstream = split_server_with_pending(
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":7}}}\n\n", true,
    ).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    let response = forward(app.handle().clone(), true).await;
    let mut body = response.into_body().into_data_stream();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !bytes.ends_with(b"\n\n") {
            bytes.extend_from_slice(&body.next().await.unwrap().unwrap());
        }
    })
    .await
    .unwrap();
    drop(body);
    assert_single_outcome(&app, 499, 0, 7);
}

#[test]
fn partial_usage_events_merge_without_losing_input_or_counting_duplicates() {
    use crate::provider_proxy::usage::scan_stream_usage_buffer;
    use crate::provider_proxy::ProxyUsageMetrics;
    let mut buffer = String::new();
    let mut usage = ProxyUsageMetrics::default();
    let mut gemini = crate::shared::gemini_usage::GeminiUsage::default();
    let start = "data: {\"message\":{\"model\":\"模型🦀\",\"usage\":{\"input_tokens\":7,\"cache_read_input_tokens\":2}}}\n\n";
    let delta = "data: {\"usage\":{\"output_tokens\":5}}\n\n";
    assert!(scan_stream_usage_buffer(
        &mut buffer,
        start,
        &mut usage,
        &mut gemini,
        crate::shared::token_usage::InputTokenBasis::ExcludesCache,
    ));
    for _ in 0..2 {
        assert!(scan_stream_usage_buffer(
            &mut buffer,
            delta,
            &mut usage,
            &mut gemini,
            crate::shared::token_usage::InputTokenBasis::ExcludesCache,
        ));
    }
    assert!(!scan_stream_usage_buffer(
        &mut buffer,
        "data: {\"model\":\"ignored\",\"usage\":{}}\n\n",
        &mut usage,
        &mut gemini,
        crate::shared::token_usage::InputTokenBasis::ExcludesCache,
    ));
    assert_eq!(usage.response_model.as_deref(), Some("模型🦀"));
    assert_eq!(usage.input_tokens, 7);
    assert_eq!(usage.output_tokens, 5);
    assert_eq!(usage.cache_read_tokens, 2);
}

#[tokio::test]
async fn every_byte_split_crlf_keeps_unicode_in_client_text_and_accounting() {
    for (format, frame) in [
        ("anthropic", "event: message_start\r\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"模型🦀\",\"usage\":{\"input_tokens\":1}}}\r\n\r\nevent: content_block_delta\r\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"你好🦀\"}}\r\n\r\nevent: message_stop\r\ndata: {\"type\":\"message_stop\"}\r\n\r\n"),
        ("openai_chat", "data: {\"id\":\"chat-1\",\"model\":\"模型🦀\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"你好🦀\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1}}\r\n\r\ndata: [DONE]\r\n\r\n"),
        ("openai_responses", "event: response.created\r\ndata: {\"response\":{\"id\":\"response-1\",\"model\":\"模型🦀\",\"usage\":{\"input_tokens\":1}}}\r\n\r\nevent: response.output_text.delta\r\ndata: {\"delta\":\"你好🦀\",\"item_id\":\"message-1\",\"content_index\":0}\r\n\r\nevent: response.completed\r\ndata: {\"response\":{\"id\":\"response-1\",\"model\":\"模型🦀\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\r\n\r\n"),
        ("gemini_native", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"你好🦀\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":1}}\r\n\r\n"),
    ] {
        let upstream = split_server(frame).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        open_profile(&app, "p1", true);
        let response = forward(app.handle().clone(), true).await;
        let bytes = tokio::time::timeout(Duration::from_secs(5), to_bytes(response.into_body(), 16384)).await.expect("Unicode stream must finish").unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("你好🦀"), "{format}: {text}");
        assert!(!text.contains('\u{fffd}'), "{format}: {text}");
        assert!(text.contains("event: message_stop"), "{format}: {text}");
        assert_eq!(profile(&app, "p1").consecutive_successes, 1, "{format}");
        assert_single_outcome(&app, 200, 1, 1);
        let db = app.state::<DbState>();
        let conn = db.0.lock().unwrap();
        let (model, tokens): (Option<String>, i64) = conn.query_row("SELECT response_model,input_tokens FROM proxy_request_logs", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(tokens, 1, "{format}");
        if format != "gemini_native" { assert_eq!(model.as_deref(), Some("模型🦀"), "{format}"); }
    }
}
