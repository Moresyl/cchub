use super::*;
use axum::{body::Body, http::Response, routing::get, Router};

async fn decode(body: String, chunked: bool) -> Result<CopilotToken, CopilotAuthError> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().route(
        "/",
        get(move || {
            let body = body.clone();
            async move {
                let body = if chunked {
                    Body::from_stream(futures_util::stream::iter([Ok::<_, std::io::Error>(
                        bytes::Bytes::from(body),
                    )]))
                } else {
                    Body::from(body)
                };
                Response::builder()
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap()
            }
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let response = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap()
        .get(url)
        .send()
        .await
        .unwrap();
    let result = decode_token(response).await;
    server.abort();
    result
}

#[tokio::test]
async fn actual_token_decoder_validates_tokens_and_bounds_known_and_chunked_responses() {
    let valid = serde_json::json!({"token":"fixture-access", "expires_at":chrono::Utc::now().timestamp() + 3600}).to_string();
    assert_eq!(decode(valid, false).await.unwrap().token, "fixture-access");
    for (body, chunked) in [
        (r#"{"token":"private-access"}"#.into(), false),
        (r#"{"token":"private-access","expires_at":1}"#.into(), false),
        (
            serde_json::json!({"token":" ", "expires_at":chrono::Utc::now().timestamp() + 3600})
                .to_string(),
            false,
        ),
        ("invalid private-access".into(), false),
        ("private-access".repeat(200_000), false),
        ("private-access".repeat(200_000), true),
    ] {
        let error = decode(body, chunked)
            .await
            .err()
            .expect("invalid token must fail");
        assert!(!error.to_string().contains("private-access"));
    }
}

#[tokio::test]
async fn blank_cached_tokens_are_refreshed_instead_of_sent() {
    let (_dir, manager) = crate::copilot_auth::test_support::seeded().await;
    manager
        .copilot_tokens
        .write()
        .await
        .get_mut("1")
        .unwrap()
        .token = " ".into();
    let lease = manager
        .lease_using(Some("1"), |_| async {
            Ok(CopilotToken {
                token: "fresh".into(),
                expires_at: chrono::Utc::now().timestamp() + 3600,
            })
        })
        .await
        .unwrap();
    assert_eq!(lease.token, "fresh");
}
