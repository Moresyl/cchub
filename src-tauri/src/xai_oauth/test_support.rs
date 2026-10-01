use super::*;

pub(crate) async fn seeded() -> (tempfile::TempDir, Arc<XaiOAuthManager>) {
    let dir = tempfile::tempdir().unwrap();
    let manager = Arc::new(XaiOAuthManager::new(dir.path().join("auth.json"), None));
    for id in ["one", "two"] {
        manager.accounts.write().await.insert(
            id.into(),
            AccountData {
                id: id.into(),
                login: id.into(),
                refresh_token: Some("refresh".into()),
                authenticated_at: 1,
                requires_reauth: false,
                revision: new_revision(),
            },
        );
        manager.access_tokens.write().await.insert(
            id.into(),
            CachedToken {
                value: format!("{id}-access"),
                expires_at_ms: expires_at(Some(3600)),
            },
        );
    }
    (dir, manager)
}
