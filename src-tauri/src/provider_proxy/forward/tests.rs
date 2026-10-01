use super::forward_proxy_request_with_client;
use crate::db::{schema, DbState};
use crate::provider_proxy::circuits::{CircuitState, EndpointCircuitState};
use crate::provider_proxy::profiles::{endpoint_circuit_key, profile_circuit_key};
use crate::provider_proxy::{LocalProviderProxyRuntime, LocalProviderProxyRuntimeInner};
use crate::proxy_optimizer::OptimizerConfig;
use axum::body::{to_bytes, Body};
use axum::http::{Request, Response, StatusCode};
use axum::{routing::any, Router};
use rusqlite::Connection;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, AppHandle, Manager};
use tokio::sync::Notify;

#[path = "affinity_tests.rs"]
mod affinity_tests;
#[path = "bounded_stream_tests.rs"]
mod bounded_stream_tests;
#[path = "cache_usage_tests.rs"]
mod cache_usage_tests;
#[path = "chat_compat_tests.rs"]
mod chat_compat_tests;
#[path = "chat_parts_tests.rs"]
mod chat_parts_tests;
#[path = "deadline_tests.rs"]
mod deadline_tests;
#[path = "gemini_tool_tests.rs"]
mod gemini_tool_tests;
#[path = "gemini_usage_tests.rs"]
mod gemini_usage_tests;
#[path = "keepalive_tests.rs"]
mod keepalive_tests;
#[path = "managed_auth_tests.rs"]
mod managed_auth_tests;
#[path = "message_id_tests.rs"]
mod message_id_tests;
#[path = "model_alias_tests.rs"]
mod model_alias_tests;
#[path = "passthrough_tests.rs"]
mod passthrough_tests;
#[path = "preflight_tests.rs"]
mod preflight_tests;
#[path = "responses_history_tests.rs"]
mod responses_history_tests;
#[path = "responses_reasoning_tests.rs"]
mod responses_reasoning_tests;
#[path = "routing_tests.rs"]
mod routing_tests;
#[path = "streaming_tests.rs"]
mod streaming_tests;
#[path = "strict_tools_tests.rs"]
mod strict_tools_tests;
#[path = "terminal_drop_tests.rs"]
mod terminal_drop_tests;
#[path = "transport_cut_tests.rs"]
mod transport_cut_tests;

