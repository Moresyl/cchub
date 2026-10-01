use super::*;
use futures_util::StreamExt;

const WHOLE: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

async fn held_terminal_server() -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            let stream = async_stream::stream! {
                yield Ok::<_, std::io::Error>(bytes::Bytes::from_static(WHOLE.as_bytes()));
                std::future::pending::<()>().await;
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
async fn client_close_after_delivered_terminal_records_success_and_releases_the_probe_once() {
    let upstream = held_terminal_server().await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let mut recovering_endpoint = EndpointCircuitState::default();
    recovering_endpoint.state = CircuitState::Open;
    recovering_endpoint.open_until = Some(Instant::now() - Duration::from_secs(1));
    app.state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .endpoint_circuits
        .insert(
            endpoint_circuit_key("p1", &upstream.url),
            recovering_endpoint,
        );
    let response = forward(app.handle().clone(), true).await;
    let mut body = response.into_body().into_data_stream();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !bytes
            .windows(b"event: message_stop".len())
            .any(|part| part == b"event: message_stop")
        {
            bytes.extend_from_slice(&body.next().await.unwrap().unwrap());
        }
    })
    .await
    .unwrap();
    assert_eq!(bytes, WHOLE.as_bytes());
    drop(body);
    assert_eq!(profile(&app, "p1").consecutive_successes, 1);
    assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_successes, 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
}

#[tokio::test]
async fn cancellation_between_translated_events_does_not_credit_source_completion() {
    let upstream = server(StatusCode::OK, "text/event-stream",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hello\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":7,\"candidatesTokenCount\":3,\"totalTokenCount\":10}}\n\n").await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "gemini_native");
    open_profile(&app, "p1", true);
    let response = forward(app.handle().clone(), true).await;
    let mut body = response.into_body().into_data_stream();
    let first = body.next().await.unwrap().unwrap();
    assert!(std::str::from_utf8(&first)
        .unwrap()
        .contains("event: message_start"));
    assert!(!std::str::from_utf8(&first)
        .unwrap()
        .contains("event: message_stop"));
    drop(body);
    assert_eq!(profile(&app, "p1").consecutive_successes, 0);
    streaming_tests::assert_single_outcome(&app, 499, 0, 7);
}

#[tokio::test]
async fn translated_terminal_delivery_and_eof_each_count_only_one_success() {
    for drop_at_terminal in [true, false] {
        let upstream = server(StatusCode::OK, "text/event-stream",
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hello\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":7,\"candidatesTokenCount\":3,\"totalTokenCount\":10}}\n\n").await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", "gemini_native");
        open_profile(&app, "p1", true);
        let response = forward(app.handle().clone(), true).await;
        let mut body = response.into_body().into_data_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.unwrap();
            if drop_at_terminal
                && std::str::from_utf8(&chunk)
                    .unwrap()
                    .contains("event: message_stop")
            {
                break;
            }
        }
        drop(body);
        assert_eq!(profile(&app, "p1").consecutive_successes, 1);
        streaming_tests::assert_single_outcome(&app, 200, 1, 7);
    }
}

#[tokio::test]
async fn terminal_drop_does_not_heal_a_new_manually_reset_circuit() {
    let upstream = held_terminal_server().await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let response = forward(app.handle().clone(), true).await;
    let mut body = response.into_body().into_data_stream();
    let chunk = body.next().await.unwrap().unwrap();
    assert!(std::str::from_utf8(&chunk)
        .unwrap()
        .contains("message_stop"));
    {
        let state = app.state::<LocalProviderProxyRuntime>();
        let mut runtime = state.0.lock().unwrap();
        runtime.profile_circuits.insert(
            profile_circuit_key("claude", "p1"),
            EndpointCircuitState::default(),
        );
        runtime.endpoint_circuits.insert(
            endpoint_circuit_key("p1", &upstream.url),
            EndpointCircuitState::default(),
        );
    }
    drop(body);
    assert_eq!(profile(&app, "p1").consecutive_successes, 0);
    assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_successes, 0);
    streaming_tests::assert_single_outcome(&app, 200, 1, 7);
}
