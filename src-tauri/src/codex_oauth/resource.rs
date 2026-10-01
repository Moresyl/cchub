use super::*;
use crate::shared::oauth_request::{self, ResourceError, TokenLease, TokenProvider};

impl CodexOAuthManager {
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
            .is_some_and(|account| account.revision == revision && !account.requires_reauth)
        {
            action();
            true
        } else {
            false
        }
    }

    pub(crate) async fn resource_json<F>(
        &self,
        account_id: Option<&str>,
        request: F,
    ) -> Result<Value, CodexOAuthError>
    where
        F: Fn(&str, &str) -> reqwest::RequestBuilder,
    {
        oauth_request::get_json(self, account_id, |lease| {
            request(&lease.account_id, &lease.token)
        })
        .await
    }

    async fn lease_revision(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<TokenLease, CodexOAuthError> {
        let _guard = self.mutation_lock.lock().await;
        let accounts = self.accounts.read().await;
        let account = accounts.get(id).ok_or(CodexOAuthError::AccountChanged)?;
        if account.revision != revision {
            return Err(CodexOAuthError::AccountChanged);
        }
        if account.requires_reauth {
            return Err(CodexOAuthError::ReauthRequired);
        }
        let tokens = self.tokens.read().await;
        let token = tokens
            .get(id)
            .filter(|token| token.usable())
            .ok_or(CodexOAuthError::AccountChanged)?;
        Ok(TokenLease {
            account_id: id.into(),
            revision: revision.into(),
            token: token.value.clone(),
        })
    }

    async fn invalidate_lease(
        &self,
        lease: &TokenLease,
        terminal: bool,
    ) -> Result<(), CodexOAuthError> {
        let _guard = self.mutation_lock.lock().await;
        self.validate(lease).await?;
        let matches = self
            .tokens
            .read()
            .await
            .get(&lease.account_id)
            .is_some_and(|token| token.value == lease.token);
        if matches {
            if terminal {
                self.record_reauth(&lease.account_id).await?;
                return Err(CodexOAuthError::ReauthRequired);
            }
            self.tokens.write().await.remove(&lease.account_id);
        } else if terminal {
            return Err(CodexOAuthError::AccountChanged);
        }
        Ok(())
    }
}

impl TokenProvider for CodexOAuthManager {
    type Error = CodexOAuthError;

    async fn lease(&self, requested: Option<&str>) -> Result<TokenLease, Self::Error> {
        let id = self.resolve_account_id(requested).await.ok_or_else(|| {
            CodexOAuthError::AccountNotFound("No OAuth account is available".into())
        })?;
        let revision = {
            let _guard = self.mutation_lock.lock().await;
            self.accounts
                .read()
                .await
                .get(&id)
                .ok_or(CodexOAuthError::AccountChanged)?
                .revision
                .clone()
        };
        self.get_token_for_revision_using(Some(&id), Some(&revision), |token| async move {
            self.refresh_token(&token).await
        })
        .await?;
        self.lease_revision(&id, &revision).await
    }

    async fn validate(&self, lease: &TokenLease) -> Result<(), Self::Error> {
        let accounts = self.accounts.read().await;
        let account = accounts
            .get(&lease.account_id)
            .ok_or(CodexOAuthError::AccountChanged)?;
        if account.revision != lease.revision {
            return Err(CodexOAuthError::AccountChanged);
        }
        if account.requires_reauth {
            return Err(CodexOAuthError::ReauthRequired);
        }
        Ok(())
    }

    async fn recover(&self, lease: &TokenLease) -> Result<TokenLease, Self::Error> {
        self.invalidate_lease(lease, false).await?;
        self.get_token_for_revision_using(
            Some(&lease.account_id),
            Some(&lease.revision),
            |token| async move { self.refresh_token(&token).await },
        )
        .await?;
        self.lease_revision(&lease.account_id, &lease.revision)
            .await
    }

    async fn expire(&self, lease: &TokenLease) -> Result<(), Self::Error> {
        self.invalidate_lease(lease, true).await
    }
}

impl From<ResourceError> for CodexOAuthError {
    fn from(error: ResourceError) -> Self {
        match error {
            ResourceError::Transport => Self::Network("OAuth resource request failed".into()),
            ResourceError::Timeout => Self::Network("OAuth resource query timed out".into()),
            ResourceError::Http(status) => {
                Self::TokenFetchFailed(format!("OAuth API returned HTTP {status}"))
            }
            ResourceError::InvalidPayload => Self::Parse("OAuth API returned invalid JSON".into()),
            ResourceError::TooLarge => Self::Parse("OAuth response exceeds the 2 MiB limit".into()),
        }
    }
}

#[cfg(test)]
mod tests;
