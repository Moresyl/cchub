use super::*;
use crate::shared::oauth_request::{self, ResourceError, TokenLease, TokenProvider};
use crate::shared::quota::Resource;

struct Owner<'a> {
    manager: &'a CodexOAuthManager,
    lease: TokenLease,
}

impl TokenProvider for Owner<'_> {
    type Error = CodexOAuthError;
    async fn lease(&self, _: Option<&str>) -> Result<TokenLease, Self::Error> {
        self.manager.validate(&self.lease).await?;
        Ok(self.lease.clone())
    }
    async fn validate(&self, lease: &TokenLease) -> Result<(), Self::Error> {
        if lease.account_id != self.lease.account_id || lease.revision != self.lease.revision {
            return Err(CodexOAuthError::AccountChanged);
        }
        self.manager.validate(lease).await
    }
    async fn recover(&self, lease: &TokenLease) -> Result<TokenLease, Self::Error> {
        self.validate(lease).await?;
        self.manager.recover(lease).await
    }
    async fn expire(&self, lease: &TokenLease) -> Result<(), Self::Error> {
        self.validate(lease).await?;
        self.manager.expire(lease).await
    }
}

impl CodexOAuthManager {
    /// Query and record quota with the same pinned login as the resource request.
    /// Models and other resource calls never populate the quota cache.
    pub(crate) async fn quota_json<F>(
        &self,
        account: Option<&str>,
        request: F,
    ) -> Result<Value, CodexOAuthError>
    where
        F: Fn(&str, &str) -> reqwest::RequestBuilder,
    {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(45);
        tokio::time::timeout_at(deadline, async {
            let id = self.resolve_account_id(account).await.ok_or_else(|| {
                CodexOAuthError::AccountNotFound("No OAuth account is available".into())
            })?;
            let (revision, query) = {
                let _guard = self.mutation_lock.lock().await;
                let accounts = self.accounts.read().await;
                let account = accounts.get(&id).ok_or(CodexOAuthError::AccountChanged)?;
                if account.requires_reauth {
                    return Err(CodexOAuthError::ReauthRequired);
                }
                let query = self
                    .quota_cache
                    .begin(&id, &account.revision, Resource::CodexUsage);
                (account.revision.clone(), query)
            };
            self.get_token_for_revision_using(Some(&id), Some(&revision), |token| async move {
                self.refresh_token(&token).await
            })
            .await?;
            let lease = self.lease_revision(&id, &revision).await?;
            let owner = Owner {
                manager: self,
                lease,
            };
            let result = oauth_request::get_json_before(
                &owner,
                None,
                |lease| request(&lease.account_id, &lease.token),
                deadline,
            )
            .await;
            let accounts = self.accounts.read().await;
            if accounts.get(&owner.lease.account_id).is_none_or(|account| {
                account.revision != owner.lease.revision || account.requires_reauth
            }) {
                return Err(CodexOAuthError::AccountChanged);
            }
            self.quota_cache.complete(query, result.as_ref().ok());
            result
        })
        .await
        .map_err(|_| CodexOAuthError::from(ResourceError::Timeout))?
    }

    pub(crate) fn quota_blocked(
        &self,
        id: &str,
        revision: &str,
        model: Option<&str>,
    ) -> Option<u64> {
        let mut blocked = None;
        self.with_account_revision(id, revision, || {
            blocked = self.quota_cache.blocked(id, revision, model);
        });
        blocked
    }
}

#[cfg(test)]
mod tests;
