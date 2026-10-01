use super::*;
use crate::codex_oauth::test_support::seeded;
use crate::shared::usage_http::test_support::read_headers;
use tokio::{io::AsyncWriteExt, sync::oneshot};

fn exhausted() -> String {
    serde_json::json!({"rate_limit":{"allowed":false,"limit_reached":true,
        "primary_window":{"used_percent":100,"reset_at":chrono::Utc::now().timestamp()+3600}}})
    .to_string()
}

async fn fixture(
    status: u16,
    body: String,
) -> (
    String,
    oneshot::Receiver<String>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/usage", listener.local_addr().unwrap());
    let (seen, observed) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = seen.send(read_headers(&mut socket).await);
        released.await.unwrap();
        let reply = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(reply.as_bytes()).await;
    });
    (url, observed, release, task)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap()
}

#[tokio::test]
async fn quota_queries_record_the_actual_owner_and_defaults_never_reassign_results() {
    let (_dir, manager) = seeded().await;
    manager.set_default_account("one").await.unwrap();
    let revision = manager.accounts.read().await["one"].revision.clone();
    let second = manager.accounts.read().await["two"].revision.clone();
    let (url, seen, release, server) = fixture(200, exhausted()).await;
    let worker = manager.clone();
    let query = tokio::spawn(async move {
        worker
            .quota_json(None, |id, token| {
                client()
                    .get(&url)
                    .bearer_auth(token)
                    .header("chatgpt-account-id", id)
            })
            .await
    });
    let headers = seen.await.unwrap();
    assert!(headers.contains("Bearer one-access"));
    manager.set_default_account("two").await.unwrap();
    release.send(()).unwrap();
    query.await.unwrap().unwrap();
    server.await.unwrap();
    assert!(manager.quota_blocked("one", &revision, None).is_some());
    assert_eq!(manager.quota_blocked("two", &second, None), None);
    assert_eq!(manager.quota_blocked("one", "old-login", None), None);
}

#[tokio::test]
async fn newest_resource_query_owns_quota_even_when_responses_finish_out_of_order() {
    let (_dir, manager) = seeded().await;
    let revision = manager.accounts.read().await["one"].revision.clone();
    let (old_url, seen, old_release, old_server) = fixture(200, exhausted()).await;
    let worker = manager.clone();
    let old = tokio::spawn(async move {
        worker
            .quota_json(Some("one"), |_, _| client().get(&old_url))
            .await
    });
    seen.await.unwrap();
    let (new_url, _, release, server) =
        fixture(200, "{\"rate_limit\":{\"allowed\":true}}".into()).await;
    release.send(()).unwrap();
    manager
        .quota_json(Some("one"), |_, _| client().get(&new_url))
        .await
        .unwrap();
    old_release.send(()).unwrap();
    old.await.unwrap().unwrap();
    server.await.unwrap();
    old_server.await.unwrap();
    assert_eq!(manager.quota_blocked("one", &revision, None), None);
}

#[tokio::test]
async fn failed_queries_clear_a_confirmed_block_and_model_queries_do_not_change_quota() {
    let (_dir, manager) = seeded().await;
    let revision = manager.accounts.read().await["one"].revision.clone();
    let (url, _, release, server) = fixture(200, exhausted()).await;
    release.send(()).unwrap();
    manager
        .quota_json(Some("one"), |_, _| client().get(&url))
        .await
        .unwrap();
    server.await.unwrap();
    let (url, _, release, server) = fixture(200, "{\"data\":[]}".into()).await;
    release.send(()).unwrap();
    manager
        .resource_json(Some("one"), |_, _| client().get(&url))
        .await
        .unwrap();
    server.await.unwrap();
    assert!(manager.quota_blocked("one", &revision, None).is_some());
    for (status, body) in [(503, "private error"), (200, "invalid JSON")] {
        let (url, _, release, server) = fixture(status, body.into()).await;
        release.send(()).unwrap();
        assert!(manager
            .quota_json(Some("one"), |_, _| client().get(&url))
            .await
            .is_err());
        server.await.unwrap();
        assert_eq!(manager.quota_blocked("one", &revision, None), None);
    }
}

#[tokio::test]
async fn replaced_removed_or_reauth_accounts_reject_late_quota() {
    for change in ["replace", "remove", "reauth"] {
        let (_dir, manager) = seeded().await;
        let revision = manager.accounts.read().await["one"].revision.clone();
        let (url, seen, release, server) = fixture(200, exhausted()).await;
        let worker = manager.clone();
        let query = tokio::spawn(async move {
            worker
                .quota_json(Some("one"), |_, _| client().get(&url))
                .await
        });
        seen.await.unwrap();
        match change {
            "replace" => {
                manager
                    .accounts
                    .write()
                    .await
                    .get_mut("one")
                    .unwrap()
                    .revision = new_revision()
            }
            "remove" => {
                manager.accounts.write().await.remove("one");
            }
            _ => {
                manager
                    .accounts
                    .write()
                    .await
                    .get_mut("one")
                    .unwrap()
                    .requires_reauth = true
            }
        }
        release.send(()).unwrap();
        assert!(query.await.unwrap().is_err());
        server.await.unwrap();
        assert_eq!(manager.quota_blocked("one", &revision, None), None);
    }
}

#[tokio::test]
async fn cancellation_keeps_quota_unknown_and_recovery_keeps_the_pinned_identity() {
    let (_dir, manager) = seeded().await;
    let lease = manager.lease(Some("one")).await.unwrap();
    let owner = Owner {
        manager: &manager,
        lease: lease.clone(),
    };
    manager.set_default_account("two").await.unwrap();
    manager.tokens.write().await.get_mut("one").unwrap().value = "new-token".into();
    let recovered = owner.recover(&lease).await.unwrap();
    assert_eq!(recovered.account_id, "one");
    assert_eq!(recovered.revision, lease.revision);
    assert_eq!(recovered.token, "new-token");
    let (url, seen, release, server) = fixture(200, exhausted()).await;
    let worker = manager.clone();
    let query = tokio::spawn(async move {
        worker
            .quota_json(Some("one"), |_, _| client().get(&url))
            .await
    });
    seen.await.unwrap();
    query.abort();
    assert!(query.await.unwrap_err().is_cancelled());
    release.send(()).unwrap();
    server.await.unwrap();
    assert_eq!(manager.quota_blocked("one", &lease.revision, None), None);
}
