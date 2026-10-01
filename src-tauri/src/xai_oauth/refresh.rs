use super::*;
use std::future::Future;

impl XaiOAuthManager {
    pub async fn get_valid_token(&self, account_id: Option<&str>) -> Result<String, XaiOAuthError> {
        self.get_token_using(account_id, |token| async move {
            self.refresh_token(&token).await
        })
        .await
    }

    async fn get_token_using<F, Fut>(
        &self,
        account_id: Option<&str>,
        refresh: F,
    ) -> Result<String, XaiOAuthError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<TokenPayload, XaiOAuthError>>,
    {
        let id = self
            .resolve_account_id(account_id)
            .await
            .ok_or_else(|| XaiOAuthError::AccountNotFound("No xAI account is available".into()))?;
        {
            let _guard = self.mutation_lock.lock().await;
            let accounts = self.accounts.read().await;
            let account = accounts
                .get(&id)
                .ok_or_else(|| XaiOAuthError::AccountNotFound(id.clone()))?;
            if account.requires_reauth {
                return Err(XaiOAuthError::ReauthRequired(id));
            }
            if let Some(token) = self.cached_token(&id).await {
                return Ok(token);
            }
        }
        let lock = self.refresh_lock(&id).await;
        let _refresh_guard = lock.lock().await;
        let (account, refresh_token) = {
            let _guard = self.mutation_lock.lock().await;
            let account = self
                .accounts
                .read()
                .await
                .get(&id)
                .cloned()
                .ok_or_else(|| XaiOAuthError::AccountNotFound(id.clone()))?;
            if account.requires_reauth {
                return Err(XaiOAuthError::ReauthRequired(id));
            }
            if let Some(token) = self.cached_token(&id).await {
                return Ok(token);
            }
            let token = match account.refresh_token.clone() {
                Some(token) => Some(token),
                None => keyring_get(&id)?,
            };
            let Some(token) = token.filter(|token| !token.trim().is_empty()) else {
                self.record_reauth(&id).await?;
                return Err(XaiOAuthError::ReauthRequired(id));
            };
            (account, token)
        };
        let result = refresh(refresh_token.clone()).await;
        let _guard = self.mutation_lock.lock().await;
        if self
            .accounts
            .read()
            .await
            .get(&id)
            .is_none_or(|current| current.revision != account.revision)
        {
            return Err(XaiOAuthError::AccountChanged);
        }
        let tokens = match result {
            Ok(tokens) => tokens,
            Err(XaiOAuthError::RefreshTokenInvalid) => {
                self.record_reauth(&id).await?;
                return Err(XaiOAuthError::ReauthRequired(id));
            }
            Err(error) => return Err(error),
        };
        if tokens.access_token.trim().is_empty() {
            return Err(XaiOAuthError::Parse("OAuth access token is empty".into()));
        }
        if token_identity(&tokens).is_some_and(|(next, _)| next != id) {
            return Err(XaiOAuthError::Parse(
                "Refreshed OAuth account identity does not match".into(),
            ));
        }
        if let Some(next) = tokens
            .refresh_token
            .as_deref()
            .filter(|value| !value.trim().is_empty() && *value != refresh_token)
        {
            let stored = keyring_set(&id, next).is_ok();
            self.accounts
                .write()
                .await
                .get_mut(&id)
                .unwrap()
                .refresh_token = (!stored).then(|| next.to_string());
            self.save_to_disk().await?;
        }
        let token = tokens.access_token;
        self.access_tokens.write().await.insert(
            id,
            CachedToken {
                value: token.clone(),
                expires_at_ms: expires_at(tokens.expires_in),
            },
        );
        Ok(token)
    }

    async fn record_reauth(&self, id: &str) -> Result<(), XaiOAuthError> {
        self.accounts
            .write()
            .await
            .get_mut(id)
            .ok_or(XaiOAuthError::AccountChanged)?
            .requires_reauth = true;
        self.access_tokens.write().await.remove(id);
        self.save_to_disk().await
    }
}

#[cfg(test)]
mod tests;
