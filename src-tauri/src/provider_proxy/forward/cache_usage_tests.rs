use super::*;
use futures_util::StreamExt;

const ANTHROPIC: &str = r#"{"type":"message","model":"fixture-model","content":[{"type":"text","text":"hello"}],"usage":{"input_tokens":100,"output_tokens":5,"cache_read_input_tokens":800,"cache_creation_input_tokens":100}}"#;
const CHAT: &str = r#"{"id":"chat-fixture","model":"fixture-model","choices":[{"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1000,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":800,"cache_write_tokens":100}}}"#;
const RESPONSES: &str = r#"{"id":"resp-fixture","object":"response","model":"fixture-model","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hello"}]}],"usage":{"input_tokens":1000,"output_tokens":5,"input_tokens_details":{"cached_tokens":800,"cache_write_tokens":100}}}"#;

const ANTHROPIC_STREAM: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":100}}}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"cache_read_input_tokens\":800}}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"cache_creation_input_tokens\":100}}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":5}}\n\n"
);
const CHAT_STREAM: &str = concat!(
    "data: {\"id\":\"chat-fixture\",\"model\":\"fixture-model\",\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":\"stop\"}]}\n\n",
    "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":1000,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":800,\"cache_write_tokens\":100}}}\n\n"
);
const RESPONSES_STREAM: &str = concat!(
    "event: response.created\ndata: {\"response\":{\"id\":\"resp-fixture\",\"model\":\"fixture-model\",\"usage\":{\"input_tokens\":1000}}}\n\n",
    "event: response.in_progress\ndata: {\"response\":{\"usage\":{\"input_tokens_details\":{\"cached_tokens\":800}}}}\n\n",
    "event: response.in_progress\ndata: {\"response\":{\"usage\":{\"input_tokens_details\":{\"cache_write_tokens\":100}}}}\n\n",
    "event: response.completed\ndata: {\"response\":{\"id\":\"resp-fixture\",\"status\":\"completed\",\"usage\":{\"output_tokens\":5}}}\n\n"
);

fn priced_app(url: &str, format: &str) -> App<MockRuntime> {
    let app = app(&[("p1", url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", format);
    app.state::<DbState>().0.lock().unwrap().execute(
        "INSERT OR REPLACE INTO model_pricing(model_id,normalized_model_id,input_cost_per_million,output_cost_per_million,cache_read_cost_per_million,cache_write_cost_per_million,created_at,updated_at) VALUES('fixture-model','fixture-model','2','3','0.5','4','2026-10-02','2026-10-02')", [],
    ).unwrap();
    app
}

async fn request(app: &App<MockRuntime>, path: &str, stream: bool) -> Response<Body> {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/proxy/claude/{path}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"model":"fixture-model","stream":stream,"messages":[],"input":[]}).to_string(),
        ))
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        forward_proxy_request_with_client(
            app.handle().clone(),
            "claude".into(),
            path.into(),
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        ),
    )
    .await
    .unwrap()
}

