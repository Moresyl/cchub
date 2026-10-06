use super::*;

struct Cancelled(Arc<AtomicUsize>);
impl Drop for Cancelled {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

async fn held_server(frame: String) -> (Upstream, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let cancelled = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let dropped = cancelled.clone();
    let router = Router::new().fallback(any(move || {
        let frame = frame.clone();
        let dropped = dropped.clone();
        observed.fetch_add(1, Ordering::SeqCst);
        async move {
            let body = async_stream::stream! {
                let _guard = Cancelled(dropped);
                for chunk in frame.as_bytes().chunks(3) {
                    yield Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(chunk));
                    tokio::task::yield_now().await;
                }
                std::future::pending::<()>().await;
            };
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(Body::from_stream(body))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (Upstream { url, hits, task }, cancelled)
}

async fn assert_cancelled(cancelled: &AtomicUsize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while cancelled.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("upstream body was not cancelled");
}

const RATE_LIMIT: &str = "data: {\"error\":{\"code\":429,\"type\":\"rate_limit_error\",\"message\":\"private fixture secret\"}}\n\n";
const ANSWER: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"second answer\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";

async fn request_path(app: &App<MockRuntime>, path: &str) -> Response<Body> {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/proxy/claude/{path}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"model":"fixture-model","stream":true,"messages":[]}).to_string(),
        ))
        .unwrap();
    forward_proxy_request_with_client(
        app.handle().clone(),
        "claude".into(),
        path.into(),
        request,
        Some(reqwest::Client::builder().no_proxy().build().unwrap()),
    )
    .await
}

#[tokio::test]
async fn held_error_before_content_reaches_next_profile_without_waiting_for_eof() {
    let primary = streaming_tests::split_server_with_pending(RATE_LIMIT, true).await;
    let alternate = server(StatusCode::OK, "text/event-stream", ANSWER).await;
    let app = app(
        &[("p1", &primary.url, vec![]), ("p2", &alternate.url, vec![])],
        OptimizerConfig::default(),
    );
    let (status, body) = tokio::time::timeout(Duration::from_secs(2), async {
        let response = request_path(&app, "v1/chat/completions").await;
        (
            response.status(),
            to_bytes(response.into_body(), 8192).await.unwrap(),
        )
    })
    .await
    .expect("a parsed error must not wait for an open upstream socket");
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).contains("second answer"));
    assert!(!String::from_utf8_lossy(&body).contains("private fixture secret"));
    assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    assert_eq!(profile(&app, "p1").consecutive_failures, 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 0);
}

