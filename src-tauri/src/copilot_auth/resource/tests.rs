use super::*;
use crate::shared::usage_http::test_support::read_headers;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;

#[tokio::test]
async fn actual_account_model_query_preserves_units_deduplicates_and_matches_routing() {
    let (_dir, manager) = test_support::seeded().await;
    let owner = manager.owned_account(Some("1"), None).await.unwrap();
    let (usage_url, usage_seen) = immediate(
        200,
        serde_json::json!({"copilot_plan":"individual_pro",
        "quota_snapshots":{"premium_interactions":{"entitlement":300,"quota_remaining":0}}})
        .to_string(),
    )
    .await;
    owner.usage_at(&usage_url, query_deadline()).await.unwrap();
    usage_seen.await.unwrap();
    let (url, seen) = immediate(200, serde_json::json!({"data":[
        {"id":"free","name":"Free","vendor":"fixture","model_picker_enabled":true,"billing":{"is_premium":false,"multiplier":0}},
        {"id":"paid","name":"Paid","vendor":"fixture","model_picker_enabled":true,"billing":{"is_premium":true,"multiplier":0.33}},
        {"id":"paid","name":"Other title","vendor":"fixture","model_picker_enabled":true,"billing":{"is_premium":true,"multiplier":1}},
        {"id":"mixed","name":"Mixed","vendor":"fixture","model_picker_enabled":true,"billing":{"is_premium":true,"multiplier":1}},
        {"id":"mixed","name":"Mixed","vendor":"fixture","model_picker_enabled":true,"billing":{"is_premium":false,"multiplier":0}},
        {"id":"unknown","name":"Unknown","vendor":"fixture","model_picker_enabled":true,"billing":{"is_premium":true,"multiplier":-1}},
        {"id":"hidden","name":"Hidden","vendor":"fixture","model_picker_enabled":false}
    ]}).to_string()).await;
    let models = owner.models_at(&url, query_deadline()).await.unwrap();
    seen.await.unwrap();
    use crate::shared::model_billing::BillingKind;
    assert_eq!(models.len(), 4);
    assert_eq!(models[0].billing.kind, BillingKind::Free);
    assert_eq!(models[1].name, "Paid");
    assert_eq!(models[1].billing.kind, BillingKind::Premium);
    assert_eq!(models[1].billing.multiplier, None);
    assert_eq!(models[2].billing.kind, BillingKind::Unknown);
    assert_eq!(models[3].billing.kind, BillingKind::Unknown);
    assert!(manager
        .quota_blocked("1", &owner.account.revision, Some("paid"))
        .is_some());
    for id in ["free", "mixed", "unknown"] {
        assert_eq!(
            manager.quota_blocked("1", &owner.account.revision, Some(id)),
            None
        );
    }
    let value = serde_json::to_value(models).unwrap();
    assert_eq!(value[0]["billing"]["multiplier"], 0.0);
    assert_eq!(value[1]["billing"]["kind"], "premium");
}

fn usage() -> String {
    serde_json::json!({
        "copilot_plan": "individual_pro",
        "quota_reset_date": "2026-11-01",
        "quota_snapshots": {
            "premium_interactions": {"entitlement": 7000, "quota_remaining": 6995.5},
            "chat": {"unlimited": true}
        }
    })
    .to_string()
}

fn models() -> String {
    serde_json::json!({"data": [
        {"id": "model-one", "name": "Model one", "vendor": "fixture", "model_picker_enabled": true},
        {"id": "hidden", "name": "Hidden", "vendor": "fixture", "model_picker_enabled": false}
    ]})
    .to_string()
}

