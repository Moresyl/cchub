use super::*;
use crate::db::{schema, DbState};
use rusqlite::Connection;
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    App, Manager,
};

fn app() -> App<MockRuntime> {
    let conn = Connection::open_in_memory().unwrap();
    schema::run_migrations(&conn).unwrap();
    mock_builder()
        .manage(DbState(Mutex::new(conn)))
        .build(mock_context(noop_assets()))
        .unwrap()
}

fn target() -> UpstreamTarget {
    UpstreamTarget {
        profile_id: "fixture-profile".into(),
        profile_name: "Fixture".into(),
        base_url: "https://fixture.invalid".into(),
        use_full_url: false,
        candidate_base_urls: vec!["https://other.invalid".into()],
        headers: vec![("Authorization".into(), "private-token".into())],
        managed_principal: None,
        affinity: None,
        request_header_overrides: vec![("X-Key".into(), "private-token".into())],
        request_body_override: Some(serde_json::json!({"private":"body"})),
        claude_api_format: None,
        is_github_copilot: false,
        is_codex_oauth: false,
        cost_multiplier: 2.0,
    }
}

fn pending() -> PendingRecord {
    PendingRecord::new(
        "fixture-request",
        "claude",
        &target(),
        &ProxyRequestInsights::default(),
        Some(&ProxyUsageMetrics {
            input_tokens: 10,
            ..Default::default()
        }),
        None,
        20,
        502,
        Some("failed"),
    )
}

#[test]
fn captured_accounting_omits_request_credentials_and_body() {
    let record = pending();
    assert!(record.upstream.headers.is_empty());
    assert!(record.upstream.request_header_overrides.is_empty());
    assert!(record.upstream.candidate_base_urls.is_empty());
    assert!(record.upstream.base_url.is_empty());
    assert!(record.upstream.request_body_override.is_none());
    assert_eq!(record.upstream.cost_multiplier, 2.0);
    assert_eq!(record.usage.input_tokens, 10);
}

#[tokio::test(flavor = "current_thread")]
async fn database_contention_keeps_the_runtime_responsive_and_record_timestamp() {
    let app = app();
    let record = pending();
    let captured = record.created_at.clone();
    let held_app = app.handle().clone();
    let (locked, ready) = tokio::sync::oneshot::channel();
    let (release, blocked) = mpsc::channel();
    let holder = std::thread::spawn(move || {
        let db = held_app.state::<DbState>();
        let _guard = db.0.lock().unwrap();
        locked.send(()).unwrap();
        blocked.recv_timeout(Duration::from_secs(3)).unwrap();
    });
    ready.await.unwrap();
    let worker_app = app.handle().clone();
    let writing = tokio::spawn(super::super::dispatch::write(
        &super::super::reserve_accounting().await.unwrap(),
        move || record.persist(&worker_app),
    ));
    let responsive = tokio::time::timeout(Duration::from_millis(100), async {
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;
    })
    .await;
    release.send(()).unwrap();
    holder.join().unwrap();
    writing.await.unwrap().unwrap();
    assert!(responsive.is_ok());
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let stored: String = conn
        .query_row("SELECT created_at FROM proxy_request_logs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(stored, captured);
}

#[tokio::test(flavor = "current_thread")]
async fn failed_parent_write_cannot_append_to_an_earlier_stream_attempt() {
    let app = app();
    pending().persist(app.handle()).unwrap();
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().unwrap();
        conn.execute_batch("CREATE TRIGGER reject_accounting BEFORE UPDATE ON proxy_request_logs BEGIN SELECT RAISE(ABORT, 'private storage details'); END;").unwrap();
    }
    let result = super::super::log_failed_stream_attempt(
        app.handle(),
        "fixture-request",
        "claude",
        &target(),
        &ProxyRequestInsights::default(),
        Some(&ProxyUsageMetrics {
            input_tokens: 99,
            ..Default::default()
        }),
        30,
        502,
        "stream failed",
        &super::super::reserve_accounting().await.unwrap(),
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        "Failed stream accounting could not be retained; no further provider was requested"
    );
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let stored: (i64, String) = conn
        .query_row(
            "SELECT input_tokens,stream_attempts_json FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored, (10, "[]".into()));
}

#[tokio::test(flavor = "current_thread")]
async fn paid_stream_attempt_is_durable_before_the_caller_continues() {
    let app = app();
    super::super::log_failed_stream_attempt(
        app.handle(),
        "fixture-request",
        "claude",
        &target(),
        &ProxyRequestInsights::default(),
        Some(&ProxyUsageMetrics {
            input_tokens: 21,
            ..Default::default()
        }),
        30,
        502,
        "stream failed",
        &super::super::reserve_accounting().await.unwrap(),
    )
    .await
    .unwrap();
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let source: String = conn
        .query_row(
            "SELECT stream_attempts_json FROM proxy_request_logs",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let attempts: Vec<super::super::StreamAttempt> = serde_json::from_str(&source).unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].input_tokens, 21);
    assert_eq!(attempts[0].profile_id, "fixture-profile");
}
