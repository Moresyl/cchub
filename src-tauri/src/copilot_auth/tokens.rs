use super::*;
use crate::shared::oauth_request::TokenLease;
use std::future::Future;

impl CopilotAuthManager {
    #[cfg(test)]
    pub(crate) fn matches_account_revision(&self, id: &str, revision: &str) -> bool {
        self.with_account_revision(id, revision, || {})
    }

    pub(crate) fn with_account_revision(
        &self,
        id: &str,
        revision: &str,
        action: impl FnOnce(),
    ) -> bool {
        let Ok(accounts) = self.accounts.try_read() else {
            return false;
        };
        if accounts
            .get(id)
            .is_some_and(|account| account.revision == revision)
        {
            action();
            true
        } else {
            false
        }
    }

    pub async fn get_valid_token_for_account(
        &self,
        requested: Option<&str>,
    ) -> Result<String, CopilotAuthError> {
        self.lease(requested).await.map(|lease| lease.token)
    }

    pub(crate) async fn lease(
        &self,
        requested: Option<&str>,
    ) -> Result<TokenLease, CopilotAuthError> {
        self.lease_using(requested, |token| async move {
            self.fetch_copilot_token_with_github_token(&token).await
        })
        .await
    }

    async fn lease_using<F, Fut>(
        &self,
        requested: Option<&str>,
        refresh: F,
    ) -> Result<TokenLease, CopilotAuthError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<CopilotToken, CopilotAuthError>>,
    {
        let id = self
            .resolve_account_id(requested)
            .await
            .ok_or(CopilotAuthError::GitHubTokenInvalid)?;
        let revision = {
            let _guard = self.mutation_lock.lock().await;
            let accounts = self.accounts.read().await;
            let account = accounts.get(&id).ok_or(CopilotAuthError::AccountChanged)?;
            if let Some(token) = self.cached_lease(&id, &account.revision).await {
                return Ok(token);
            }
            account.revision.clone()
        };
        let refresh_lock = self.get_refresh_lock(&id).await;
        let _refresh_guard = refresh_lock.lock().await;
        let github_token = {
            let _guard = self.mutation_lock.lock().await;
            let accounts = self.accounts.read().await;
            let account = accounts.get(&id).ok_or(CopilotAuthError::AccountChanged)?;
            if account.revision != revision {
                return Err(CopilotAuthError::AccountChanged);
            }
            if let Some(token) = self.cached_lease(&id, &revision).await {
                return Ok(token);
            }
            account.github_token.clone()
        };
        let result = refresh(github_token).await;
        let _guard = self.mutation_lock.lock().await;
        if self
            .accounts
            .read()
            .await
            .get(&id)
            .is_none_or(|account| account.revision != revision)
        {
            return Err(CopilotAuthError::AccountChanged);
        }
        let token = result?;
        validate_token(&token)?;
        let lease = TokenLease {
            account_id: id.clone(),
            revision,
            token: token.token.clone(),
        };
        self.copilot_tokens.write().await.insert(id, token);
        Ok(lease)
    }

    async fn cached_lease(&self, id: &str, revision: &str) -> Option<TokenLease> {
        let tokens = self.copilot_tokens.read().await;
        let token = tokens
            .get(id)
            .filter(|token| !token.is_expiring_soon() && !token.token.trim().is_empty())?;
        Some(TokenLease {
            account_id: id.into(),
            revision: revision.into(),
            token: token.token.clone(),
        })
    }

    pub(super) async fn fetch_copilot_token_with_github_token(
        &self,
        github_token: &str,
    ) -> Result<CopilotToken, CopilotAuthError> {
        let response = self
            .http_client
            .get(COPILOT_TOKEN_URL)
            .header("Authorization", format!("token {github_token}"))
            .header("User-Agent", COPILOT_USER_AGENT)
            .header("Editor-Version", COPILOT_EDITOR_VERSION)
            .header("Editor-Plugin-Version", COPILOT_PLUGIN_VERSION)
            .send()
            .await
            .map_err(|_| CopilotAuthError::Network("Copilot token request failed".into()))?;
        match response.status() {
            reqwest::StatusCode::UNAUTHORIZED => return Err(CopilotAuthError::GitHubTokenInvalid),
            reqwest::StatusCode::FORBIDDEN => return Err(CopilotAuthError::NoCopilotSubscription),
            status if !status.is_success() => {
                return Err(CopilotAuthError::Token(format!(
                    "Copilot API returned HTTP {status}"
                )))
            }
            _ => {}
        }
        decode_token(response).await
    }
}

async fn decode_token(response: reqwest::Response) -> Result<CopilotToken, CopilotAuthError> {
    let value = crate::shared::oauth_request::read_json(response)
        .await
        .map_err(|_| {
            CopilotAuthError::Parse("Copilot API returned invalid or oversized token data".into())
        })?;
    let response: CopilotTokenResponse = serde_json::from_value(value)
        .map_err(|_| CopilotAuthError::Parse("Copilot API returned invalid token data".into()))?;
    let token = CopilotToken {
        token: response.token,
        expires_at: response.expires_at,
    };
    validate_token(&token)?;
    Ok(token)
}

fn validate_token(token: &CopilotToken) -> Result<(), CopilotAuthError> {
    if token.token.trim().is_empty() || token.expires_at <= chrono::Utc::now().timestamp() {
        return Err(CopilotAuthError::Parse(
            "Copilot API returned an empty or expired token".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod http_tests;
