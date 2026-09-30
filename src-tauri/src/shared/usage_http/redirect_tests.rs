use super::*;
use std::time::Duration;
use tokio::{io::AsyncWriteExt, net::TcpListener};

#[tokio::test]
async fn follows_same_origin_redirects_without_losing_authentication() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/usage", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in ["HTTP/1.1 302 Found\r\nLocation: /quota\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"] {
            let (mut socket, _) = listener.accept().await.unwrap();
            requests.push(test_support::read_headers(&mut socket).await.to_ascii_lowercase());
            socket.write_all(response.as_bytes()).await.unwrap();
        }
        requests
    });
    assert!(request_json(
        &client().unwrap(),
        &url,
        "same-origin-key",
        true,
        Instant::now() + Duration::from_secs(2)
    )
    .await
    .unwrap()
    .is_ok());
    let requests = task.await.unwrap();
    assert!(requests[1].starts_with("get /quota "));
    assert!(requests
        .iter()
        .all(|request| request.contains("authorization: bearer same-origin-key")));
}
