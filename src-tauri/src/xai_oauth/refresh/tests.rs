use super::*;

async fn seeded() -> (tempfile::TempDir, Arc<XaiOAuthManager>) {
    let dir = tempfile::tempdir().unwrap();
    let manager = Arc::new(XaiOAuthManager::new(dir.path().join("auth.json"), None));
    for id in ["one", "two"] {
        manager.accounts.write().await.insert(
            id.into(),
            AccountData {
                id: id.into(),
                login: id.into(),
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
async fn rejection_is_persisted_and_only_affects_its_own_account() {
    let (_dir, manager) = seeded().await;
    assert!(matches!(
        manager
            .get_token_using(Some("one"), |_| async {
                Err(XaiOAuthError::RefreshTokenInvalid)
            })
            .await,
        Err(XaiOAuthError::ReauthRequired(_))
    ));
    let stored: Store = serde_json::from_slice(&fs::read(&manager.storage_path).unwrap()).unwrap();
    assert!(stored.accounts["one"].requires_reauth);
    assert!(!stored.accounts["two"].requires_reauth);
    assert!(matches!(
        manager.set_default_account("one").await,
        Err(XaiOAuthError::ReauthRequired(_))
    ));
    assert!(manager
        .get_token_using(Some("one"), |_| async {
            panic!("refused credentials must not be retried")
        })
        .await
        .is_err());
    assert!(
        manager
            .list_accounts()
            .await
            .iter()
            .find(|account| account.id == "one")
            .unwrap()
            .requires_reauth
    );
}

#[tokio::test]
async fn transient_failure_preserves_sign_in_and_success_can_retry() {
    let (_dir, manager) = seeded().await;
    assert!(manager
        .get_token_using(Some("one"), |_| async {
            Err(XaiOAuthError::Network("offline".into()))
        })
        .await
        .is_err());
    assert!(!manager.accounts.read().await["one"].requires_reauth);
    assert_eq!(
        manager
            .get_token_using(Some("one"), |_| async { Ok(fresh()) })
            .await
            .unwrap(),
        "new-access"
    );
}

#[tokio::test]
async fn old_refresh_results_cannot_expire_or_cache_over_a_new_login() {
    for refused in [true, false] {
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
                        Err(XaiOAuthError::RefreshTokenInvalid)
                    } else {
                        Ok(fresh())
                    }
                })
                .await
        });
        started.await.unwrap();
        {
            let _guard = manager.mutation_lock.lock().await;
            manager
                .accounts
                .write()
                .await
                .get_mut("one")
                .unwrap()
                .revision = new_revision();
            manager.access_tokens.write().await.insert(
                "one".into(),
                CachedToken {
                    value: "new-login".into(),
                    expires_at_ms: expires_at(Some(3600)),
                },
            );
        }
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
            "new-login"
        );
        finish.send(()).unwrap();
        assert!(matches!(
            task.await.unwrap(),
            Err(XaiOAuthError::AccountChanged)
        ));
        assert!(!manager.accounts.read().await["one"].requires_reauth);
        assert_eq!(manager.access_tokens.read().await["one"].value, "new-login");
        assert!(!manager.storage_path.exists());
    }
}

#[tokio::test]
async fn empty_or_wrong_account_token_cannot_be_cached() {
    let (_dir, manager) = seeded().await;
    let mut tokens = fresh();
    tokens.access_token.clear();
    assert!(matches!(
        manager
            .get_token_using(Some("one"), |_| async { Ok(tokens) })
            .await,
        Err(XaiOAuthError::Parse(_))
    ));
    let claims = URL_SAFE_NO_PAD.encode(br#"{"sub":"two"}"#);
    let mut tokens = fresh();
    tokens.id_token = Some(format!("header.{claims}.signature"));
    assert!(matches!(
        manager
            .get_token_using(Some("one"), |_| async { Ok(tokens) })
            .await,
        Err(XaiOAuthError::Parse(_))
    ));
    assert!(manager.access_tokens.read().await.is_empty());
}

#[tokio::test]
async fn cancelled_device_flow_rejects_late_authorization() {
    let (_dir, manager) = seeded().await;
    manager.pending.write().await.insert(
        "device".into(),
        PendingDeviceCode {
            token_endpoint: "https://auth.x.ai/token".into(),
            expires_at_ms: expires_at(Some(300)),
            interval_secs: 5,
            next_poll_at_ms: 0,
        },
    );
    manager.cancel_device_flow("device").await;
    assert!(matches!(
        manager
            .add_account("one".into(), "one".into(), "refresh".into(), None, "device")
            .await,
        Err(XaiOAuthError::ExpiredToken)
    ));
    assert!(!manager.storage_path.exists());
}