async fn fixture(
    status: u16,
    body: String,
    gate_body: bool,
) -> (
    String,
    oneshot::Receiver<String>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/resource", listener.local_addr().unwrap());
    let (seen, observed) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let headers = read_headers(&mut socket).await;
        let reply = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        if gate_body {
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
        let _ = seen.send(headers);
        released.await.unwrap();
        if !gate_body {
            let _ = socket.write_all(reply.as_bytes()).await;
        }
        let _ = socket.write_all(body.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    (url, observed, release, task)
}

async fn immediate(status: u16, body: String) -> (String, oneshot::Receiver<String>) {
    let (url, observed, release, _) = fixture(status, body, false).await;
    release.send(()).unwrap();
    (url, observed)
}

#[tokio::test]
async fn managed_accounts_query_independently_and_partial_model_failure_preserves_quota() {
    let (_dir, manager) = test_support::seeded().await;
    let account = manager
        .get_status()
        .await
        .accounts
        .into_iter()
        .find(|a| a.id == "2")
        .unwrap();
    let (usage_url, usage_seen) = immediate(200, usage()).await;
    let (models_url, models_seen) = immediate(503, "secret upstream response".into()).await;
    let result = manager
        .resources_at(
            "2",
            &account.revision,
            &usage_url,
            &models_url,
            query_deadline(),
        )
        .await
        .unwrap();
    assert_eq!(result.account.id, "2");
    assert_eq!(result.account.revision, account.revision);
    assert!(result.models.is_none());
    assert!(matches!(
        result.models_error,
        Some(CopilotResourceFailure::Unavailable)
    ));
    assert!(result.usage_error.is_none());
    let snapshots = result.usage.unwrap().quota_snapshots;
    assert_eq!(
        snapshots.premium_interactions.unwrap().remaining,
        Some(6995.5)
    );
    assert!(snapshots.chat.unwrap().unlimited);
    assert!(snapshots.completions.is_none());
    assert!(usage_seen.await.unwrap().contains("token github-two"));
    assert!(models_seen.await.unwrap().contains("Bearer 2-access"));
    assert_eq!(
        manager.default_account_id.read().await.as_deref(),
        Some("1")
    );
}

#[tokio::test]
async fn stale_list_revisions_and_missing_ids_never_query_a_replacement_or_default() {
    let (_dir, manager) = test_support::seeded().await;
    let account = manager.get_status().await.accounts.remove(0);
    manager
        .accounts
        .write()
        .await
        .get_mut(&account.id)
        .unwrap()
        .revision = new_account_revision();
    for (id, revision) in [
        (&*account.id, &*account.revision),
        ("missing", "revision"),
        ("", "revision"),
        ("1", ""),
    ] {
        assert!(matches!(
            manager
                .resources_at(
                    id,
                    revision,
                    "http://invalid",
                    "http://invalid",
                    query_deadline()
                )
                .await,
            Err(CopilotAuthError::AccountChanged)
        ));
    }
}

#[tokio::test]
async fn removed_and_replaced_logins_own_neither_late_success_nor_error_after_headers_or_body() {
    for remove in [false, true] {
        for gate_body in [false, true] {
            for status in [200, 503] {
                let (_dir, manager) = test_support::seeded().await;
                let revision = manager.accounts.read().await["1"].revision.clone();
                let (wire_status, body) = if gate_body && status == 503 {
                    (200, "invalid JSON with secret".into())
                } else {
                    (status, usage())
                };
                let (url, seen, release, server) = fixture(wire_status, body, gate_body).await;
                let worker = manager.clone();
                let task = tokio::spawn(async move {
                    let owner = worker
                        .owned_account(Some("1"), Some(&revision))
                        .await
                        .unwrap();
                    owner.usage_at(&url, query_deadline()).await
                });
                assert!(seen.await.unwrap().contains("token github-one"));
                if remove {
                    manager.remove_account("1").await.unwrap();
                } else {
                    manager
                        .add_account_internal(
                            "new-github".into(),
                            GitHubUser {
                                id: 1,
                                login: "new-user".into(),
                                avatar_url: None,
                            },
                            CopilotToken {
                                token: "new-token".into(),
                                expires_at: chrono::Utc::now().timestamp() + 3600,
                            },
                        )
                        .await
                        .unwrap();
                }
                release.send(()).unwrap();
                assert!(matches!(
                    task.await.unwrap(),
                    Err(CopilotAuthError::AccountChanged)
                ));
                server.await.unwrap();
                if !remove {
                    assert_eq!(manager.copilot_tokens.read().await["1"].token, "new-token");
                }
            }
        }
    }
}

#[tokio::test]
async fn default_switch_during_model_query_keeps_explicit_owner_and_model_filtering() {
    let (_dir, manager) = test_support::seeded().await;
    let (url, seen, release, server) = fixture(200, models(), true).await;
    let worker = manager.clone();
    let task = tokio::spawn(async move {
        let owner = worker.owned_account(None, None).await.unwrap();
        let result = owner.models_at(&url, query_deadline()).await.unwrap();
        (owner.account.id, result)
    });
    assert!(seen.await.unwrap().contains("Bearer 1-access"));
    manager.set_default_account("2").await.unwrap();
    release.send(()).unwrap();
    let (id, result) = task.await.unwrap();
    assert_eq!(id, "1");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "model-one");
    server.await.unwrap();
}

#[tokio::test]
async fn late_model_401_retries_with_newer_same_login_token_and_never_evicts_it() {
    let (_dir, manager) = test_support::seeded().await;
    let (url, seen, release, server) =
        crate::shared::oauth_request::test_support::gated_rejection().await;
    let worker = manager.clone();
    let task = tokio::spawn(async move {
        let owner = worker.owned_account(Some("1"), None).await.unwrap();
        oauth_request::get_json(&owner, Some("1"), |lease| {
            worker.http_client.get(&url).bearer_auth(&lease.token)
        })
        .await
    });
    seen.await.unwrap();
    manager
        .copilot_tokens
        .write()
        .await
        .get_mut("1")
        .unwrap()
        .token = "newer-token".into();
    release.send(()).unwrap();
    assert_eq!(task.await.unwrap().unwrap()["result"], "ok");
    let requests = server.await.unwrap();
    assert!(requests[0].contains("Bearer 1-access"));
    assert!(requests[1].contains("Bearer newer-token"));
    assert_eq!(
        manager.copilot_tokens.read().await["1"].token,
        "newer-token"
    );
}

#[tokio::test]
async fn late_model_rejection_cannot_refresh_a_replaced_login() {
    let (_dir, manager) = test_support::seeded().await;
    let (url, seen, release, server) = fixture(401, "secret".into(), false).await;
    let worker = manager.clone();
    let task = tokio::spawn(async move {
        worker
            .owned_account(Some("1"), None)
            .await
            .unwrap()
            .models_at(&url, query_deadline())
            .await
    });
    seen.await.unwrap();
    manager
        .accounts
        .write()
        .await
        .get_mut("1")
        .unwrap()
        .revision = new_account_revision();
    manager
        .copilot_tokens
        .write()
        .await
        .get_mut("1")
        .unwrap()
        .token = "new-login-token".into();
    release.send(()).unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(CopilotAuthError::AccountChanged)
    ));
    assert_eq!(
        manager.copilot_tokens.read().await["1"].token,
        "new-login-token"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn invalid_oversized_and_failed_resources_use_sanitized_independent_errors() {
    for (status, body, expected) in [
        (200, "not JSON with secret".into(), "invalid_response"),
        (200, " ".repeat(2 * 1024 * 1024 + 1), "invalid_response"),
        (401, "secret token rejection".into(), "sign_in_required"),
        (429, "secret quota info".into(), "rate_limited"),
    ] {
        let (_dir, manager) = test_support::seeded().await;
        let revision = manager.accounts.read().await["1"].revision.clone();
        let (url, _) = immediate(status, body).await;
        let (models_url, _) = immediate(200, models()).await;
        let result = manager
            .resources_at("1", &revision, &url, &models_url, query_deadline())
            .await
            .unwrap();
        assert!(result.usage.is_none());
        assert_eq!(result.models.unwrap().len(), 1);
        assert_eq!(serde_json::to_value(result.usage_error).unwrap(), expected);
        assert_eq!(manager.accounts.read().await.len(), 2);
    }
}

#[tokio::test]
async fn shared_deadline_keeps_completed_quota_when_models_stall() {
    let (_dir, manager) = test_support::seeded().await;
    let revision = manager.accounts.read().await["1"].revision.clone();
    let (usage_url, _) = immediate(200, usage()).await;
    let (models_url, seen, release, server) = fixture(200, models(), true).await;
    let result = manager
        .resources_at(
            "1",
            &revision,
            &usage_url,
            &models_url,
            Instant::now() + Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert!(result.usage.is_some());
    assert!(matches!(
        result.models_error,
        Some(CopilotResourceFailure::Timeout)
    ));
    seen.await.unwrap();
    release.send(()).unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn rejected_old_lease_preserves_new_cache_and_stale_revision_never_refreshes() {
    let (_dir, manager) = test_support::seeded().await;
    let owner = manager.owned_account(Some("1"), None).await.unwrap();
    let lease = owner.lease(Some("1")).await.unwrap();
    manager
        .copilot_tokens
        .write()
        .await
        .get_mut("1")
        .unwrap()
        .token = "new-cache".into();
    owner.expire(&lease).await.unwrap();
    assert_eq!(manager.copilot_tokens.read().await["1"].token, "new-cache");
    manager
        .accounts
        .write()
        .await
        .get_mut("1")
        .unwrap()
        .revision = new_account_revision();
    assert!(matches!(
        owner.lease(Some("1")).await,
        Err(CopilotAuthError::AccountChanged)
    ));
    assert_eq!(manager.copilot_tokens.read().await["1"].token, "new-cache");
}
