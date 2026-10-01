use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn response(status: u16, body: &str) -> reqwest::Response {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let reply = format!(
        "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        socket.read(&mut request).await.unwrap();
        socket.write_all(reply.as_bytes()).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap()
        .get(format!("http://{address}/token"))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn credential_rejections_are_distinct_from_proxy_and_service_failures() {
    for (status, body) in [
        (401, "<html>Unauthorized</html>"),
        (
            400,
            r#"{"error":"invalid_grant","error_description":"private secret"}"#,
        ),
        (400, r#"{"error":{"code":"refresh_token_reused"}}"#),
        (403, r#"{"error":"invalid_token"}"#),
    ] {
        assert_eq!(
            read_refresh_response(response(status, body).await).await,
            Err(RefreshResponseError::ReauthRequired)
        );
    }
    for (status, body) in [
        (403, "<html>Proxy challenge</html>"),
        (400, r#"{"error":"invalid_request"}"#),
        (429, r#"{"error":"invalid_grant"}"#),
        (503, r#"{"error":"invalid_grant"}"#),
    ] {
        assert_eq!(
            read_refresh_response(response(status, body).await).await,
            Err(RefreshResponseError::Rejected(status))
        );
    }
}

#[tokio::test]
async fn successful_and_invalid_payloads_are_bounded() {
    assert_eq!(
        read_refresh_response(response(200, r#"{"access_token":"opaque"}"#).await)
            .await
            .unwrap()["access_token"],
        "opaque"
    );
    assert_eq!(
        read_refresh_response(response(200, "private secret").await).await,
        Err(RefreshResponseError::InvalidPayload)
    );
    assert_eq!(
        read_refresh_response(response(200, &"x".repeat(65537)).await).await,
        Err(RefreshResponseError::InvalidPayload)
    );
}
