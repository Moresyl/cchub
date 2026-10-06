use super::*;
use crate::db::{schema, DbState};
use crate::provider_proxy::cost::{drain_accounting, reserve_accounting, Lifecycle};
use futures_util::StreamExt;
use rusqlite::Connection;
use std::time::Duration;
use tauri::{
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    App, Manager,
};

async fn fixture() -> (App<MockRuntime>, StreamRecord<MockRuntime>) {
    let conn = Connection::open_in_memory().unwrap();
    schema::run_migrations(&conn).unwrap();
    conn.execute_batch("CREATE TABLE accounting_writes(id INTEGER PRIMARY KEY); CREATE TRIGGER count_accounting_writes BEFORE INSERT ON proxy_request_logs BEGIN INSERT INTO accounting_writes(id) VALUES(NULL); END;").unwrap();
    let app = mock_builder()
        .manage(DbState(Mutex::new(conn)))
        .build(mock_context(noop_assets()))
        .unwrap();
    let capture = super::super::UsageCapture::default();
    super::super::capture_stream_usage(
        futures_util::stream::iter([Ok::<_, std::io::Error>(bytes::Bytes::from_static(
            b"data: {\"usage\":{\"prompt_tokens\":7}}\n\n",
        ))]),
        capture.clone(),
        crate::shared::token_usage::InputTokenBasis::IncludesCache,
    )
    .collect::<Vec<_>>()
    .await;
    let record = StreamRecord {
        app_handle: app.handle().clone(),
        request_id: uuid::Uuid::new_v4().to_string(),
        tool_id: "claude".into(),
        upstream: UpstreamTarget {
            profile_id: "fixture".into(),
            profile_name: "Fixture".into(),
            base_url: String::new(),
            use_full_url: false,
            candidate_base_urls: vec![],
            headers: vec![],
            managed_principal: None,
            affinity: None,
            request_header_overrides: vec![],
            request_body_override: None,
            claude_api_format: None,
            is_github_copilot: false,
            is_codex_oauth: false,
            cost_multiplier: 1.0,
        },
        insights: ProxyRequestInsights::default(),
        started_at: Instant::now(),
        upstream_status: 200,
        status_code: 499,
        error_message: Some("Client disconnected".into()),
        usage: ProxyUsageMetrics::default(),
        health: crate::provider_proxy::forward::streaming_health::StreamHealth::default(),
        capture,
        timing: super::super::StreamTimingCapture::new(Instant::now()),
        accounting: Some(reserve_accounting().await.unwrap()),
    };
    (app, record)
}

fn assert_record(app: &App<MockRuntime>, status: u16) {
    drain_accounting(Duration::from_secs(5)).unwrap();
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let actual: (i64, u16, i64) = conn
        .query_row(
            "SELECT COUNT(*),status_code,input_tokens FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(actual, (1, status, 7));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM accounting_writes", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT total_requests FROM proxy_usage_daily_rollups",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn shutdown_snapshots_a_retained_unpolled_stream_once_and_releases_its_capacity() {
    let (app, record) = fixture().await;
    let lifecycle = Arc::new(Lifecycle::default());
    let log = StreamRequestLog::new_in(record, lifecycle.clone());
    lifecycle.close();
    assert!(log.record.lock().unwrap().accounting.is_none());
    assert_record(&app, 499);
    drop(log);
    lifecycle.close();
    assert_record(&app, 499);
}

#[tokio::test]
async fn normal_completion_and_shutdown_do_not_submit_the_same_stream_twice() {
    let (app, record) = fixture().await;
    let lifecycle = Arc::new(Lifecycle::default());
    let log = StreamRequestLog::new_in(record, lifecycle.clone());
    log.complete();
    log.submit().await;
    lifecycle.close();
    drop(log);
    assert_record(&app, 200);
}

#[tokio::test]
async fn a_stream_registered_during_shutdown_records_cancellation_immediately() {
    let (app, record) = fixture().await;
    let lifecycle = Arc::new(Lifecycle::default());
    lifecycle.close();
    let log = StreamRequestLog::new_in(record, lifecycle);
    assert!(log.record.lock().unwrap().accounting.is_none());
    assert_record(&app, 499);
    drop(log);
    assert_record(&app, 499);
}