#[tokio::test]
async fn held_protocol_errors_cancel_immediately_and_keep_each_attempt_after_recovery_or_exhaustion(
) {
    for (path, format, prefix, failure, answer, input) in [
        ("v1/chat/completions", "anthropic", "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}],\"usage\":{\"prompt_tokens\":7}}\n\n", RATE_LIMIT, ANSWER, 7),
        ("v1/responses", "anthropic", "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"r1\",\"output\":[],\"usage\":{\"input_tokens\":7}}}\n\n", "event: response.failed\ndata: {\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"rate_limit_exceeded\"}}}\n\n", "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":11,\"output_tokens\":3}}}\n\n", 7),
        ("v1/messages", "anthropic", "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"content\":[],\"usage\":{\"input_tokens\":7,\"cache_read_input_tokens\":2}}}\n\n", "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}\n\n", "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n", 9),
        ("v1beta/models/gemini:streamGenerateContent", "anthropic", "data: {\"usageMetadata\":{\"promptTokenCount\":7}}\n\n", "data: {\"error\":{\"status\":\"RESOURCE_EXHAUSTED\"}}\n\n", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"second answer\"}]},\"finishReason\":\"STOP\"}]}\n\n", 7),
        ("v1/messages", "openai_chat", "data: {\"id\":\"c1\",\"choices\":[{\"delta\":{\"role\":\"assistant\"}}],\"usage\":{\"prompt_tokens\":7}}\n\n", RATE_LIMIT, ANSWER, 7),
        ("v1/messages", "openai_responses", "event: response.created\ndata: {\"response\":{\"id\":\"r1\",\"output\":[],\"usage\":{\"input_tokens\":7}}}\n\n", "event: response.failed\ndata: {\"response\":{\"error\":{\"code\":429}}}\n\n", "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"r2\",\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":11,\"output_tokens\":3}}}\n\n", 7),
        ("v1/messages", "gemini_native", "data: {\"usageMetadata\":{\"promptTokenCount\":7}}\n\n", "data: {\"error\":{\"code\":429}}\n\n", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"second answer\"}]},\"finishReason\":\"STOP\"}]}\n\n", 7),
    ] {
        for both_failed in [false, true] {
            let wire = format!("{prefix}{failure}").replace('\n', "\r\n");
            let (first, first_cancelled) = held_server(wire.clone()).await;
            let (last, last_cancelled) = if both_failed { held_server(wire).await } else { (server(StatusCode::OK, "text/event-stream", answer).await, Arc::new(AtomicUsize::new(0))) };
            let app = app(&[("p1", &first.url, vec![]), ("p2", &last.url, vec![])], OptimizerConfig { max_profile_retries: 1, ..Default::default() });
            set_format(&app, "p1", format);
            set_format(&app, "p2", format);
            open_profile(&app, "p2", true);
            let (status, body) = tokio::time::timeout(Duration::from_secs(3), async {
                let response = request_path(&app, path).await;
                (response.status(), to_bytes(response.into_body(), 16384).await.unwrap())
            }).await.expect("held errors must not wait for EOF");
            assert_eq!(status, if both_failed { StatusCode::TOO_MANY_REQUESTS } else { StatusCode::OK }, "{path}/{format}: {}", String::from_utf8_lossy(&body));
            assert!(!String::from_utf8_lossy(&body).contains("private fixture secret"));
            assert_cancelled(&first_cancelled).await;
            if both_failed { assert_cancelled(&last_cancelled).await; }
            assert_eq!(first.hits.load(Ordering::SeqCst), 1);
            assert_eq!(last.hits.load(Ordering::SeqCst), 1);
            let request_id: String = app.state::<DbState>().0.lock().unwrap().query_row("SELECT request_id FROM proxy_request_logs", [], |row| row.get(0)).unwrap();
            let record = crate::commands::usage_commands::get_request_detail(request_id, app.state()).unwrap().unwrap();
            assert_eq!(record.profile_id, "p2");
            assert_eq!(record.status_code, if both_failed { 429 } else { 200 }, "{path}/{format}: {}", String::from_utf8_lossy(&body));
            assert_eq!(record.stream_attempts.len(), if both_failed { 2 } else { 1 });
            assert_eq!(record.stream_attempts[0].input_tokens, input);
            assert_eq!(record.stream_attempts[0].profile_id, "p1");
            assert_eq!(record.stream_attempts[0].status_code, 429);
            assert_eq!(profile(&app, "p1").consecutive_failures, 1);
            assert_eq!(profile(&app, "p2").consecutive_successes, if both_failed { 0 } else { 1 });
        }
    }
}

#[tokio::test]
async fn committed_reasoning_tools_text_and_unknown_events_never_retry_and_errors_close_the_socket()
{
    for (path, prefix) in [
        ("v1/chat/completions", "data: {\"choices\":[{\"delta\":{\"content\":\"visible\"}}]}\n\n"),
        ("v1/chat/completions", "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think\"}}]}\n\n"),
        ("v1/chat/completions", "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call1\",\"function\":{\"name\":\"write\",\"arguments\":\"{}\"}}]}}]}\n\n"),
        ("v1/messages", "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"tool_use\",\"id\":\"call1\",\"name\":\"write\",\"input\":{}}}\n\n"),
        ("v1/responses", "event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call1\",\"name\":\"write\"}}\n\n"),
        ("v1/responses", "event: custom\ndata: {\"action\":\"unknown\"}\n\n"),
        ("v1beta/models/gemini:streamGenerateContent", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"think\",\"thought\":true}]}}]}\n\n"),
    ] {
        let (first, cancelled) = held_server(format!("{prefix}{RATE_LIMIT}")).await;
        let last = server(StatusCode::OK, "text/event-stream", ANSWER).await;
        let app = app(&[("p1", &first.url, vec![]), ("p2", &last.url, vec![])], OptimizerConfig::default());
        let response = request_path(&app, path).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = tokio::time::timeout(Duration::from_secs(2), to_bytes(response.into_body(), 16384)).await.unwrap().unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("error"));
        assert_eq!(last.hits.load(Ordering::SeqCst), 0, "{path}");
        assert_cancelled(&cancelled).await;
        streaming_tests::assert_single_outcome(&app, 502, 0, 0);
    }
}

