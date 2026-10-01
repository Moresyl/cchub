use super::*;

#[tokio::test]
async fn keepalive_does_not_hide_silence_or_an_incomplete_eof() {
    for pending in [false, true] {
        for format in ["openai_chat", "openai_responses", "gemini_native"] {
            let upstream = active_server(frames(format).0, "", pending).await;
            let app = app(
                &[("p1", &upstream.url, vec![])],
                OptimizerConfig {
                    streaming_idle_timeout: 1,
                    ..Default::default()
                },
            );
            set_format(&app, "p1", format);
            open_profile(&app, "p1", true);
            let proxy = proxy_server(app.handle().clone()).await;
            // After real activity stops, the upstream's one-second idle
            // deadline can start just short of one second after the last
            // rate-limited ping. Allow both gaps to receive the error; the
            // strict live-provider watchdog remains 1.4 seconds elsewhere.
            let body = tokio::time::timeout(
                Duration::from_secs(5),
                read_with_watchdog_budget(client(&proxy).await, Duration::from_millis(2400)),
            )
            .await
            .expect("a stopped provider must end rather than receive synthetic cover");
            assert!(body.contains("event: ping"), "{format}: {body}");
            assert_eq!(body.matches("event: error").count(), 1, "{format}: {body}");
            assert!(!body.contains("event: message_stop"), "{format}: {body}");
            assert_eq!(profile(&app, "p1").consecutive_successes, 0);
            streaming_tests::assert_single_outcome(&app, 502, 0, 7);
        }
    }
}

#[tokio::test]
async fn keepalive_does_not_turn_a_vendor_error_into_success() {
    for format in ["openai_chat", "openai_responses", "gemini_native"] {
        let upstream = active_server(
            frames(format).0,
            "event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"fixture failure\"}}\n\n",
            false,
        ).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        open_profile(&app, "p1", true);
        let proxy = proxy_server(app.handle().clone()).await;
        let body = read_with_watchdog(client(&proxy).await).await;
        assert!(body.contains("event: ping"), "{format}: {body}");
        assert_eq!(body.matches("event: error").count(), 1, "{format}: {body}");
        assert!(!body.contains("event: message_stop"), "{format}: {body}");
        assert_eq!(profile(&app, "p1").consecutive_successes, 0);
        streaming_tests::assert_single_outcome(&app, 502, 0, 7);
    }
}

#[tokio::test]
async fn keepalive_before_the_first_answer_does_not_invent_content_or_usage() {
    for format in ["openai_chat", "openai_responses", "gemini_native"] {
        let upstream = active_server(PING, frames(format).1, false).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        open_profile(&app, "p1", true);
        let proxy = proxy_server(app.handle().clone()).await;
        let body = read_with_watchdog(client(&proxy).await).await;
        assert!(
            body.starts_with("event: ping\ndata: {\"type\":\"ping\"}\n\n"),
            "{format}: {body}"
        );
        assert!(body.matches("event: ping").count() >= 2, "{format}: {body}");
        assert!(!body.contains("text_delta"), "{format}: {body}");
        assert_eq!(
            body.matches("event: message_stop").count(),
            1,
            "{format}: {body}"
        );
        streaming_tests::assert_single_outcome(&app, 200, 1, 7);
    }
}

#[tokio::test]
async fn cancellation_after_a_keepalive_preserves_partial_usage_without_success() {
    for format in ["openai_chat", "openai_responses", "gemini_native"] {
        let upstream = active_server(frames(format).0, "", true).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        open_profile(&app, "p1", true);
        let response = forward(app.handle().clone(), true).await;
        let mut stream = response.into_body().into_data_stream();
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(chunk) = stream.next().await {
                if std::str::from_utf8(&chunk.unwrap())
                    .unwrap()
                    .contains("event: ping")
                {
                    return;
                }
            }
            panic!("no keepalive was delivered");
        })
        .await
        .unwrap();
        drop(stream);
        assert_eq!(profile(&app, "p1").consecutive_successes, 0);
        streaming_tests::assert_single_outcome(&app, 499, 0, 7);
    }
}

#[tokio::test]
async fn native_streams_retain_their_comments_and_opaque_event_bytes() {
    const NATIVE: &str = ": provider-alive\n\nevent: custom\ndata: {\"opaque\":999999999999999999999999999999,\"signature\":\"abc==\"}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let upstream = server(StatusCode::OK, "text/event-stream", NATIVE).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let proxy = proxy_server(app.handle().clone()).await;
    let body = read_with_watchdog(client(&proxy).await).await;
    assert_eq!(body, NATIVE);
    streaming_tests::assert_single_outcome(&app, 200, 1, 0);
}
