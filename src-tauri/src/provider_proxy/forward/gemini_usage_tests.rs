use super::*;
use futures_util::StreamExt;

const PARTIAL: &str = concat!(
    "data: {\"modelVersion\":\"fixture-gemini\",\"usageMetadata\":{\"promptTokenCount\":7}}\n\n",
    "data: {\"usageMetadata\":{\"candidatesTokenCount\":5}}\n\n",
    "data: {\"usageMetadata\":{\"thoughtsTokenCount\":3}}\n\n",
    "data: {\"usageMetadata\":{\"cachedContentTokenCount\":2}}\n\n",
    "data: {\"usageMetadata\":{\"candidatesTokenCount\":2,\"thoughtsTokenCount\":1}}\n\n",
    "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hello\"}]}}]}\n\n"
);
const COMPLETE: &str = concat!(
    "data: {\"modelVersion\":\"fixture-gemini\",\"usageMetadata\":{\"promptTokenCount\":7}}\n\n",
    "data: {\"usageMetadata\":{\"candidatesTokenCount\":5}}\n\n",
    "data: {\"usageMetadata\":{\"thoughtsTokenCount\":3}}\n\n",
    "data: {\"usageMetadata\":{\"cachedContentTokenCount\":2}}\n\n",
    "data: {\"usageMetadata\":{\"candidatesTokenCount\":2,\"thoughtsTokenCount\":1}}\n\n",
    "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hello\"}]},\"finishReason\":\"STOP\"}]}\n\n"
);

async fn native_forward(app: &App<MockRuntime>) -> Response<Body> {
    let path = "v1beta/models/fixture:streamGenerateContent";
    let request = Request::builder()
        .method("POST")
        .uri(format!("/proxy/claude/{path}"))
        .header("content-type", "application/json")
        .body(Body::from(json!({"stream":true,"contents":[]}).to_string()))
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

fn assert_accounting(app: &App<MockRuntime>, status: u16, success: i64) {
    streaming_tests::assert_single_outcome(app, status, success, 7);
    let db = app.state::<DbState>();
    let conn = db.0.lock().unwrap();
    let actual: (i64, i64, String) = conn
        .query_row(
            "SELECT output_tokens,cache_read_tokens,response_model FROM proxy_request_logs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(actual, (8, 2, "fixture-gemini".into()));
    let daily: (i64, i64) = conn
        .query_row(
            "SELECT total_output_tokens,total_cache_read_tokens FROM proxy_usage_daily_rollups",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(daily, (8, 2));
}

#[tokio::test]
async fn independent_gemini_readings_survive_native_and_translated_success_or_interruption() {
    for translated in [false, true] {
        for completed in [false, true] {
            let upstream = server(
                StatusCode::OK,
                "text/event-stream",
                if completed { COMPLETE } else { PARTIAL },
            )
            .await;
            let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
            let response = if translated {
                set_format(&app, "p1", "gemini_native");
                forward(app.handle().clone(), true).await
            } else {
                native_forward(&app).await
            };
            let bytes = tokio::time::timeout(
                Duration::from_secs(5),
                to_bytes(response.into_body(), 65536),
            )
            .await
            .unwrap()
            .unwrap();
            let output = std::str::from_utf8(&bytes).unwrap();
            if translated && completed {
                assert!(output.contains("\"output_tokens\":8"));
                assert!(output.contains("\"cache_read_input_tokens\":2"));
                assert_eq!(output.matches("event: message_stop").count(), 1);
            }
            if !completed {
                assert!(output.contains("error"));
                assert!(!output.contains("event: message_stop"));
            }
            assert_accounting(
                &app,
                if completed { 200 } else { 502 },
                i64::from(completed),
            );
        }
    }
}

#[tokio::test]
async fn translated_cancel_keeps_native_readings_before_a_final_usage_event_exists() {
    let upstream = server(StatusCode::OK, "text/event-stream", PARTIAL).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    set_format(&app, "p1", "gemini_native");
    let response = forward(app.handle().clone(), true).await;
    let mut body = response.into_body().into_data_stream();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let chunk = body.next().await.unwrap().unwrap();
            if std::str::from_utf8(&chunk)
                .unwrap()
                .contains("\"text\":\"hello\"")
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    drop(body);
    assert_accounting(&app, 499, 0);
}
