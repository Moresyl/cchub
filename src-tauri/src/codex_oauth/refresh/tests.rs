use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

async fn seeded() -> (tempfile::TempDir, Arc<CodexOAuthManager>) {
    let dir = tempfile::tempdir().unwrap();
    let manager = Arc::new(CodexOAuthManager::new(dir.path().join("auth.json"), None));
    for id in ["one", "two"] {
        manager.accounts.write().await.insert(
            id.into(),
            AccountData {
                id: id.into(),
                email: None,
                refresh_token: Some("old-refresh".into()),
                authenticated_at: 1,
                requires_reauth: false,
                revision: new_revision(),
            },
        );
    }
    (dir, manager)
}

fn fresh() -> TokenPayload {
    TokenPayload {
        access_token: "new-access".into(),
        refresh_token: None,
        id_token: None,
        expires_in: Some(3600),
    }
}

#[tokio::test]
async fn rejected_refresh_is_durable_account_scoped_and_not_repeated() {
    let (_dir, manager) = seeded().await;
    manager.tokens.write().await.insert(
        "one".into(),
        CachedToken {
            value: "expired".into(),
            expires_at_ms: 0,
        },
    );
    assert!(matches!(
        manager
            .get_token_using(Some("one"), |_| async {
                Err(CodexOAuthError::RefreshTokenInvalid)
            })
            .await,
        Err(CodexOAuthError::ReauthRequired)
    ));
    let stored: Store = serde_json::from_slice(&fs::read(&manager.storage_path).unwrap()).unwrap();
    assert!(stored.accounts["one"].requires_reauth);
    assert!(!stored.accounts["two"].requires_reauth);
    assert!(!manager.tokens.read().await.contains_key("one"));
    assert!(matches!(
        manager.set_default_account("one").await,
        Err(CodexOAuthError::ReauthRequired)
    ));
    assert!(
        manager
            .list_accounts()
            .await
            .iter()
            .find(|account| account.id == "one")
            .unwrap()
            .requires_reauth
    );
    assert!(matches!(
        manager
            .get_token_using(Some("one"), |_| async {
                panic!("must not retry refused credentials")
            })
            .await,
        Err(CodexOAuthError::ReauthRequired)
    ));
}

#[tokio::test]
async fn transient_failure_keeps_the_account_and_can_retry() {
    let (_dir, manager) = seeded().await;
    assert!(matches!(
        manager
            .get_token_using(Some("one"), |_| async {
                Err(CodexOAuthError::Network("offline".into()))
            })
            .await,
        Err(CodexOAuthError::Network(_))
    ));
    assert!(!manager.accounts.read().await["one"].requires_reauth);
    assert!(!manager.storage_path.exists());
    assert_eq!(
        manager
            .get_token_using(Some("one"), |_| async { Ok(fresh()) })
            .await
            .unwrap(),
        "new-access"
    );
}

#[tokio::test]
async fn old_success_or_failure_cannot_change_a_new_sign_in_or_deleted_account() {
    for (refused, removed) in [(true, false), (false, false), (false, true)] {
        let (_dir, manager) = seeded().await;
        let (start, started) = tokio::sync::oneshot::channel();
        let (finish, finished) = tokio::sync::oneshot::channel();
        let worker = manager.clone();
        let task = tokio::spawn(async move {
            worker
                .get_token_using(Some("one"), |_| async move {
                    start.send(()).unwrap();
                    finished.await.unwrap();
                    if refused {
                        Err(CodexOAuthError::RefreshTokenInvalid)
                    } else {
                        Ok(fresh())
                    }
                })
                .await
        });
        started.await.unwrap();
        {
            let _guard = manager.mutation_lock.lock().await;
            if removed {
                manager.accounts.write().await.remove("one");
            } else {
                manager
                    .accounts
                    .write()
                    .await
                    .get_mut("one")
                    .unwrap()
                    .revision = new_revision();
                manager.tokens.write().await.insert(
                    "one".into(),
                    CachedToken {
                        value: "signed-in-now".into(),
                        expires_at_ms: expires_at(Some(3600)),
                    },
                );
            }
        }
        if !removed {
            assert_eq!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    manager.get_token_using(Some("one"), |_| async {
                        panic!("new login must not wait for an old refresh")
                    })
                )
                .await
                .unwrap()
                .unwrap(),
                "signed-in-now"
            );
        }
        finish.send(()).unwrap();
        assert!(matches!(
            task.await.unwrap(),
            Err(CodexOAuthError::AccountChanged)
        ));
        if !removed {
            assert!(!manager.accounts.read().await["one"].requires_reauth);
            assert_eq!(manager.tokens.read().await["one"].value, "signed-in-now");
        } else {
            assert!(!manager.tokens.read().await.contains_key("one"));
        }
        assert!(!manager.storage_path.exists());
    }
}

#[tokio::test]
async fn simultaneous_token_reads_only_refresh_once() {
    let (_dir, manager) = seeded().await;
    let calls = AtomicUsize::new(0);
    let refresh = |_| async {
        calls.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        Ok(fresh())
    };
    let (first, second) = tokio::join!(
        manager.get_token_using(Some("one"), refresh),
        manager.get_token_using(Some("one"), refresh)
    );
    assert_eq!(first.unwrap(), "new-access");
    assert_eq!(second.unwrap(), "new-access");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn refresh_rejects_empty_tokens_and_other_account_identity() {
    let (_dir, manager) = seeded().await;
    let mut tokens = fresh();
    tokens.access_token.clear();
    assert!(matches!(
        manager
            .get_token_using(Some("one"), |_| async { Ok(tokens) })
            .await,
        Err(CodexOAuthError::Parse(_))
    ));
    let claims = URL_SAFE_NO_PAD.encode(br#"{"chatgpt_account_id":"two"}"#);
    let mut tokens = fresh();
    tokens.id_token = Some(format!("header.{claims}.signature"));
    assert!(matches!(
        manager
            .get_token_using(Some("one"), |_| async { Ok(tokens) })
            .await,
        Err(CodexOAuthError::Parse(_))
    ));
    assert!(manager.tokens.read().await.is_empty());
    assert!(!manager.accounts.read().await["one"].requires_reauth);
}

#[test]
fn legacy_accounts_receive_a_revision_without_becoming_expired() {
    let raw = r#"{"id":"one","email":null,"authenticated_at":1}"#;
    let first: AccountData = serde_json::from_str(raw).unwrap();
    let second: AccountData = serde_json::from_str(raw).unwrap();
    assert!(!first.requires_reauth);
    assert_ne!(first.revision, second.revision);
}

#[tokio::test]
async fn cancelled_device_flow_cannot_commit_a_late_login() {
    let (_dir, manager) = seeded().await;
    manager.pending.write().await.insert(
        "device".into(),
        PendingDeviceCode {
            user_code: "code".into(),
            expires_at_ms: expires_at(Some(300)),
        },
    );
    manager.cancel_device_flow("device").await;
    assert!(matches!(
        manager
            .add_account(
                "one".into(),
                "refresh".into(),
                None,
                "device",
                CachedToken {
                    value: "access".into(),
                    expires_at_ms: expires_at(Some(3600))
                }
            )
            .await,
        Err(CodexOAuthError::ExpiredToken)
    ));
    assert!(!manager.storage_path.exists());
    assert!(!manager.accounts.read().await["one"].requires_reauth);
}
