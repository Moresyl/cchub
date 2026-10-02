use super::*;

pub(crate) async fn seeded() -> (tempfile::TempDir, Arc<CodexOAuthManager>) {
    let dir = tempfile::tempdir().unwrap();
    let manager = Arc::new(CodexOAuthManager::new(dir.path().join("auth.json"), None));
    for id in ["one", "two"] {
        manager.accounts.write().await.insert(
            id.into(),
            AccountData {
                id: id.into(),
                email: None,
                refresh_token: Some("refresh".into()),
                authenticated_at: 1,
                requires_reauth: false,
                revision: new_revision(),
            },
        );
        manager.tokens.write().await.insert(
            id.into(),
            CachedToken {
                value: format!("{id}-access"),
                expires_at_ms: expires_at(Some(3600)),
            },
        );
    }
    (dir, manager)
}

pub(crate) async fn replace_cached_token(manager: &CodexOAuthManager, account: &str, token: &str) {
    manager.tokens.write().await.insert(
        account.into(),
        CachedToken {
            value: token.into(),
            expires_at_ms: expires_at(Some(3600)),
        },
    );
}
