use super::*;
use serde_json::Value;

const REASONING: &str = concat!(
    "event: response.created\ndata: {\"response\":{\"id\":\"resp_1\",\"model\":\"fixture-model\"}}\n\n",
    "event: response.reasoning_summary_part.added\ndata: {\"item_id\":\"rs_1\",\"summary_index\":0,\"part\":{\"type\":\"summary_text\",\"text\":\"\"}}\n\n",
    "data: {\"type\":\"response.reasoning_summary_text.delta\",\"item_id\":\"rs_1\",\"summary_index\":0,\"delta\":\"思考🦀\"}\n\n",
    "event: response.reasoning_summary_text.done\ndata: {\"item_id\":\"rs_1\",\"summary_index\":0,\"text\":\"思考🦀\"}\n\n",
    "event: response.reasoning_summary_part.done\ndata: {\"item_id\":\"rs_1\",\"summary_index\":0,\"part\":{\"type\":\"summary_text\",\"text\":\"思考🦀\"}}\n\n",
    "event: response.output_text.delta\ndata: {\"item_id\":\"msg_1\",\"content_index\":0,\"delta\":\"答案\"}\n\n",
    "event: response.completed\ndata: {\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}}\n\n"
);

#[tokio::test]
async fn standard_reasoning_reaches_the_proxy_client_once_with_original_usage() {
    let upstream = server(StatusCode::OK, "text/event-stream", REASONING).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "openai_responses");
    let response = forward(app.handle().clone(), true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 32 * 1024).await.unwrap();
    let events = String::from_utf8(body.to_vec())
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(events[0]["type"], "message_start");
    assert_eq!(events.last().unwrap()["type"], "message_stop");
    assert_eq!(
        events
            .iter()
            .filter_map(|event| event["delta"]["thinking"].as_str())
            .collect::<String>(),
        "思考🦀"
    );
    assert_eq!(
        events
            .iter()
            .filter_map(|event| event["delta"]["text"].as_str())
            .collect::<String>(),
        "答案"
    );
    let text = events
        .iter()
        .position(|event| event["content_block"]["type"] == "text")
        .unwrap();
    assert_eq!(events[text - 1]["type"], "content_block_stop");
    assert_ne!(events[text]["index"], events[text - 1]["index"]);
    streaming_tests::assert_single_outcome(&app, 200, 1, 3);
    let db = app.state::<DbState>();
    let output: i64 =
        db.0.lock()
            .unwrap()
            .query_row("SELECT output_tokens FROM proxy_request_logs", [], |row| {
                row.get(0)
            })
            .unwrap();
    assert_eq!(output, 4);
}

#[tokio::test]
async fn local_adapter_failure_is_recorded_as_failure_even_if_vendor_completed() {
    let mut wire = String::new();
    for index in 0..=4096 {
        let data = json!({"item_id":"rs","summary_index":index,"text":"done"});
        wire += &format!("event: response.reasoning_summary_text.done\ndata: {data}\n\n");
    }
    wire += "event: response.completed\ndata: {\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}}\n\n";
    let upstream = dynamic_server(wire).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "openai_responses");
    let response = forward(app.handle().clone(), true).await;
    let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    let wire = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(wire.matches("event: error\n").count(), 1);
    assert!(!wire.contains("message_stop"));
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let status: u16 = conn
        .query_row("SELECT status_code FROM proxy_request_logs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(status, 502);
    let counts: (i64, i64) = conn
        .query_row(
            "SELECT total_requests,success_requests FROM proxy_usage_daily_rollups",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (1, 0));
}

async fn dynamic_server(wire: String) -> Upstream {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let wire = Arc::new(wire);
    let router = Router::new().fallback(any(move || {
        let wire = wire.clone();
        let observed = observed.clone();
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            Response::builder()
                .header("content-type", "text/event-stream")
                .body(Body::from(wire.as_str().to_owned()))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Upstream { url, hits, task }
}