struct Upstream {
    url: String,
    hits: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Upstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn server(status: StatusCode, content_type: &'static str, body: &'static str) -> Upstream {
    controlled_server(status, content_type, body, None, false).await
}

async fn controlled_server(
    status: StatusCode,
    content_type: &'static str,
    body: &'static str,
    gate: Option<Arc<Notify>>,
    pending_body: bool,
) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let router = Router::new().fallback(any(move || {
        let observed = observed.clone();
        let gate = gate.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = gate {
                gate.notified().await;
            }
            let body = if pending_body {
                Body::from_stream(futures_util::stream::pending::<
                    Result<bytes::Bytes, std::io::Error>,
                >())
            } else {
                Body::from(body)
            };
            Response::builder()
                .status(status)
                .header("content-type", content_type)
                .header("retry-after", "17")
                .body(body)
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Upstream { url, hits, task }
}

fn app(profiles: &[(&str, &str, Vec<String>)], config: OptimizerConfig) -> App<MockRuntime> {
    let conn = Connection::open_in_memory().unwrap();
    schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO app_settings(key,value) VALUES('local_provider_proxy_settings',?1)",
        [json!({"enabled_apps":["claude"],"port":34567}).to_string()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO app_settings(key,value) VALUES('current_profile_claude',?1)",
        [profiles[0].0],
    )
    .unwrap();
    for (index, (id, url, alternatives)) in profiles.iter().enumerate() {
        conn.execute("INSERT INTO config_profiles(id,name,tool_id,config_snapshot,sort_order) VALUES(?1,?1,'claude',?2,?3)",
            rusqlite::params![id, json!({"env":{"ANTHROPIC_BASE_URL":url,"ANTHROPIC_AUTH_TOKEN":"fixture-token"},
                "metadata":{"endpointCandidates":alternatives}}).to_string(), index as i64]).unwrap();
    }
    mock_builder()
        .manage(DbState(Mutex::new(conn)))
        .manage(LocalProviderProxyRuntime(Arc::new(Mutex::new(
            LocalProviderProxyRuntimeInner {
                optimizer_config: Some(config),
                ..Default::default()
            },
        ))))
        .build(mock_context(noop_assets()))
        .unwrap()
}

async fn forward(app: AppHandle<MockRuntime>, streaming: bool) -> Response<Body> {
    let request = Request::builder()
        .method("POST")
        .uri("/proxy/claude/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"model":"fixture-model","stream":streaming,"messages":[]}).to_string(),
        ))
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        forward_proxy_request_with_client(
            app,
            "claude".into(),
            "v1/messages".into(),
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        ),
    )
    .await
    .expect("forwarding must terminate")
}

fn profile(app: &App<MockRuntime>, id: &str) -> EndpointCircuitState {
    app.state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .profile_circuits
        .get(&profile_circuit_key("claude", id))
        .unwrap()
        .clone()
}

fn endpoint(app: &App<MockRuntime>, id: &str, url: &str) -> EndpointCircuitState {
    app.state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .endpoint_circuits
        .get(&endpoint_circuit_key(id, url))
        .unwrap()
        .clone()
}

fn open_profile(app: &App<MockRuntime>, id: &str, expired: bool) {
    let mut state = EndpointCircuitState::default();
    state.state = CircuitState::Open;
    state.open_until = Some(if expired {
        Instant::now() - Duration::from_secs(1)
    } else {
        Instant::now() + Duration::from_secs(30)
    });
    app.state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .profile_circuits
        .insert(profile_circuit_key("claude", id), state);
}

#[tokio::test]
async fn status_failure_tries_each_endpoint_once_and_recovers_profile() {
    let primary = server(
        StatusCode::SERVICE_UNAVAILABLE,
        "application/json",
        r#"{"error":"offline"}"#,
    )
    .await;
    let alternate = server(
        StatusCode::OK,
        "application/json",
        r#"{"content":[],"model":"fixture-model"}"#,
    )
    .await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    let response = forward(app.handle().clone(), false).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
    assert_eq!(
        app.state::<LocalProviderProxyRuntime>()
            .0
            .lock()
            .unwrap()
            .preferred_base_urls["p1"],
        alternate.url
    );
}

#[tokio::test]
async fn network_failure_moves_to_alternate_instead_of_looping() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refused = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let alternate = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[("p1", &refused, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    assert_eq!(endpoint(&app, "p1", &refused).consecutive_failures, 1);
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
}

#[tokio::test]
async fn exhausted_endpoints_record_one_profile_failure_and_keep_vendor_reply() {
    let primary = server(
        StatusCode::TOO_MANY_REQUESTS,
        "application/json",
        r#"{"error":"limited"}"#,
    )
    .await;
    let alternate = server(
        StatusCode::TOO_MANY_REQUESTS,
        "application/json",
        r#"{"error":"still limited"}"#,
    )
    .await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    let response = forward(app.handle().clone(), false).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["retry-after"], "17");
    assert!(
        String::from_utf8(to_bytes(response.into_body(), 1024).await.unwrap().to_vec())
            .unwrap()
            .contains("still limited")
    );
    assert_eq!(profile(&app, "p1").consecutive_failures, 1);
    assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn disabled_failover_and_zero_budget_preserve_status_headers_and_body() {
    for config in [
        OptimizerConfig {
            failover_enabled: false,
            ..Default::default()
        },
        OptimizerConfig {
            max_profile_retries: 0,
            ..Default::default()
        },
    ] {
        let primary = server(
            StatusCode::TOO_MANY_REQUESTS,
            "application/json",
            r#"{"error":"quota"}"#,
        )
        .await;
        let secondary = server(StatusCode::OK, "application/json", "{}").await;
        let app = app(
            &[("p1", &primary.url, vec![]), ("p2", &secondary.url, vec![])],
            config,
        );
        let response = forward(app.handle().clone(), false).await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()["retry-after"], "17");
        assert_eq!(secondary.hits.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn all_open_circuits_never_bypass_the_cooldown() {
    let upstream = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", false);
    let response = forward(app.handle().clone(), false).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let retry_after: u64 = response.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((29..=31).contains(&retry_after));
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn untried_candidates_do_not_reserve_their_recovery_probe() {
    let primary = server(StatusCode::OK, "application/json", "{}").await;
    let secondary = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[("p1", &primary.url, vec![]), ("p2", &secondary.url, vec![])],
        OptimizerConfig::default(),
    );
    open_profile(&app, "p2", true);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(profile(&app, "p2").state, CircuitState::Open);
    assert!(profile(&app, "p2").is_available());
}

#[tokio::test]
async fn malformed_success_body_is_failed_and_uses_alternate() {
    let primary = server(StatusCode::OK, "application/json", "broken JSON").await;
    let alternate = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn successful_recovery_requires_two_completed_requests() {
    let upstream = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(profile(&app, "p1").state, CircuitState::HalfOpen);
    assert_eq!(profile(&app, "p1").consecutive_successes, 1);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(profile(&app, "p1").state, CircuitState::Closed);
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn client_error_is_neutral_to_recovery() {
    let upstream = server(
        StatusCode::UNAUTHORIZED,
        "application/json",
        r#"{"error":"invalid credential"}"#,
    )
    .await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(profile(&app, "p1").state, CircuitState::HalfOpen);
    assert_eq!(profile(&app, "p1").consecutive_successes, 0);
    assert!(profile(&app, "p1").is_available());
}

#[tokio::test]
async fn only_one_recovery_request_runs_and_abort_releases_it() {
    let gate = Arc::new(Notify::new());
    let upstream =
        controlled_server(StatusCode::OK, "application/json", "{}", Some(gate), false).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let pending = tokio::spawn(forward(app.handle().clone(), false));
    tokio::time::timeout(Duration::from_secs(2), async {
        while upstream.hits.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert!(profile(&app, "p1").is_available());
    assert!(endpoint(&app, "p1", &upstream.url).is_available());
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
}

#[tokio::test]
async fn dropped_unpolled_stream_releases_probe_without_success() {
    let upstream = server(StatusCode::OK, "text/event-stream", "data: {}\n\n").await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!profile(&app, "p1").is_available());
    drop(response);
    assert!(profile(&app, "p1").is_available());
    assert_eq!(profile(&app, "p1").consecutive_successes, 0);
    streaming_tests::assert_single_outcome(&app, 499, 0, 0);
}

#[tokio::test]
async fn stream_timeout_is_failure_instead_of_healthy_headers() {
    let upstream = controlled_server(StatusCode::OK, "text/event-stream", "", None, true).await;
    let app = app(
        &[("p1", &upstream.url, vec![])],
        OptimizerConfig {
            streaming_first_byte_timeout: 1,
            ..Default::default()
        },
    );
    open_profile(&app, "p1", true);
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(profile(&app, "p1").consecutive_successes, 0);
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("first byte timeout"));
    assert_eq!(profile(&app, "p1").state, CircuitState::Open);
    assert_eq!(endpoint(&app, "p1", &upstream.url).consecutive_failures, 1);
    streaming_tests::assert_single_outcome(&app, 502, 0, 0);
}

fn set_format(app: &App<MockRuntime>, id: &str, format: &str) {
    assert!(
        format == "anthropic"
            || crate::provider_proxy::ClaudeApiFormat::from_str(format).needs_transform()
    );
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let snapshot: String = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    let mut snapshot: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    snapshot["env"]["ANTHROPIC_API_FORMAT"] = json!(format);
    conn.execute(
        "UPDATE config_profiles SET config_snapshot=?1 WHERE id=?2",
        rusqlite::params![snapshot.to_string(), id],
    )
    .unwrap();
}

#[tokio::test]
async fn retryable_failure_switches_to_the_next_profile() {
    let primary = server(StatusCode::SERVICE_UNAVAILABLE, "application/json", "{}").await;
    let secondary = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[("p1", &primary.url, vec![]), ("p2", &secondary.url, vec![])],
        OptimizerConfig::default(),
    );
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(primary.hits.load(Ordering::SeqCst), 1);
    assert_eq!(secondary.hits.load(Ordering::SeqCst), 1);
    assert_eq!(profile(&app, "p1").consecutive_failures, 1);
    assert_eq!(profile(&app, "p2").consecutive_failures, 0);
}

#[tokio::test]
async fn later_blocked_profiles_preserve_the_vendor_error_and_translation() {
    for format in ["anthropic", "openai_chat"] {
        let primary = server(
            StatusCode::TOO_MANY_REQUESTS,
            "application/json",
            r#"{"error":{"message":"vendor quota"}}"#,
        )
        .await;
        let secondary = server(StatusCode::OK, "application/json", "{}").await;
        let app = app(
            &[("p1", &primary.url, vec![]), ("p2", &secondary.url, vec![])],
            OptimizerConfig::default(),
        );
        set_format(&app, "p1", format);
        open_profile(&app, "p2", false);
        let response = forward(app.handle().clone(), false).await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()["retry-after"], "17");
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(body["error"]["message"], "vendor quota");
        if format != "anthropic" {
            assert_eq!(body["type"], "error");
        }
        assert_eq!(secondary.hits.load(Ordering::SeqCst), 0);
        streaming_tests::assert_single_outcome(&app, 429, 0, 0);
    }
}

#[tokio::test]
async fn disabled_failover_never_selects_an_alternate_when_primary_is_open() {
    let primary = server(StatusCode::OK, "application/json", "{}").await;
    let secondary = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[("p1", &primary.url, vec![]), ("p2", &secondary.url, vec![])],
        OptimizerConfig {
            failover_enabled: false,
            ..Default::default()
        },
    );
    open_profile(&app, "p1", false);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(primary.hits.load(Ordering::SeqCst), 0);
    assert_eq!(secondary.hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn all_open_endpoints_are_not_retried_as_a_fallback() {
    let upstream = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    let mut state = EndpointCircuitState::default();
    state.state = CircuitState::Open;
    state.open_until = Some(Instant::now() + Duration::from_secs(30));
    app.state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .unwrap()
        .endpoint_circuits
        .insert(endpoint_circuit_key("p1", &upstream.url), state);
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 0);
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
}

#[tokio::test]
async fn successful_stream_is_healthy_only_after_body_completion() {
    let upstream = server(
        StatusCode::OK,
        "text/event-stream",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    )
    .await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    for successes in [1, 2] {
        let response = forward(app.handle().clone(), true).await;
        assert_eq!(profile(&app, "p1").consecutive_successes, successes - 1);
        assert!(!profile(&app, "p1").is_available());
        assert!(!to_bytes(response.into_body(), 4096)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(profile(&app, "p1").consecutive_successes, successes);
    }
    assert_eq!(profile(&app, "p1").state, CircuitState::Closed);
}

#[tokio::test]
async fn initial_in_band_errors_return_sanitized_failures_and_never_count_as_recovery() {
    for (format, event) in [
        ("anthropic", "event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"vendor quota\"}}\n\n"),
        ("openai_chat", "data: {\"error\":{\"message\":\"vendor quota\"}}\n\n"),
        ("openai_responses", "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"message\":\"vendor quota\"}}}\n\n"),
        ("gemini_native", "data: {\"error\":{\"message\":\"vendor quota\"}}\n\n"),
    ] {
        let upstream = server(StatusCode::OK, "text/event-stream", event).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        open_profile(&app, "p1", true);
        let response = forward(app.handle().clone(), true).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY, "{format}");
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["error"]["code"], 502, "{format}: {text}");
        assert!(!text.contains("vendor quota"), "{format}: {text}");
        assert_eq!(profile(&app, "p1").state, CircuitState::Open, "{format}");
        assert_eq!(profile(&app, "p1").consecutive_successes, 0, "{format}");
        streaming_tests::assert_single_outcome(&app, 502, 0, 0);
    }
}

#[tokio::test]
async fn truncated_response_body_uses_alternate_without_reporting_success() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for content_type in ["application/json", "text/plain"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            connection.read(&mut request).await.unwrap();
            connection.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{{}}").as_bytes()).await.unwrap();
            connection.shutdown().await.unwrap();
        });
        let alternate = server(StatusCode::OK, "application/json", "{}").await;
        let app = app(
            &[("p1", &url, vec![alternate.url.clone()])],
            OptimizerConfig::default(),
        );
        assert_eq!(
            forward(app.handle().clone(), false).await.status(),
            StatusCode::OK
        );
        task.await.unwrap();
        assert_eq!(endpoint(&app, "p1", &url).consecutive_failures, 1);
        assert_eq!(profile(&app, "p1").consecutive_failures, 0);
        assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn incomplete_streams_fail_without_fabricating_a_normal_completion() {
    for (format, event) in [
        ("anthropic", "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"fixture-model\"}}\n\n"),
        ("openai_chat", "data: {\"id\":\"chat-1\",\"model\":\"fixture-model\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hello\"},\"finish_reason\":null}]}\n\n"),
        ("openai_responses", "event: response.created\ndata: {\"response\":{\"id\":\"response-1\",\"model\":\"fixture-model\"}}\n\n"),
        ("gemini_native", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hello\"}]}}]}\n\n"),
    ] {
        let upstream = server(StatusCode::OK, "text/event-stream", event).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        set_format(&app, "p1", format);
        open_profile(&app, "p1", true);
        let response = forward(app.handle().clone(), true).await;
        let body = to_bytes(response.into_body(), 8192).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("event: error"), "{format}: {text}");
        assert!(text.contains("api_error"), "{format}: {text}");
        assert!(!text.contains("event: message_stop"), "{format}: {text}");
        assert_eq!(profile(&app, "p1").state, CircuitState::Open, "{format}");
        assert_eq!(profile(&app, "p1").consecutive_successes, 0, "{format}");
        streaming_tests::assert_single_outcome(&app, 502, 0, 0);
    }
}

#[tokio::test]
async fn request_rectification_retries_the_same_endpoint_only_while_body_changes() {
    let upstream = server(
        StatusCode::BAD_REQUEST,
        "application/json",
        r#"{"error":{"message":"thinking.budget_tokens: Input should be greater than or equal to 1024"}}"#,
    )
    .await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let request = Request::builder().method("POST").uri("/proxy/claude/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(json!({"model":"fixture-model","max_tokens":2048,"thinking":{"type":"enabled","budget_tokens":512},"messages":[{"role":"user","content":"hello"}]}).to_string())).unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        forward_proxy_request_with_client(
            app.handle().clone(),
            "claude".into(),
            "v1/messages".into(),
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        ),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(upstream.hits.load(Ordering::SeqCst), 2);
    assert!(profile(&app, "p1").is_available());
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
    assert_eq!(profile(&app, "p1").consecutive_successes, 0);
}

#[tokio::test]
async fn successful_json_reply_to_a_stream_request_is_processed_as_json() {
    let upstream = server(
        StatusCode::OK,
        "application/json",
        r#"{"type":"message","model":"fixture-model","content":[],"stop_reason":"end_turn"}"#,
    )
    .await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    open_profile(&app, "p1", true);
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    assert!(to_bytes(response.into_body(), 4096).await.is_ok());
    assert_eq!(profile(&app, "p1").consecutive_successes, 1);
}

#[tokio::test]
async fn json_error_with_success_status_uses_alternate_without_healing_primary() {
    let primary = server(
        StatusCode::OK,
        "application/json",
        r#"{"error":{"message":"upstream failure"}}"#,
    )
    .await;
    let alternate = server(StatusCode::OK, "application/json", "{}").await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::OK
    );
    assert_eq!(endpoint(&app, "p1", &primary.url).consecutive_failures, 1);
    assert_eq!(profile(&app, "p1").consecutive_failures, 0);
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    streaming_tests::assert_single_outcome(&app, 200, 1, 0);
}

