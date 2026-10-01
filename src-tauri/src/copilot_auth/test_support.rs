use super::*;

pub(crate) async fn seeded() -> (tempfile::TempDir, Arc<CopilotAuthManager>) {
    let dir = tempfile::tempdir().unwrap();
    let manager = Arc::new(CopilotAuthManager::new(dir.path().join("auth.json"), None));
    for (id, github_token) in [(1, "github-one"), (2, "github-two")] {
        manager.accounts.write().await.insert(
            id.to_string(),
            GitHubAccountData {
                github_token: github_token.into(),
                user: GitHubUser {
                    login: format!("user-{id}"),
                    id,
                    avatar_url: None,
                },
                authenticated_at: id as i64,
                revision: new_account_revision(),
            },
        );
        manager.copilot_tokens.write().await.insert(
            id.to_string(),
            CopilotToken {
                token: format!("{id}-access"),
                expires_at: chrono::Utc::now().timestamp() + 3600,
            },
        );
    }
    *manager.default_account_id.write().await = Some("1".into());
    (dir, manager)
}