#[tokio::test]
async fn invalid_request_and_disabled_profile_failover_do_not_spend_another_attempt() {
    for (failure, enabled, status) in [
        (
            "event: error\ndata: {\"error\":{\"type\":\"invalid_request_error\"}}\n\n",
            true,
            StatusCode::BAD_REQUEST,
        ),
        (RATE_LIMIT, false, StatusCode::TOO_MANY_REQUESTS),
    ] {
        let (first, cancelled) = held_server(failure.into()).await;
        let last = server(StatusCode::OK, "text/event-stream", ANSWER).await;
        let app = app(
            &[("p1", &first.url, vec![]), ("p2", &last.url, vec![])],
            OptimizerConfig {
                failover_enabled: enabled,
                ..Default::default()
            },
        );
        let response = request_path(&app, "v1/chat/completions").await;
        assert_eq!(response.status(), status);
        to_bytes(response.into_body(), 8192).await.unwrap();
        assert_eq!(last.hits.load(Ordering::SeqCst), 0);
        assert_cancelled(&cancelled).await;
    }
}

#[tokio::test]
async fn accounting_failure_stops_before_requesting_another_provider_and_keeps_latest_usage() {
    let (first, _) = held_server(format!(
        "data: {{\"usage\":{{\"prompt_tokens\":7}}}}\n\n{RATE_LIMIT}"
    ))
    .await;
    let last = server(StatusCode::OK, "text/event-stream", ANSWER).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &last.url, vec![])],
        OptimizerConfig::default(),
    );
    app.state::<DbState>().0.lock().unwrap().execute_batch("CREATE TRIGGER fail_attempt BEFORE UPDATE OF stream_attempts_json ON proxy_request_logs BEGIN SELECT RAISE(ABORT,'private database secret'); END;").unwrap();
    let response = request_path(&app, "v1/chat/completions").await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    assert!(!String::from_utf8_lossy(&body).contains("private database secret"));
    assert_eq!(last.hits.load(Ordering::SeqCst), 0);
    streaming_tests::assert_single_outcome(&app, 429, 0, 7);
}

#[tokio::test]
async fn parent_accounting_failure_stops_before_requesting_another_provider() {
    let (first, _) = held_server(format!(
        "data: {{\"usage\":{{\"prompt_tokens\":7}}}}\n\n{RATE_LIMIT}"
    ))
    .await;
    let last = server(StatusCode::OK, "text/event-stream", ANSWER).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &last.url, vec![])],
        OptimizerConfig::default(),
    );
    app.state::<DbState>().0.lock().unwrap().execute_batch(
        "CREATE TRIGGER fail_parent BEFORE INSERT ON proxy_request_logs BEGIN SELECT RAISE(ABORT,'private parent storage secret'); END;"
    ).unwrap();
    let response = request_path(&app, "v1/chat/completions").await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    assert!(!String::from_utf8_lossy(&body).contains("private parent storage secret"));
    assert_eq!(first.hits.load(Ordering::SeqCst), 1);
    assert_eq!(last.hits.load(Ordering::SeqCst), 0);
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM proxy_request_logs", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn retry_attempt_ledger_survives_sql_backup_and_is_removed_with_its_request() {
    let (first, _) = held_server(format!(
        "data: {{\"usage\":{{\"prompt_tokens\":7}}}}\n\n{RATE_LIMIT}"
    ))
    .await;
    let last = server(StatusCode::OK, "text/event-stream", ANSWER).await;
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &last.url, vec![])],
        OptimizerConfig::default(),
    );
    let response = request_path(&app, "v1/chat/completions").await;
    to_bytes(response.into_body(), 8192).await.unwrap();
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let original: String = conn
        .query_row(
            "SELECT stream_attempts_json FROM proxy_request_logs",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut backup = crate::db::schema::get_schema_sql();
    crate::commands::extra_commands::append_backup_database_rows(&conn, &mut backup);
    let restored = Connection::open_in_memory().unwrap();
    restored.execute_batch(&backup).unwrap();
    crate::db::schema::run_migrations(&restored).unwrap();
    let ledger: String = restored
        .query_row(
            "SELECT stream_attempts_json FROM proxy_request_logs",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(ledger, original);
    assert!(ledger.contains("\"input_tokens\":7"));
    assert!(!ledger.contains("private fixture secret"));
    restored
        .execute("DELETE FROM proxy_request_logs", [])
        .unwrap();
    assert_eq!(
        restored
            .query_row("SELECT COUNT(*) FROM proxy_request_logs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