fn assert_accounting(app: &App<MockRuntime>, status: u16) {
    streaming_tests::assert_single_outcome(app, status, i64::from(status == 200), 1000);
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let row: (i64, i64, i64, f64) = conn.query_row(
        "SELECT output_tokens,cache_read_tokens,cache_creation_tokens,CAST(total_cost_usd AS REAL) FROM proxy_request_logs", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).unwrap();
    assert_eq!(row, (5, 800, 100, 0.001015));
    let daily: (i64, i64, i64, f64) = conn.query_row(
        "SELECT total_output_tokens,total_cache_read_tokens,total_cache_creation_tokens,CAST(total_cost_usd AS REAL) FROM proxy_usage_daily_rollups", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).unwrap();
    assert_eq!(daily, row);
    let marker: bool = conn
        .query_row(
            "SELECT input_tokens_is_total FROM proxy_request_logs",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(marker);
    drop(conn);
    let analytics = crate::commands::usage_analytics::get_usage_analytics(
        Some(1),
        None,
        None,
        None,
        app.state::<DbState>(),
    )
    .unwrap();
    assert_eq!(analytics.summary.total_tokens, 1005);
    assert_eq!(analytics.providers[0].total_tokens, 1005);
    assert_eq!(analytics.models[0].total_tokens, 1005);
}

fn assert_anthropic_usage(usage: &serde_json::Value) {
    assert_eq!(usage["input_tokens"], 100);
    assert_eq!(usage["output_tokens"], 5);
    assert_eq!(usage["cache_read_input_tokens"], 800);
    assert_eq!(usage["cache_creation_input_tokens"], 100);
}

#[test]
fn cache_usage_dashboard_mixes_marked_totals_with_legacy_rows_without_rewriting_them() {
    let app = priced_app("http://127.0.0.1:1", "anthropic");
    let timestamp = chrono::Utc::now().to_rfc3339();
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().unwrap();
        for (id, input, marker) in [("legacy", 100, 0), ("new", 1000, 1)] {
            conn.execute("INSERT INTO proxy_request_logs(request_id,tool_id,profile_id,provider_name,response_model,input_tokens,output_tokens,cache_read_tokens,cache_creation_tokens,input_tokens_is_total,total_cost_usd,status_code,created_at) VALUES(?1,'claude','p1','fixture','fixture-model',?2,5,800,100,?3,'0.001015',200,?4)",rusqlite::params![id,input,marker,timestamp]).unwrap();
        }
    }
    for _ in 0..2 {
        let analytics = crate::commands::usage_analytics::get_usage_analytics(
            Some(1),
            None,
            None,
            None,
            app.state::<DbState>(),
        )
        .unwrap();
        assert_eq!(analytics.summary.input_tokens, 1100);
        assert_eq!(analytics.summary.total_tokens, 2010);
        assert_eq!(analytics.summary.total_cost_usd, "0.002030");
        assert_eq!(analytics.providers[0].total_tokens, 2010);
        assert_eq!(analytics.models[0].total_tokens, 2010);
    }
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let legacy: (i64,i64,String) = conn.query_row("SELECT input_tokens,input_tokens_is_total,total_cost_usd FROM proxy_request_logs WHERE request_id='legacy'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(legacy, (100, 0, "0.001015".into()));
}

#[tokio::test]
async fn whole_cache_usage_prices_each_input_category_once_in_native_and_translated_protocols() {
    for (format, path, body, translated) in [
        ("anthropic", "v1/messages", ANTHROPIC, false),
        ("openai_chat", "v1/messages", CHAT, true),
        ("openai_responses", "v1/messages", RESPONSES, true),
        ("anthropic", "v1/chat/completions", CHAT, false),
        ("anthropic", "v1/responses", RESPONSES, false),
    ] {
        let upstream = server(StatusCode::OK, "application/json", body).await;
        let app = priced_app(&upstream.url, format);
        let response = request(&app, path, false).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 16384).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        if translated {
            assert_anthropic_usage(&value["usage"]);
        } else {
            assert_eq!(
                value,
                serde_json::from_str::<serde_json::Value>(body).unwrap()
            );
        }
        assert_accounting(&app, 200);
    }
}

#[tokio::test]
async fn streamed_cache_usage_preserves_metadata_only_tail_and_native_wire_counters() {
    for (format, path, partial, terminal, translated) in [
        (
            "anthropic",
            "v1/messages",
            ANTHROPIC_STREAM,
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            false,
        ),
        (
            "openai_chat",
            "v1/messages",
            CHAT_STREAM,
            "data: [DONE]\n\n",
            true,
        ),
        (
            "openai_responses",
            "v1/messages",
            RESPONSES_STREAM,
            "",
            true,
        ),
        (
            "anthropic",
            "v1/chat/completions",
            CHAT_STREAM,
            "data: [DONE]\n\n",
            false,
        ),
        ("anthropic", "v1/responses", RESPONSES_STREAM, "", false),
    ] {
        let wire = format!("{partial}{terminal}");
        let body: &'static str = Box::leak(wire.into_boxed_str());
        let upstream = streaming_tests::split_server(body).await;
        let app = priced_app(&upstream.url, format);
        let response = request(&app, path, true).await;
        let bytes = tokio::time::timeout(
            Duration::from_secs(5),
            to_bytes(response.into_body(), 16384),
        )
        .await
        .unwrap()
        .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        if translated {
            let final_usage = text
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .filter(|event| event["type"] == "message_delta")
                .filter_map(|event| {
                    event
                        .get("usage")
                        .filter(|usage| usage.is_object())
                        .cloned()
                })
                .last()
                .unwrap();
            assert_anthropic_usage(&final_usage);
        } else {
            assert_eq!(text, body);
        }
        assert_accounting(&app, 200);
    }
}

#[tokio::test]
async fn interrupted_anthropic_cache_usage_keeps_partial_cost_and_does_not_count_success() {
    let upstream = server(StatusCode::OK, "text/event-stream", ANTHROPIC_STREAM).await;
    let app = priced_app(&upstream.url, "anthropic");
    let response = request(&app, "v1/messages", true).await;
    let bytes = to_bytes(response.into_body(), 16384).await.unwrap();
    assert!(String::from_utf8(bytes.to_vec())
        .unwrap()
        .contains("event: error"));
    assert_accounting(&app, 502);
}

#[tokio::test]
async fn cancellation_after_cache_readings_keeps_partial_cost_without_counting_success() {
    let upstream = streaming_tests::split_server_with_pending(ANTHROPIC_STREAM, true).await;
    let app = priced_app(&upstream.url, "anthropic");
    let mut body = request(&app, "v1/messages", true)
        .await
        .into_body()
        .into_data_stream();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while bytes.len() < ANTHROPIC_STREAM.len() {
            bytes.extend_from_slice(&body.next().await.unwrap().unwrap());
        }
    })
    .await
    .unwrap();
    drop(body);
    assert_accounting(&app, 499);
}
