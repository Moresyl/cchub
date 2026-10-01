use super::*;
use crate::shared::usage_http::test_support::read_headers;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Notify};

#[derive(Debug, PartialEq)]
enum TestError {
    Resource(ResourceError),
    Changed,
    Expired,
}
impl From<ResourceError> for TestError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

#[derive(Default)]
struct Provider {
    changed: AtomicBool,
    refreshes: AtomicUsize,
    expirations: AtomicUsize,
    validations: AtomicUsize,
    checked: Notify,
}

impl TokenProvider for Provider {
    type Error = TestError;
    async fn lease(&self, _: Option<&str>) -> Result<TokenLease, TestError> {
        Ok(TokenLease {
            account_id: "account-one".into(),
            revision: "login-one".into(),
            token: "old".into(),
        })
    }
    async fn validate(&self, _: &TokenLease) -> Result<(), TestError> {
        self.validations.fetch_add(1, Ordering::SeqCst);
        self.checked.notify_one();
        if self.changed.load(Ordering::SeqCst) {
            Err(TestError::Changed)
        } else {
            Ok(())
        }
    }
    async fn recover(&self, lease: &TokenLease) -> Result<TokenLease, TestError> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        self.validate(lease).await?;
        Ok(TokenLease {
            account_id: lease.account_id.clone(),
            revision: lease.revision.clone(),
            token: "new".into(),
        })
    }
    async fn expire(&self, lease: &TokenLease) -> Result<(), TestError> {
        self.validate(lease).await?;
        self.expirations.fetch_add(1, Ordering::SeqCst);
        Err(TestError::Expired)
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
}

