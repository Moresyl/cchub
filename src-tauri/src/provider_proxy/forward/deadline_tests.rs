use super::*;

const COMPLETE: &str = "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

async fn delayed_server(
    content_type: &'static str,
    header_delay: Duration,
    chunks: Vec<(Duration, &'static str)>,
    pending: bool,
) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        let chunks = chunks.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(header_delay).await;
            let stream = async_stream::stream! {
                for (delay, chunk) in chunks {
                    tokio::time::sleep(delay).await;
                    yield Ok::<_, std::io::Error>(bytes::Bytes::from_static(chunk.as_bytes()));
                }
                if pending { std::future::pending::<()>().await; }
            };
            Response::builder()
                .header("content-type", content_type)
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
async fn stalled_headers_fail_over_for_streaming_and_ordinary_requests() {
    for streaming in [false, true] {
        let gate = Arc::new(Notify::new());
        let primary =
            controlled_server(StatusCode::OK, "application/json", "{}", Some(gate), false).await;
        let alternate = server(StatusCode::OK, "application/json", "{}").await;
        let app = app(
            &[("p1", &primary.url, vec![alternate.url.clone()])],
            OptimizerConfig {
                streaming_first_byte_timeout: 1,
                non_streaming_timeout: 1,
                ..Default::default()
            },
        );
        assert_eq!(
            forward(app.handle().clone(), streaming).await.status(),
            StatusCode::OK
        );
        assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
        assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
        assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
        assert_eq!(profile(&app, "p1").consecutive_failures, 0);
        streaming_tests::assert_single_outcome(&app, 200, 1, 0);
    }
}

#[tokio::test]
async fn first_raw_byte_timeout_fails_over_before_committing_client_headers() {
    let primary = controlled_server(StatusCode::OK, "text/event-stream", "", None, true).await;
    let alternate = server(StatusCode::OK, "text/event-stream", COMPLETE).await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig {
            streaming_first_byte_timeout: 1,
            ..Default::default()
        },
    );
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        COMPLETE
    );
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 0);
}

#[tokio::test]
async fn empty_stream_fails_over_before_returning_an_unusable_success() {
    let primary = server(StatusCode::OK, "text/event-stream", "").await;
    let alternate = server(StatusCode::OK, "text/event-stream", COMPLETE).await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        COMPLETE
    );
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 0);
}

#[tokio::test]
async fn stalled_bounded_bodies_fail_over_including_vendor_errors_and_stream_json_fallback() {
    for (status, content_type, streaming) in [
        (StatusCode::OK, "application/json", false),
        (StatusCode::OK, "text/plain", false),
        (StatusCode::TOO_MANY_REQUESTS, "application/json", false),
        (StatusCode::OK, "application/json", true),
    ] {
        let primary = controlled_server(status, content_type, "", None, true).await;
        let alternate = server(StatusCode::OK, "application/json", "{}").await;
        let app = app(
            &[("p1", &primary.url, vec![alternate.url.clone()])],
            OptimizerConfig {
                non_streaming_timeout: 1,
                ..Default::default()
            },
        );
        assert_eq!(
            forward(app.handle().clone(), streaming).await.status(),
            StatusCode::OK
        );
        assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
        streaming_tests::assert_single_outcome(&app, 200, 1, 0);
    }
}

#[tokio::test]
async fn first_byte_deadline_is_not_restarted_after_response_headers() {
    let primary = delayed_server(
        "text/event-stream",
        Duration::from_millis(650),
        vec![(Duration::from_millis(650), COMPLETE)],
        false,
    )
    .await;
    let alternate = server(StatusCode::OK, "text/event-stream", COMPLETE).await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig {
            streaming_first_byte_timeout: 1,
            ..Default::default()
        },
    );
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        COMPLETE
    );
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
}

#[tokio::test]
async fn ordinary_total_deadline_is_not_reset_by_headers_or_trickling_body() {
    for chunks in [
        vec![(Duration::from_millis(650), "{}")],
        vec![(Duration::from_millis(250), " "); 8],
    ] {
        let primary = delayed_server(
            "application/json",
            Duration::from_millis(650),
            chunks,
            false,
        )
        .await;
        let alternate = server(StatusCode::OK, "application/json", "{}").await;
        let app = app(
            &[("p1", &primary.url, vec![alternate.url.clone()])],
            OptimizerConfig {
                non_streaming_timeout: 1,
                ..Default::default()
            },
        );
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::OK
        );
        assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
        assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
    }
}