#[tokio::test]
async fn retained_vendor_reply_owns_the_log_after_a_later_parse_failure() {
    let primary = server(
        StatusCode::TOO_MANY_REQUESTS,
        "application/json",
        r#"{"error":{"message":"quota"}}"#,
    )
    .await;
    let alternate = server(StatusCode::OK, "application/json", "invalid JSON").await;
    let app = app(
        &[("p1", &primary.url, vec![alternate.url.clone()])],
        OptimizerConfig::default(),
    );
    let response = forward(app.handle().clone(), false).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(alternate.hits.load(Ordering::SeqCst), 1);
    streaming_tests::assert_single_outcome(&app, 429, 0, 0);
    let db = app.state::<DbState>();
    let message: String =
        db.0.lock()
            .unwrap()
            .query_row("SELECT error_message FROM proxy_request_logs", [], |row| {
                row.get(0)
            })
            .unwrap();
    assert_eq!(message, "quota");
}

#[tokio::test]
async fn exhausted_malformed_bodies_log_the_final_gateway_failure_once() {
    let upstream = server(StatusCode::OK, "application/json", "invalid JSON").await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    assert_eq!(
        forward(app.handle().clone(), false).await.status(),
        StatusCode::BAD_GATEWAY
    );
    streaming_tests::assert_single_outcome(&app, 502, 0, 0);
}
