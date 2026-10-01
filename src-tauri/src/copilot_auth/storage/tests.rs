use super::*;

#[tokio::test]
async fn failed_account_mutations_preserve_the_previous_identity_and_cached_credentials() {
    for operation in ["default", "remove", "clear", "signin"] {
        let (dir, manager) = crate::copilot_auth::test_support::seeded().await;
        manager.set_default_account("1").await.unwrap();
        let lease = manager.lease(Some("1")).await.unwrap();
        let saved = dir.path().join("saved-auth.json");
        fs::rename(&manager.storage_path, &saved).unwrap();
        let original = fs::read(&saved).unwrap();
        // A directory at the exact file location causes both replacement and deletion to fail.
        fs::create_dir(&manager.storage_path).unwrap();
        let result = match operation {
            "default" => manager.set_default_account("2").await,
            "remove" => manager.remove_account("1").await,
            "clear" => manager.clear_auth().await,
            "signin" => manager
                .add_account_internal(
                    "replacement-github".into(),
                    GitHubUser {
                        id: 1,
                        login: "replacement".into(),
                        avatar_url: None,
                    },
                    CopilotToken {
                        token: "replacement-access".into(),
                        expires_at: chrono::Utc::now().timestamp() + 3600,
                    },
                )
                .await
                .map(|_| ()),
            _ => unreachable!(),
        };
        assert!(result.is_err(), "{operation}");
        assert!(manager.matches_account_revision(&lease.account_id, &lease.revision));
        assert_eq!(manager.lease(None).await.unwrap().token, "1-access");
        assert_eq!(manager.lease(Some("2")).await.unwrap().token, "2-access");
        assert_eq!(manager.accounts.read().await["1"].user.login, "user-1");
        assert_eq!(manager.accounts.read().await.len(), 2);
        assert_eq!(fs::read(&saved).unwrap(), original);
        assert!(manager.storage_path.is_dir());
    }
}

#[tokio::test]
async fn successful_persistence_keeps_revisions_across_restart_and_removal_updates_the_default() {
    let (_dir, manager) = crate::copilot_auth::test_support::seeded().await;
    let lease = manager.lease(Some("1")).await.unwrap();
    manager.set_default_account("2").await.unwrap();
    let reloaded = CopilotAuthManager::new(manager.storage_path.clone(), None);
    assert!(reloaded.matches_account_revision(&lease.account_id, &lease.revision));
    assert_eq!(
        reloaded.default_account_id.read().await.as_deref(),
        Some("2")
    );
    assert!(reloaded.copilot_tokens.read().await.is_empty());
    manager.remove_account("2").await.unwrap();
    assert_eq!(manager.lease(None).await.unwrap().account_id, "1");
    let reloaded = CopilotAuthManager::new(manager.storage_path.clone(), None);
    assert_eq!(reloaded.accounts.read().await.len(), 1);
    assert_eq!(
        reloaded.default_account_id.read().await.as_deref(),
        Some("1")
    );
    manager.clear_auth().await.unwrap();
    assert!(!manager.storage_path.exists());
    assert!(manager.accounts.read().await.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn persisted_accounts_and_parent_directory_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, manager) = crate::copilot_auth::test_support::seeded().await;
    manager.set_default_account("1").await.unwrap();
    assert_eq!(
        fs::metadata(&manager.storage_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(manager.storage_path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}
