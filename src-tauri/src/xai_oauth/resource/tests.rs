use super::*;
use crate::xai_oauth::test_support::seeded;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn revision_matching_is_scoped_and_rejects_changed_reauth_removed_or_locked_accounts() {
    let (_dir, manager) = seeded().await;
    let lease = manager.lease(Some("one")).await.unwrap();
    assert!(manager.matches_account_revision(&lease.account_id, &lease.revision));
    assert!(!manager.matches_account_revision("two", &lease.revision));
    assert!(!manager.matches_account_revision("one", "old-revision"));
    assert!(manager.with_account_revision("one", &lease.revision, || {
        assert!(
            manager.accounts.try_write().is_err(),
            "the commit must hold account ownership"
        );
    }));
    assert!(!manager.with_account_revision("one", "old-revision", || panic!("stale commit")));
    {
        let mut accounts = manager.accounts.write().await;
        assert!(!manager.matches_account_revision("one", &lease.revision));
        accounts.get_mut("one").unwrap().requires_reauth = true;
    }
    assert!(!manager.matches_account_revision("one", &lease.revision));
    manager.accounts.write().await.remove("one");
    assert!(!manager.matches_account_revision("one", &lease.revision));
}

#[tokio::test]
async fn actual_resource_pipeline_keeps_original_account_after_default_changes() {
    let (_dir, manager) = seeded().await;
    *manager.default_account_id.write().await = Some("one".into());
    let (url, seen, release, server) =
        crate::shared::oauth_request::test_support::gated_rejection().await;
    let worker = manager.clone();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap();
    let query = tokio::spawn(async move {
        worker
            .resource_json(None, |token| client.get(&url).bearer_auth(token))
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), seen)
        .await
        .unwrap()
        .unwrap();
    *manager.default_account_id.write().await = Some("two".into());
    manager
        .access_tokens
        .write()
        .await
        .get_mut("one")
        .unwrap()
        .value = "fresh-one".into();
    release.send(()).unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), query)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(result["result"], "ok");
    let headers = server.await.unwrap();
    assert!(headers[0].contains("Bearer one-access"));
    assert!(headers[1].contains("Bearer fresh-one"));

    assert_eq!(
        manager.access_tokens.read().await["two"].value,
        "two-access"
    );
    assert!(!manager.accounts.read().await["one"].requires_reauth);
    assert!(!manager.storage_path.exists());
}

#[tokio::test]
async fn rejected_lease_preserves_other_accounts_and_newer_cached_tokens() {
    let (_dir, manager) = seeded().await;
    let lease = manager.lease(Some("one")).await.unwrap();
    manager
        .access_tokens
        .write()
        .await
        .get_mut("one")
        .unwrap()
        .value = "new-access".into();
    manager.invalidate_lease(&lease, false).await.unwrap();
    assert_eq!(manager.recover(&lease).await.unwrap().token, "new-access");
    assert!(matches!(
        manager.expire(&lease).await,
        Err(XaiOAuthError::AccountChanged)
    ));
    assert!(!manager.accounts.read().await["one"].requires_reauth);
    assert_eq!(
        manager.access_tokens.read().await["two"].value,
        "two-access"
    );
    assert!(!manager.storage_path.exists());
}

#[tokio::test]
async fn repeated_rejection_is_durable_and_cannot_fall_back_to_another_account() {
    let (_dir, manager) = seeded().await;
    let lease = manager.lease(Some("one")).await.unwrap();
    assert!(matches!(
        manager.expire(&lease).await,
        Err(XaiOAuthError::ReauthRequired(_))
    ));
    let store: Store = serde_json::from_slice(&fs::read(&manager.storage_path).unwrap()).unwrap();
    assert!(store.accounts["one"].requires_reauth);
    assert!(!store.accounts["two"].requires_reauth);
    assert!(!manager.access_tokens.read().await.contains_key("one"));
    assert_eq!(
        manager.access_tokens.read().await["two"].value,
        "two-access"
    );
    assert!(matches!(
        manager.lease(Some("one")).await,
        Err(XaiOAuthError::ReauthRequired(_))
    ));
    assert!(matches!(
        manager.lease(Some("missing")).await,
        Err(XaiOAuthError::AccountNotFound(_))
    ));
}

#[tokio::test]
async fn replaced_or_removed_login_rejects_old_lease_without_mutation() {
    for removed in [false, true] {
        let (_dir, manager) = seeded().await;
        let lease = manager.lease(Some("one")).await.unwrap();
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
        }
        assert!(matches!(
            manager.invalidate_lease(&lease, false).await,
            Err(XaiOAuthError::AccountChanged)
        ));
        assert!(matches!(
            manager.expire(&lease).await,
            Err(XaiOAuthError::AccountChanged)
        ));
        assert_eq!(
            manager.access_tokens.read().await["one"].value,
            "one-access"
        );
        assert!(!manager.storage_path.exists());
    }
}

#[tokio::test]
async fn simultaneous_rejections_share_the_actual_refresh() {
    let (_dir, manager) = seeded().await;
    let lease = manager.lease(Some("one")).await.unwrap();
    manager.invalidate_lease(&lease, false).await.unwrap();
    manager.invalidate_lease(&lease, false).await.unwrap();
    let calls = AtomicUsize::new(0);
    let refresh = |_| async {
        calls.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        Ok(TokenPayload {
            access_token: "fresh".into(),
            refresh_token: None,
            id_token: None,
            expires_in: Some(3600),
        })
    };
    let (one, two) = tokio::join!(
        manager.get_token_for_revision_using(Some("one"), Some(&lease.revision), refresh),
        manager.get_token_for_revision_using(Some("one"), Some(&lease.revision), refresh),
    );
    assert_eq!(one.unwrap(), "fresh");
    assert_eq!(two.unwrap(), "fresh");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!manager.accounts.read().await["one"].requires_reauth);
}

#[tokio::test]
async fn changed_login_before_or_while_waiting_for_refresh_never_contacts_the_vendor() {
    for during_wait in [false, true] {
        let (_dir, manager) = seeded().await;
        let lease = manager.lease(Some("one")).await.unwrap();
        manager.invalidate_lease(&lease, false).await.unwrap();
        let lock = manager.refresh_lock("one").await;
        let guard = lock.lock().await;
        let future =
            manager.get_token_for_revision_using(Some("one"), Some(&lease.revision), |_| async {
                panic!("an old resource request cannot refresh a replacement login");
            });
        tokio::pin!(future);
        if during_wait {
            tokio::select! {
                biased;
                result = &mut future => panic!("refresh unexpectedly completed: {result:?}"),
                _ = tokio::task::yield_now() => {}
            }
        }
        manager
            .accounts
            .write()
            .await
            .get_mut("one")
            .unwrap()
            .revision = new_revision();
        drop(guard);
        assert!(matches!(future.await, Err(XaiOAuthError::AccountChanged)));
        assert!(!manager.accounts.read().await["one"].requires_reauth);
        assert!(!manager.storage_path.exists());
    }
}