async fn fixture(replies: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = seen.clone();
    tokio::spawn(async move {
        for (status, body) in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            let headers = read_headers(&mut socket).await;
            recorded.lock().await.push(headers);
            let reply = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            // Oversized-body tests deliberately stop reading before all bytes arrive.
            let _ = socket.write_all(reply.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    (format!("http://{address}/resource"), seen)
}

#[tokio::test]
async fn resource_401_recovers_once_with_the_same_account_and_new_bearer() {
    let (url, seen) = fixture(vec![
        (401, "private error".into()),
        (200, r#"{"value":7}"#.into()),
    ])
    .await;
    let provider = Provider::default();
    let client = client();
    let value = get_json(&provider, None, |lease| {
        client
            .get(&url)
            .bearer_auth(&lease.token)
            .header("ChatGPT-Account-Id", &lease.account_id)
    })
    .await
    .unwrap();
    assert_eq!(value["value"], 7);
    assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(provider.expirations.load(Ordering::SeqCst), 0);
    let headers = seen.lock().await;
    assert_eq!(headers.len(), 2);
    assert!(headers[0].contains("Bearer old"));
    assert!(headers[1].contains("Bearer new"));
    assert!(headers.iter().all(|header| header.contains("account-one")));
}

#[tokio::test]
async fn second_401_expires_the_owned_session_without_a_third_attempt() {
    let (url, seen) = fixture(vec![(401, "".into()), (401, "".into())]).await;
    let provider = Provider::default();
    let client = client();
    assert_eq!(
        get_json(&provider, None, |_| client.get(&url)).await,
        Err(TestError::Expired)
    );
    assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(provider.expirations.load(Ordering::SeqCst), 1);
    assert_eq!(seen.lock().await.len(), 2);
}

#[tokio::test]
async fn forbidden_rate_limit_and_service_failures_preserve_authorization() {
    for status in [403, 429, 503] {
        let (url, seen) = fixture(vec![(status, "secret vendor error".into())]).await;
        let provider = Provider::default();
        let client = client();
        assert_eq!(
            get_json(&provider, None, |_| client.get(&url)).await,
            Err(TestError::Resource(ResourceError::Http(status)))
        );
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
        assert_eq!(provider.expirations.load(Ordering::SeqCst), 0);
        assert_eq!(seen.lock().await.len(), 1);
    }
}

#[tokio::test]
async fn invalid_and_oversized_payloads_do_not_expire_accounts() {
    for (body, error) in [
        ("not JSON".into(), ResourceError::InvalidPayload),
        (" ".repeat(2 * 1024 * 1024 + 1), ResourceError::TooLarge),
    ] {
        let (url, _) = fixture(vec![(200, body)]).await;
        let provider = Provider::default();
        let client = client();
        assert_eq!(
            get_json(&provider, None, |_| client.get(&url)).await,
            Err(TestError::Resource(error))
        );
        assert_eq!(provider.expirations.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn replacement_during_body_read_discards_both_success_and_parse_failure() {
    for body in [r#"{"value":7}"#, "not JSON"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/resource", listener.local_addr().unwrap());
        let (finish, finished) = tokio::sync::oneshot::channel();
        let body = body.to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_headers(&mut socket).await;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            finished.await.unwrap();
            socket.write_all(body.as_bytes()).await.unwrap();
        });
        let provider = Arc::new(Provider::default());
        let worker = provider.clone();
        let client = client();
        let task =
            tokio::spawn(async move { get_json(&*worker, None, |_| client.get(&url)).await });
        provider.checked.notified().await;
        provider.changed.store(true, Ordering::SeqCst);
        finish.send(()).unwrap();
        assert_eq!(task.await.unwrap(), Err(TestError::Changed));
        assert_eq!(provider.expirations.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn deadline_also_covers_stalled_bodies_without_expiring_authorization() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/resource", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_headers(&mut socket).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n")
            .await
            .unwrap();
        std::future::pending::<()>().await;
    });
    let provider = Provider::default();
    let client = client();
    assert_eq!(
        get_json_before(
            &provider,
            None,
            |_| client.get(&url),
            tokio::time::Instant::now() + Duration::from_secs(1)
        )
        .await,
        Err(TestError::Resource(ResourceError::Timeout))
    );
    assert_eq!(provider.expirations.load(Ordering::SeqCst), 0);
    assert!(provider.validations.load(Ordering::SeqCst) >= 1);
    server.abort();
}

#[tokio::test]
async fn chunked_size_limit_and_truncated_bodies_preserve_sign_in() {
    let big = " ".repeat(2 * 1024 * 1024 + 1);
    let chunked = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:X}\r\n{big}\r\n0\r\n\r\n", big.len());
    for (reply, error) in [
        (chunked, ResourceError::TooLarge),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{}".into(),
            ResourceError::Transport,
        ),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/resource", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_headers(&mut socket).await;
            let _ = socket.write_all(reply.as_bytes()).await;
            let _ = socket.shutdown().await;
        });
        let provider = Provider::default();
        let client = client();
        assert_eq!(
            get_json(&provider, None, |_| client.get(&url)).await,
            Err(TestError::Resource(error))
        );
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
        assert_eq!(provider.expirations.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn query_client_follows_only_same_origin_redirects() {
    let destination = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let external = format!("http://{}/secret", destination.local_addr().unwrap());
    let destination_task = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_millis(300), destination.accept())
            .await
            .is_ok()
    });
    for redirect_to in [None, Some(external)] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/resource", listener.local_addr().unwrap());
        let same_origin = redirect_to.is_none();
        let location = redirect_to.unwrap_or_else(|| format!("{url}?next=1"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let initial = read_headers(&mut socket).await;
            assert!(initial.contains("Bearer old"));
            assert!(initial.contains("account-one"));
            socket.write_all(format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
            if same_origin {
                let (mut next, _) = listener.accept().await.unwrap();
                let headers = read_headers(&mut next).await;
                assert!(headers.contains("Bearer old"));
                assert!(headers.contains("account-one"));
                next.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                )
                .await
                .unwrap();
            }
        });
        let provider = Provider::default();
        let client = super::client(None, "CCHub OAuth test", Duration::from_secs(3)).unwrap();
        let result = get_json(&provider, None, |lease| {
            client
                .get(&url)
                .bearer_auth(&lease.token)
                .header("ChatGPT-Account-Id", &lease.account_id)
        })
        .await;
        if same_origin {
            assert_eq!(result.unwrap(), serde_json::json!({}));
        } else {
            assert_eq!(result, Err(TestError::Resource(ResourceError::Http(302))));
        }
        server.await.unwrap();
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
        assert_eq!(provider.expirations.load(Ordering::SeqCst), 0);
    }
    assert!(
        !destination_task.await.unwrap(),
        "credentials reached another origin"
    );
}

#[test]
fn invalid_query_proxy_errors_do_not_echo_the_proxy_secret() {
    let error = super::client(
        Some("http://user:private@ invalid"),
        "CCHub",
        Duration::from_secs(3),
    )
    .unwrap_err();
    assert_eq!(error, "Invalid OAuth query proxy");
}