#[tokio::test]
async fn raw_heartbeats_keep_translated_stream_alive_beyond_first_byte_and_body_budgets() {
    let mut chunks = vec![(Duration::ZERO, ": ping\n\n")];
    chunks.extend(vec![(Duration::from_millis(250), ": ping\n\n"); 6]);
    chunks.push((Duration::ZERO, "data: {\"id\":\"fixture\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{\"content\":\"alive\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"));
    let upstream = delayed_server("text/event-stream", Duration::ZERO, chunks, false).await;
    let app = app(
        &[("p1", &upstream.url, vec![])],
        OptimizerConfig {
            streaming_first_byte_timeout: 1,
            streaming_idle_timeout: 1,
            non_streaming_timeout: 1,
            ..Default::default()
        },
    );
    set_format(&app, "p1", "openai_chat");
    let response = forward(app.handle().clone(), true).await;
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert!(
        String::from_utf8_lossy(&body).contains("alive"),
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 0);
    streaming_tests::assert_single_outcome(&app, 200, 1, 0);
}

#[tokio::test]
async fn stream_idle_timeout_after_first_raw_byte_records_failure() {
    let upstream = delayed_server(
        "text/event-stream",
        Duration::ZERO,
        vec![(Duration::ZERO, ": ping\n\n")],
        true,
    )
    .await;
    let app = app(
        &[("p1", &upstream.url, vec![])],
        OptimizerConfig {
            streaming_idle_timeout: 1,
            ..Default::default()
        },
    );
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("event: error"));
    assert!(text.contains("api_error"));
    assert!(!text.contains("message_stop"));
    assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 1);
    streaming_tests::assert_single_outcome(&app, 502, 0, 0);
}

#[tokio::test]
async fn zero_disables_deadlines_without_substituting_another_default() {
    let gate = Arc::new(Notify::new());
    let upstream = controlled_server(
        StatusCode::OK,
        "application/json",
        "{}",
        Some(gate.clone()),
        false,
    )
    .await;
    let app = app(
        &[("p1", &upstream.url, vec![])],
        OptimizerConfig {
            non_streaming_timeout: 0,
            streaming_first_byte_timeout: 0,
            streaming_idle_timeout: 0,
            ..Default::default()
        },
    );
    let future = forward(app.handle().clone(), true);
    tokio::pin!(future);
    // Observe the blocked upstream before measuring deadline behavior. Database
    // and HTTP client setup can exceed the observation window on a busy machine.
    tokio::select! {
        response = &mut future => panic!("unexpected response before releasing the upstream: {}", response.status()),
        reached = tokio::time::timeout(Duration::from_secs(10), async {
            while upstream.hits.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        }) => reached.expect("request did not reach the controlled upstream"),
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut future)
            .await
            .is_err()
    );
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
    assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 0);
    gate.notify_one();
    assert_eq!(future.await.status(), StatusCode::OK);
    streaming_tests::assert_single_outcome(&app, 200, 1, 0);
}

#[test]
fn legacy_timeout_config_defaults_and_extreme_duration_validation() {
    let mut old = serde_json::to_value(OptimizerConfig::default()).unwrap();
    old.as_object_mut().unwrap().remove("nonStreamingTimeout");
    let migrated: OptimizerConfig = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(migrated.non_streaming_timeout, 600);
    for name in [
        "streamingFirstByteTimeout",
        "streamingIdleTimeout",
        "nonStreamingTimeout",
        "circuitTimeoutSecs",
    ] {
        let mut invalid = old.clone();
        invalid[name] = json!(u64::MAX);
        let config: OptimizerConfig = serde_json::from_value(invalid).unwrap();
        assert!(config.validate_timeouts().unwrap_err().contains(name));
    }
    assert!(OptimizerConfig {
        non_streaming_timeout: 0,
        ..Default::default()
    }
    .validate_timeouts()
    .is_ok());
}

#[tokio::test]
async fn invalid_timeout_config_fails_before_contacting_or_penalizing_upstream() {
    let upstream = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[("p1", &upstream.url, vec![])],
        OptimizerConfig {
            streaming_idle_timeout: u64::MAX,
            ..Default::default()
        },
    );
    assert_eq!(
        forward(app.handle().clone(), true).await.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 0);
    assert!(app
        .state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .endpoint_circuits
        .is_empty());
}
