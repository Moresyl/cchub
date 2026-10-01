use super::*;
use crate::shared::oauth_request::{self, ResourceError, TokenLease, TokenProvider};

impl XaiOAuthManager {
    pub(crate) async fn resource_json<F>(
        &self,
        account_id: Option<&str>,
        request: F,
    ) -> Result<Value, XaiOAuthError>
    where
        F: Fn(&str) -> reqwest::RequestBuilder,
    {
        oauth_request::get_json(self, account_id, |lease| request(&lease.token)).await
    }

    async fn lease_revision(&self, id: &str, revision: &str) -> Result<TokenLease, XaiOAuthError> {
        let _guard = self.mutation_lock.lock().await;
        let accounts = self.accounts.read().await;
        let account = accounts.get(id).ok_or(XaiOAuthError::AccountChanged)?;
        if account.revision != revision {
            return Err(XaiOAuthError::AccountChanged);
        }
        if account.requires_reauth {
            return Err(XaiOAuthError::ReauthRequired(id.into()));
        }
        let tokens = self.access_tokens.read().await;
        let token = tokens
            .get(id)
            .filter(|token| token.usable())
            .ok_or(XaiOAuthError::AccountChanged)?;
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
    ) -> Result<(), XaiOAuthError> {
        let _guard = self.mutation_lock.lock().await;
        self.validate(lease).await?;
        let matches = self
            .access_tokens
            .read()
            .await
            .get(&lease.account_id)
            .is_some_and(|token| token.value == lease.token);
        if matches {
            if terminal {
                self.record_reauth(&lease.account_id).await?;
                return Err(XaiOAuthError::ReauthRequired(lease.account_id.clone()));
            }
            self.access_tokens.write().await.remove(&lease.account_id);
        } else if terminal {
            return Err(XaiOAuthError::AccountChanged);
        }
        Ok(())
    }
}

impl TokenProvider for XaiOAuthManager {
    type Error = XaiOAuthError;

    async fn lease(&self, requested: Option<&str>) -> Result<TokenLease, Self::Error> {
        let id = self
            .resolve_account_id(requested)
            .await
            .ok_or_else(|| XaiOAuthError::AccountNotFound("No xAI account is available".into()))?;
        let revision = {
            let _guard = self.mutation_lock.lock().await;
            self.accounts
                .read()
                .await
                .get(&id)
                .ok_or(XaiOAuthError::AccountChanged)?
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
            .ok_or(XaiOAuthError::AccountChanged)?;
        if account.revision != lease.revision {
            return Err(XaiOAuthError::AccountChanged);
        }
        if account.requires_reauth {
            return Err(XaiOAuthError::ReauthRequired(lease.account_id.clone()));
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

impl From<ResourceError> for XaiOAuthError {
    fn from(error: ResourceError) -> Self {
        match error {
            ResourceError::Transport => Self::Network("xAI resource request failed".into()),
            ResourceError::Timeout => Self::Network("xAI resource query timed out".into()),
            ResourceError::Http(status) => {
                Self::TokenFetchFailed(format!("xAI API returned HTTP {status}"))
            }
            ResourceError::InvalidPayload => Self::Parse("xAI API returned invalid JSON".into()),
            ResourceError::TooLarge => Self::Parse("xAI response exceeds the 2 MiB limit".into()),
        }
    }
}

#[cfg(test)]
mod tests;
