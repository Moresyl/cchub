use super::*;
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn server(payloads: Vec<Value>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for payload in payloads {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut buffer = [0u8; 2048];
                let count = socket.read(&mut buffer).await.unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            requests.push(String::from_utf8(request).unwrap());
            let body = payload.to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
        requests
    });
    (url, task)
}

#[tokio::test]
async fn selected_protocol_drives_endpoint_authentication_and_pagination() {
    let (url, task) = server(vec![
        json!({"models":[{"name":"models/alpha"}],"nextPageToken":"next&one"}),
        json!({"models":[{"name":"models/beta","inputTokenLimit":1000}]}),
    ])
    .await;
    let client = Client::builder().no_proxy().build().unwrap();
    let headers = BTreeMap::from([("x-custom".into(), "fixture".into())]);
    let models = fetch_catalog(
        &client,
        "opencode",
        &url,
        "test-secret",
        false,
        Some("@ai-sdk/google"),
        Some("Fixture UI"),
        Some(&headers),
    )
    .await
    .unwrap();
    assert_eq!(
        models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        vec!["alpha", "beta"]
    );
    let requests = task.await.unwrap();
    assert!(requests[0].starts_with("GET /v1beta/models "));
    assert!(requests[1].starts_with("GET /v1beta/models?pageToken=next%26one "));
    for request in requests {
        assert!(request.contains("x-goog-api-key: test-secret"));
        assert!(request.contains("user-agent: Fixture UI"));
        assert!(request.contains("x-custom: fixture"));
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("key="));
    }
}

#[tokio::test]
async fn anthropic_pages_enrich_duplicate_metadata() {
    let (url, task) = server(vec![
        json!({"data":[{"id":"a","max_input_tokens":200000}],"has_more":true,"last_id":"a"}),
        json!({"data":[{"id":"a","max_tokens":8192},{"id":"b"}],"has_more":false}),
    ])
    .await;
    let client = Client::builder().no_proxy().build().unwrap();
    let models = fetch_catalog(
        &client,
        "openclaw",
        &url,
        "secret",
        false,
        Some("anthropic-messages"),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].context_window, Some(200000));
    assert_eq!(models[0].max_output_tokens, Some(8192));
    let requests = task.await.unwrap();
    assert!(requests[1].starts_with("GET /v1/models?after_id=a "));
    assert!(requests[0].contains("x-api-key: secret"));
    assert!(requests[0].contains("anthropic-version:"));
}

#[tokio::test]
async fn repeated_page_cursor_is_an_error_not_a_partial_catalog() {
    let payload = json!({"models":[{"name":"models/a"}],"nextPageToken":"same"});
    let (url, task) = server(vec![payload.clone(), payload]).await;
    let result = fetch_catalog(
        &Client::builder().no_proxy().build().unwrap(),
        "gemini",
        &url,
        "secret",
        false,
        None,
        None,
        None,
    )
    .await;
    assert!(result.unwrap_err().contains("repeated"));
    assert_eq!(task.await.unwrap().len(), 2);
}

#[test]
fn protocol_defaults_and_invalid_cursor_are_checked() {
    assert_eq!(
        Protocol::resolve("claude", None).unwrap(),
        Protocol::Anthropic
    );
    assert_eq!(
        Protocol::resolve("claude", Some("openai_responses")).unwrap(),
        Protocol::OpenAi
    );
    assert!(Protocol::resolve("opencode", Some("@ai-sdk/amazon-bedrock")).is_err());
    assert!(next_cursor(&json!({"has_more":true}), Protocol::Anthropic).is_err());
    assert!(next_cursor(&json!({"nextPageToken":{}}), Protocol::Gemini).is_err());
    assert_eq!(
        next_cursor(&json!({"nextPageToken":""}), Protocol::Gemini).unwrap(),
        None
    );
    assert!(Protocol::OpenAi
        .url("https://user:secret@example.test", false)
        .is_err());
}

#[tokio::test]
async fn one_deadline_bounds_a_stalled_response_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 4096];
        socket.read(&mut request).await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{")
            .await
            .unwrap();
        std::future::pending::<()>().await;
    });
    let client = Client::builder().no_proxy().build().unwrap();
    let result = fetch_catalog_until(
        &client,
        "codex",
        &url,
        "secret",
        false,
        None,
        None,
        None,
        Instant::now() + std::time::Duration::from_millis(200),
    )
    .await;
    task.abort();
    assert_eq!(result.unwrap_err(), "Model discovery timed out");
}

#[tokio::test]
async fn oversized_catalogs_fail_instead_of_returning_partial_results() {
    let (url, task) = server(vec![json!({"data":(0..10001).map(|id| json!({"id":format!("model-{id}")})).collect::<Vec<_>>()})]).await;
    let result = fetch_catalog(
        &Client::builder().no_proxy().build().unwrap(),
        "codex",
        &url,
        "secret",
        false,
        None,
        None,
        None,
    )
    .await;
    assert!(result.unwrap_err().contains("model count limit"));
    task.await.unwrap();
}
