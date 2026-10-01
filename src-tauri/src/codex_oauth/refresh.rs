use super::*;
use std::future::Future;

impl CodexOAuthManager {
    pub async fn get_valid_token(
        &self,
        account_id: Option<&str>,
    ) -> Result<String, CodexOAuthError> {
        self.get_token_using(account_id, |token| async move {
            self.refresh_token(&token).await
        })
        .await
    }

    async fn get_token_using<F, Fut>(
        &self,
        account_id: Option<&str>,
        refresh: F,
    ) -> Result<String, CodexOAuthError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<TokenPayload, CodexOAuthError>>,
    {
        let id = self.resolve_account_id(account_id).await.ok_or_else(|| {
            CodexOAuthError::AccountNotFound("No OAuth account is available".into())
        })?;
        {
            let _guard = self.mutation_lock.lock().await;
            let accounts = self.accounts.read().await;
            let account = accounts
                .get(&id)
                .ok_or_else(|| CodexOAuthError::AccountNotFound(id.clone()))?;
            if account.requires_reauth {
                return Err(CodexOAuthError::ReauthRequired);
            }
            if let Some(token) = self
                .tokens
                .read()
                .await
                .get(&id)
                .filter(|token| token.usable())
            {
                return Ok(token.value.clone());
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
                .ok_or_else(|| CodexOAuthError::AccountNotFound(id.clone()))?;
            if account.requires_reauth {
                return Err(CodexOAuthError::ReauthRequired);
            }
            if let Some(token) = self
                .tokens
                .read()
                .await
                .get(&id)
                .filter(|token| token.usable())
            {
                return Ok(token.value.clone());
            }
            let token = account
                .refresh_token
                .clone()
                .or(keyring_get_if_needed(&account)?);
            let Some(token) = token.filter(|token| !token.trim().is_empty()) else {
                self.record_reauth(&id).await?;
                return Err(CodexOAuthError::ReauthRequired);
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
            return Err(CodexOAuthError::AccountChanged);
        }
        let refreshed = match result {
            Ok(tokens) => tokens,
            Err(CodexOAuthError::RefreshTokenInvalid) => {
                self.record_reauth(&id).await?;
                return Err(CodexOAuthError::ReauthRequired);
            }
            Err(error) => return Err(error),
        };
        if refreshed.access_token.trim().is_empty() {
            return Err(CodexOAuthError::Parse("OAuth access token is empty".into()));
        }
        if token_identity(&refreshed).0.is_some_and(|next| next != id) {
            return Err(CodexOAuthError::Parse(
                "Refreshed OAuth account identity does not match".into(),
            ));
        }
        if let Some(next) = refreshed
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
        let token = refreshed.access_token;
        self.tokens.write().await.insert(
            id,
            CachedToken {
                value: token.clone(),
                expires_at_ms: expires_at(refreshed.expires_in),
            },
        );
        Ok(token)
    }

    async fn record_reauth(&self, id: &str) -> Result<(), CodexOAuthError> {
        self.accounts
            .write()
            .await
            .get_mut(id)
            .ok_or(CodexOAuthError::AccountChanged)?
            .requires_reauth = true;
        self.tokens.write().await.remove(id);
        self.save_to_disk().await
    }
}

fn keyring_get_if_needed(account: &AccountData) -> Result<Option<String>, CodexOAuthError> {
    if account.refresh_token.is_some() {
        Ok(None)
    } else {
        keyring_get(&account.id)
    }
}

#[cfg(test)]
mod tests;
