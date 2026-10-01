use super::*;
use futures_util::StreamExt;

async fn cut_server(frame: &'static str, gate: Arc<Notify>) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let gate = gate.clone();
        observed.fetch_add(1, Ordering::SeqCst);
        async move {
            let stream = async_stream::stream! {
                yield Ok::<_, std::io::Error>(bytes::Bytes::from_static(frame.as_bytes()));
                gate.notified().await;
                yield Err(std::io::Error::new(std::io::ErrorKind::ConnectionReset, "private transport credential"));
            };
            Response::builder().header("content-type", "text/event-stream")
                .body(Body::from_stream(stream)).unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Upstream { url, hits, task }
}

#[tokio::test]
async fn transport_cuts_are_recognizable_after_each_translation_without_replaying_partial_answers()
{
    for (format, frame) in [
        ("anthropic", "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":7}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"partial answer\"}}\n\n"),
        ("openai_chat", "data: {\"id\":\"c1\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{\"content\":\"partial answer\"}}],\"usage\":{\"prompt_tokens\":7}}\n\n"),
        ("openai_responses", "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"r1\",\"usage\":{\"input_tokens\":7}}}\n\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"item_id\":\"m1\",\"content_index\":0,\"delta\":\"partial answer\"}\n\n"),
        ("gemini_native", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial answer\"}]}}],\"usageMetadata\":{\"promptTokenCount\":7}}\n\n"),
    ] {
        let gate = Arc::new(Notify::new());
        let upstream = cut_server(frame, gate.clone()).await;
        let alternative = server(StatusCode::OK, "text/event-stream", "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n").await;
        let app = app(&[("p1", &upstream.url, vec![]), ("p2", &alternative.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        let response = forward(app.handle().clone(), true).await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body().into_data_stream();
        let mut output = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !String::from_utf8_lossy(&output).contains("partial answer") {
                output.extend_from_slice(&body.next().await.unwrap().unwrap());
            }
        }).await.unwrap();
        gate.notify_one();
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(chunk) = body.next().await { output.extend_from_slice(&chunk.unwrap()); }
        }).await.unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("connection lost"), "{format}: {text}");
        assert_eq!(text.matches("event: error").count(), 1, "{format}: {text}");
        assert!(!text.contains("private transport credential"));
        assert!(!text.contains("message_stop"));
        assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
        assert_eq!(alternative.hits.load(Ordering::SeqCst), 0, "partial answers must not be replayed by the proxy");
        streaming_tests::assert_single_outcome(&app, 502, 0, 7);
    }
}

#[tokio::test]
async fn invalid_utf8_is_a_data_error_after_forwarding_and_each_adapter_not_a_connection_failure() {
    for format in [
        "anthropic",
        "openai_chat",
        "openai_responses",
        "gemini_native",
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new().fallback(any(|| async {
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(Body::from(vec![0xff]))
                .unwrap()
        }));
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let invalid = Upstream {
            url,
            hits: Arc::new(AtomicUsize::new(0)),
            task,
        };
        let app = app(&[("p1", &invalid.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        let response = forward(app.handle().clone(), true).await;
        let bytes =
            tokio::time::timeout(Duration::from_secs(5), to_bytes(response.into_body(), 8192))
                .await
                .unwrap()
                .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("invalid data"), "{format}: {text}");
        assert!(!text.contains("connection"), "{format}: {text}");
        assert_eq!(text.matches("event: error").count(), 1);
        streaming_tests::assert_single_outcome(&app, 502, 0, 0);
    }
}
