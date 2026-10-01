use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

async fn seeded() -> (tempfile::TempDir, Arc<CopilotAuthManager>) {
    let (dir, manager) = crate::copilot_auth::test_support::seeded().await;
    manager.copilot_tokens.write().await.clear();
    (dir, manager)
}

fn fresh(value: &str) -> CopilotToken {
    CopilotToken {
        token: value.into(),
        expires_at: chrono::Utc::now().timestamp() + 3600,
    }
}

#[tokio::test]
async fn revision_matching_rejects_replaced_removed_or_locked_accounts() {
    let (_dir, manager) = seeded().await;
    let revision = manager.accounts.read().await["1"].revision.clone();
    assert!(manager.matches_account_revision("1", &revision));
    assert!(!manager.matches_account_revision("2", &revision));
    assert!(manager.with_account_revision("1", &revision, || {
        assert!(
            manager.accounts.try_write().is_err(),
            "the commit must hold account ownership"
        );
    }));
    assert!(!manager.with_account_revision("1", "old-revision", || panic!("stale commit")));
    {
        let mut accounts = manager.accounts.write().await;
        assert!(!manager.matches_account_revision("1", &revision));
        accounts.get_mut("1").unwrap().revision = new_account_revision();
    }
    assert!(!manager.matches_account_revision("1", &revision));
    manager.remove_account("1").await.unwrap();
    assert!(!manager.matches_account_revision("1", &revision));
}

#[tokio::test]
async fn resolved_lease_keeps_the_account_selected_before_an_inflight_default_switch() {
    let (_dir, manager) = seeded().await;
    let revision = manager.accounts.read().await["1"].revision.clone();
    let (start, started) = tokio::sync::oneshot::channel();
    let (finish, finished) = tokio::sync::oneshot::channel();
    let worker = manager.clone();
    let task = tokio::spawn(async move {
        worker
            .lease_using(None, |token| async move {
                assert_eq!(token, "github-one");
                start.send(()).unwrap();
                finished.await.unwrap();
                Ok(fresh("copilot-one"))
            })
            .await
    });
    started.await.unwrap();
    manager.set_default_account("2").await.unwrap();
    finish.send(()).unwrap();
    let lease = task.await.unwrap().unwrap();
    assert_eq!(lease.account_id, "1");
    assert_eq!(lease.revision, revision);
    assert_eq!(lease.token, "copilot-one");
    let next = manager
        .lease_using(None, |token| async move {
            assert_eq!(token, "github-two");
            Ok(fresh("copilot-two"))
        })
        .await
        .unwrap();
    assert_eq!(next.account_id, "2");
    assert_eq!(next.token, "copilot-two");
}

#[tokio::test]
async fn old_refresh_success_and_error_cannot_own_a_removed_or_replaced_account() {
    for (remove, fail) in [(true, false), (true, true), (false, false), (false, true)] {
        let (_dir, manager) = seeded().await;
        let (start, started) = tokio::sync::oneshot::channel();
        let (finish, finished) = tokio::sync::oneshot::channel();
        let worker = manager.clone();
        let task = tokio::spawn(async move {
            worker
                .lease_using(Some("1"), |_| async move {
                    start.send(()).unwrap();
                    finished.await.unwrap();
                    if fail {
                        Err(CopilotAuthError::GitHubTokenInvalid)
                    } else {
                        Ok(fresh("old-result"))
                    }
                })
                .await
        });
        started.await.unwrap();
        if remove {
            manager.remove_account("1").await.unwrap();
        } else {
            manager
                .add_account_internal(
                    "new-github".into(),
                    GitHubUser {
                        id: 1,
                        login: "new-login".into(),
                        avatar_url: None,
                    },
                    fresh("new-login-token"),
                )
                .await
                .unwrap();
            let lease = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                manager.lease_using(Some("1"), |_| async {
                    panic!("cached new login must not wait for an old refresh")
                }),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(lease.token, "new-login-token");
        }
        finish.send(()).unwrap();
        assert!(matches!(
            task.await.unwrap(),
            Err(CopilotAuthError::AccountChanged)
        ));
        assert_eq!(
            manager
                .copilot_tokens
                .read()
                .await
                .get("1")
                .map(|token| token.token.clone()),
            (!remove).then(|| "new-login-token".into())
        );
    }
}

#[tokio::test]
async fn concurrent_expired_token_requests_share_one_refresh_and_account_revision() {
    let (_dir, manager) = seeded().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(tokio::sync::Notify::new());
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let worker = manager.clone();
        let calls = calls.clone();
        let gate = gate.clone();
        tasks.push(tokio::spawn(async move {
            worker
                .lease_using(Some("1"), |_| async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    gate.notified().await;
                    Ok(fresh("shared-token"))
                })
                .await
                .unwrap()
        }));
    }
    while calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    gate.notify_one();
    let revision = manager.accounts.read().await["1"].revision.clone();
    for task in tasks {
        let lease = task.await.unwrap();
        assert_eq!(lease.account_id, "1");
        assert_eq!(lease.revision, revision);
        assert_eq!(lease.token, "shared-token");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn transient_and_invalid_tokens_do_not_fill_the_cache_or_remove_signin() {
    let (_dir, manager) = seeded().await;
    assert!(manager
        .lease_using(Some("1"), |_| async {
            Err(CopilotAuthError::Network("offline".into()))
        })
        .await
        .is_err());
    for token in [
        fresh(" "),
        CopilotToken {
            token: "old".into(),
            expires_at: 1,
        },
    ] {
        assert!(manager
            .lease_using(Some("1"), |_| async move { Ok(token) })
            .await
            .is_err());
    }
    assert!(manager.copilot_tokens.read().await.is_empty());
    assert_eq!(manager.accounts.read().await.len(), 2);
    assert!(manager
        .lease_using(Some("missing"), |_| async {
            panic!("missing account must not refresh")
        })
        .await
        .is_err());
    assert_eq!(
        manager
            .lease_using(Some("1"), |_| async { Ok(fresh("retry")) })
            .await
            .unwrap()
            .token,
        "retry"
    );
}

#[tokio::test]
async fn cleared_auth_does_not_resurrect_an_inflight_token_and_legacy_accounts_get_revisions() {
    let (_dir, manager) = seeded().await;
    let legacy: GitHubAccountData = serde_json::from_str(r#"{"github_token":"fixture","user":{"id":3,"login":"legacy","avatar_url":null},"authenticated_at":1}"#).unwrap();
    assert!(uuid::Uuid::parse_str(&legacy.revision).is_ok());
    let (start, started) = tokio::sync::oneshot::channel();
    let (finish, finished) = tokio::sync::oneshot::channel();
    let worker = manager.clone();
    let task = tokio::spawn(async move {
        worker
            .lease_using(Some("1"), |_| async move {
                start.send(()).unwrap();
                finished.await.unwrap();
                Ok(fresh("old-result"))
            })
            .await
    });
    started.await.unwrap();
    manager.clear_auth().await.unwrap();
    finish.send(()).unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(CopilotAuthError::AccountChanged)
    ));
    assert!(manager.accounts.read().await.is_empty());
    assert!(manager.copilot_tokens.read().await.is_empty());
}
