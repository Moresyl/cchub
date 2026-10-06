use super::*;

async fn timed_server(
    initial: &'static str,
    output: &'static str,
    terminal: &'static str,
) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            let stream = async_stream::stream! {
                yield Ok::<_, std::io::Error>(bytes::Bytes::from_static(initial.as_bytes()));
                tokio::time::sleep(Duration::from_millis(80)).await;
                yield Ok(bytes::Bytes::from_static(output.as_bytes()));
                tokio::time::sleep(Duration::from_millis(160)).await;
                yield Ok(bytes::Bytes::from_static(output.as_bytes()));
                tokio::time::sleep(Duration::from_millis(80)).await;
                yield Ok(bytes::Bytes::from_static(terminal.as_bytes()));
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

#[tokio::test]
async fn native_output_timing_survives_preflight_adapters_late_usage_and_all_query_paths() {
    for (format, initial, output, terminal) in [
        ("anthropic",
         "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":7}}}\n\n",
         "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
         "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":12}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"),
        ("openai_chat",
         "data: {\"id\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
         "data: {\"id\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
         "data: {\"id\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":12}}\n\ndata: [DONE]\n\n"),
        ("openai_responses",
         "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"m\",\"model\":\"fixture-model\"}}\n\n",
         "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"item_id\":\"i\",\"content_index\":0,\"delta\":\"hi\"}\n\n",
         "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"m\",\"status\":\"completed\",\"usage\":{\"input_tokens\":7,\"output_tokens\":12}}}\n\n"),
        ("gemini_native",
         "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"\"}]}}]}\n\n",
         "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}\n\n",
         "data: {\"candidates\":[{\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":7,\"candidatesTokenCount\":12}}\n\n"),
    ] {
        let upstream = timed_server(initial, output, terminal).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        let response = forward(app.handle().clone(), true).await;
        // The first output was already received by preflight. Delaying the first
        // downstream poll must not stamp that output at the later delivery time.
        tokio::time::sleep(Duration::from_millis(400)).await;
        let bytes = tokio::time::timeout(Duration::from_secs(5), to_bytes(response.into_body(), 16384)).await.unwrap().unwrap();
        assert!(String::from_utf8(bytes.to_vec()).unwrap().contains("hi"), "{format}");
        streaming_tests::assert_single_outcome(&app, 200, 1, 7);
        let db = app.state::<DbState>();
        let recent = crate::commands::usage_commands::get_recent_proxy_request_logs(Some(10), app.state::<DbState>()).unwrap();
        let searched = crate::commands::usage_commands::search_proxy_request_logs(None, app.state::<DbState>()).unwrap();
        let detail = crate::commands::usage_commands::get_request_detail(recent[0].request_id.clone(), app.state::<DbState>()).unwrap().unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(searched.len(), 1);
        for row in [&recent[0], &searched[0], &detail] {
            let first = row.first_output_ms.expect("actual output must be timed");
            let generation = row.generation_ms.expect("two output events must be timed");
            assert!(first >= 60, "{format}: {first}");
            assert!(first + 300 < row.latency_ms, "preflight output must precede delayed polling: {format}, {first}, {}", row.latency_ms);
            assert!(generation >= 100 && generation <= row.latency_ms - first, "{format}: {generation}");
            assert_eq!(row.output_tokens, 12, "late native usage must survive: {format}");
            assert_eq!((row.first_output_ms, row.generation_ms), (detail.first_output_ms, detail.generation_ms));
        }
        assert!(detail.stream_attempts.is_empty());
        drop(db);
    }
}

#[tokio::test]
async fn cancellation_retains_observed_first_output_without_turning_it_into_success() {
    use futures_util::StreamExt;
    let upstream = streaming_tests::split_server_with_pending(
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n", true,
    ).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    let response = forward(app.handle().clone(), true).await;
    let mut body = response.into_body().into_data_stream();
    assert!(body.next().await.unwrap().is_ok());
    drop(body);
    drain_accounting();
    let rows = crate::commands::usage_commands::get_recent_proxy_request_logs(
        None,
        app.state::<DbState>(),
    )
    .unwrap();
    assert_eq!(rows[0].status_code, 499);
    assert!(rows[0].first_output_ms.is_some());
    assert_eq!(rows[0].generation_ms, Some(0));
}
