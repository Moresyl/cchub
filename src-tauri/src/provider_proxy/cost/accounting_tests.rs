use super::*;
use crate::db::schema;

fn write(
    conn: &Connection,
    id: &str,
    status: u16,
    tokens: u64,
    latency: u64,
    cost: f64,
    created_at: &str,
) -> rusqlite::Result<()> {
    let upstream = UpstreamTarget {
        profile_id: format!("profile-{status}"),
        profile_name: format!("provider-{status}"),
        base_url: "http://127.0.0.1".into(),
        use_full_url: false,
        candidate_base_urls: vec![],
        headers: vec![],
        managed_principal: None,
        request_header_overrides: vec![],
        request_body_override: None,
        claude_api_format: None,
        is_github_copilot: false,
        is_codex_oauth: false,
        cost_multiplier: 1.0,
    };
    let usage = ProxyUsageMetrics {
        input_tokens: tokens,
        output_tokens: tokens / 2,
        cache_read_tokens: tokens / 3,
        cache_creation_tokens: tokens / 4,
        ..Default::default()
    };
    persist_request(
        conn,
        &RequestRecord {
            request_id: id,
            tool_id: "claude",
            upstream: &upstream,
            insights: &ProxyRequestInsights::default(),
            usage: &usage,
            latency_ms: latency,
            status_code: status,
            error_message: (status >= 400).then_some("failed"),
            created_at,
            total_cost_usd: cost,
        },
    )
}

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    schema::run_migrations(&conn).unwrap();
    conn
}

fn totals(conn: &Connection) -> (i64, i64, i64, f64, f64) {
    conn.query_row("SELECT total_requests,success_requests,total_input_tokens,CAST(total_cost_usd AS REAL),avg_latency_ms FROM proxy_usage_daily_rollups", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).unwrap()
}

#[test]
fn replacing_attempts_is_idempotent_and_keeps_the_original_day() {
    let conn = database();
    write(
        &conn,
        "request",
        502,
        0,
        20,
        0.0,
        "2026-10-01T23:59:59+00:00",
    )
    .unwrap();
    for _ in 0..2 {
        write(
            &conn,
            "request",
            200,
            7,
            100,
            0.4,
            "2026-10-02T00:00:02+00:00",
        )
        .unwrap();
    }
    assert_eq!(totals(&conn), (1, 1, 7, 0.4, 100.0));
    let caches: (i64,i64,i64) = conn.query_row("SELECT total_output_tokens,total_cache_read_tokens,total_cache_creation_tokens FROM proxy_usage_daily_rollups", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
    assert_eq!(caches, (3, 2, 1));
    let values: (String, String, String) = conn
        .query_row(
            "SELECT created_at,profile_id,provider_name FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        values,
        (
            "2026-10-01T23:59:59+00:00".into(),
            "profile-200".into(),
            "provider-200".into()
        )
    );
}

#[test]
fn corrected_failures_subtract_success_usage_cost_and_latency_contributions() {
    let conn = database();
    write(&conn, "a", 200, 10, 100, 0.01, "2026-10-01T01:00:00Z").unwrap();
    write(&conn, "b", 502, 0, 20, 0.0, "2026-10-01T01:00:01Z").unwrap();
    assert_eq!(totals(&conn), (2, 1, 10, 0.01, 60.0));
    write(&conn, "a", 502, 2, 120, 0.002, "2026-10-01T01:00:02Z").unwrap();
    assert_eq!(totals(&conn), (2, 0, 2, 0.002, 70.0));
}

#[test]
fn later_rollup_failure_rolls_back_the_request_update() {
    let conn = database();
    write(&conn, "a", 502, 0, 20, 0.0, "2026-10-01T01:00:00Z").unwrap();
    conn.execute_batch("CREATE TRIGGER fail_rollup BEFORE UPDATE ON proxy_usage_daily_rollups BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(write(&conn, "a", 200, 7, 100, 0.4, "2026-10-01T01:00:02Z").is_err());
    assert_eq!(totals(&conn), (1, 0, 0, 0.0, 20.0));
    let status: i64 = conn
        .query_row("SELECT status_code FROM proxy_request_logs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(status, 502);
}

#[test]
fn failed_initial_rollup_never_leaves_an_orphaned_request_log() {
    let conn = database();
    conn.execute_batch("CREATE TRIGGER fail_rollup BEFORE INSERT ON proxy_usage_daily_rollups BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(write(&conn, "a", 200, 7, 100, 0.4, "2026-10-01T01:00:02Z").is_err());
    for table in ["proxy_request_logs", "proxy_usage_daily_rollups"] {
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[test]
fn missing_rollup_is_rebuilt_from_all_existing_logs() {
    let conn = database();
    write(&conn, "a", 200, 10, 100, 0.01, "2026-10-01T01:00:00Z").unwrap();
    write(&conn, "b", 200, 5, 20, 0.005, "2026-10-01T01:00:01Z").unwrap();
    conn.execute("DELETE FROM proxy_usage_daily_rollups", [])
        .unwrap();
    write(&conn, "a", 200, 10, 100, 0.01, "2026-10-01T01:00:02Z").unwrap();
    assert_eq!(totals(&conn), (2, 2, 15, 0.015, 60.0));
}

#[test]
fn invalid_new_dates_fail_without_writing_rows() {
    let conn = database();
    assert!(write(&conn, "a", 200, 7, 100, 0.4, "short").is_err());
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM proxy_request_logs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn a_request_id_cannot_overwrite_another_tools_log() {
    let conn = database();
    conn.execute("INSERT INTO proxy_request_logs(request_id,tool_id,profile_id,provider_name,status_code,created_at) VALUES('owned','codex','p','provider',200,'2026-10-01T01:00:00Z')", []).unwrap();
    assert!(write(&conn, "owned", 502, 0, 20, 0.0, "2026-10-01T01:00:02Z").is_err());
    let saved: (String, i64) = conn
        .query_row(
            "SELECT tool_id,status_code FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(saved, ("codex".into(), 200));
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM proxy_usage_daily_rollups",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn cost_deltas_use_the_same_precision_as_stored_request_costs() {
    let conn = database();
    for id in ["a", "b"] {
        write(&conn, id, 200, 0, 20, 0.0000006, "2026-10-01T01:00:00Z").unwrap();
    }
    for _ in 0..3 {
        write(&conn, "a", 200, 0, 20, 0.0000006, "2026-10-01T01:00:01Z").unwrap();
    }
    assert_eq!(totals(&conn), (2, 2, 0, 0.000002, 20.0));
    for invalid in [f64::INFINITY, f64::NAN, -1.0] {
        assert!(write(
            &conn,
            "invalid",
            200,
            0,
            20,
            invalid,
            "2026-10-01T01:00:01Z"
        )
        .is_err());
    }
    assert_eq!(totals(&conn), (2, 2, 0, 0.000002, 20.0));
}
