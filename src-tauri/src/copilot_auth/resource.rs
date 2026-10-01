use super::*;
use crate::shared::oauth_request::{self, ResourceError, TokenLease, TokenProvider};
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CopilotResourceFailure {
    SignInRequired,
    SubscriptionUnavailable,
    RateLimited,
    Unavailable,
    InvalidResponse,
    Timeout,
}

#[derive(Debug, Serialize)]
pub struct CopilotAccountResources {
    pub account: GitHubAccount,
    pub fetched_at: String,
    pub usage: Option<CopilotUsage>,
    pub models: Option<Vec<CopilotModel>>,
    pub usage_error: Option<CopilotResourceFailure>,
    pub models_error: Option<CopilotResourceFailure>,
}

struct OwnedAccount<'a> {
    manager: &'a CopilotAuthManager,
    account: GitHubAccount,
    github_token: String,
}

impl CopilotAuthManager {
    async fn owned_account(
        &self,
        requested: Option<&str>,
        expected_revision: Option<&str>,
    ) -> Result<OwnedAccount<'_>, CopilotAuthError> {
        let _guard = self.mutation_lock.lock().await;
        let id =
            self.resolve_account_id(requested)
                .await
                .ok_or(if expected_revision.is_some() {
                    CopilotAuthError::AccountChanged
                } else {
                    CopilotAuthError::GitHubTokenInvalid
                })?;
        let accounts = self.accounts.read().await;
        let data = accounts.get(&id).ok_or(CopilotAuthError::AccountChanged)?;
        if expected_revision.is_some_and(|revision| revision != data.revision) {
            return Err(CopilotAuthError::AccountChanged);
        }
        Ok(OwnedAccount {
            manager: self,
            account: GitHubAccount::from(data),
            github_token: data.github_token.clone(),
        })
    }

    pub async fn fetch_usage(
        &self,
        account_id: Option<&str>,
    ) -> Result<CopilotUsage, CopilotAuthError> {
        let owner = self.owned_account(account_id, None).await?;
        owner.usage_at(COPILOT_USAGE_URL, query_deadline()).await
    }

    pub async fn fetch_models(
        &self,
        account_id: Option<&str>,
    ) -> Result<Vec<CopilotModel>, CopilotAuthError> {
        let owner = self.owned_account(account_id, None).await?;
        owner.models_at(COPILOT_MODELS_URL, query_deadline()).await
    }

    pub async fn account_resources(
        &self,
        account_id: &str,
        expected_revision: &str,
    ) -> Result<CopilotAccountResources, CopilotAuthError> {
        self.resources_at(
            account_id,
            expected_revision,
            COPILOT_USAGE_URL,
            COPILOT_MODELS_URL,
            query_deadline(),
        )
        .await
    }

    async fn resources_at(
        &self,
        account_id: &str,
        expected_revision: &str,
        usage_url: &str,
        models_url: &str,
        deadline: Instant,
    ) -> Result<CopilotAccountResources, CopilotAuthError> {
        // Both independent results belong to the same exact login, even after a
        // default change. A stale account-list revision never queries a new login.
        if account_id.trim().is_empty() || expected_revision.is_empty() {
            return Err(CopilotAuthError::AccountChanged);
        }
        let owner = self
            .owned_account(Some(account_id), Some(expected_revision))
            .await?;
        let (usage, models) = tokio::join!(
            owner.usage_at(usage_url, deadline),
            owner.models_at(models_url, deadline),
        );
        owner.validate_account().await?;
        let usage_error = usage.as_ref().err().map(CopilotResourceFailure::from);
        let models_error = models.as_ref().err().map(CopilotResourceFailure::from);
        Ok(CopilotAccountResources {
            account: owner.account,
            fetched_at: chrono::Utc::now().to_rfc3339(),
            usage: usage.ok(),
            models: models.ok(),
            usage_error,
            models_error,
        })
    }
}

fn query_deadline() -> Instant {
    Instant::now() + Duration::from_secs(45)
}

impl OwnedAccount<'_> {
    async fn validate_account(&self) -> Result<(), CopilotAuthError> {
        if self
            .manager
            .accounts
            .read()
            .await
            .get(&self.account.id)
            .is_none_or(|data| data.revision != self.account.revision)
        {
            return Err(CopilotAuthError::AccountChanged);
        }
        Ok(())
    }

    async fn usage_at(
        &self,
        url: &str,
        deadline: Instant,
    ) -> Result<CopilotUsage, CopilotAuthError> {
        let result = tokio::time::timeout_at(deadline, async {
            let response = self
                .manager
                .http_client
                .get(url)
                .header("Authorization", format!("token {}", self.github_token))
                .header("Content-Type", "application/json")
                .header("editor-version", COPILOT_EDITOR_VERSION)
                .header("editor-plugin-version", COPILOT_PLUGIN_VERSION)
                .header("x-github-api-version", COPILOT_API_VERSION)
                .send()
                .await;
            self.validate_account().await?;
            let response = response.map_err(|_| ResourceError::Transport)?;
            let value = oauth_request::read_json(response).await?;
            let usage: CopilotUsage =
                serde_json::from_value(value).map_err(|_| ResourceError::InvalidPayload)?;
            if usage.copilot_plan.trim().is_empty() {
                return Err(ResourceError::InvalidPayload.into());
            }
            Ok(usage)
        })
        .await
        .map_err(|_| CopilotAuthError::QueryTimeout)
        .and_then(|value| value);
        self.validate_account().await?;
        result
    }

    async fn models_at(
        &self,
        url: &str,
        deadline: Instant,
    ) -> Result<Vec<CopilotModel>, CopilotAuthError> {
        let result = tokio::time::timeout_at(deadline, async {
            let value = oauth_request::get_json(self, Some(&self.account.id), |lease| {
                let mut request = self.manager.http_client.get(url);
                for (name, value) in copilot_request_headers(&lease.token) {
                    request = request.header(name, value);
                }
                request
            })
            .await?;
            let payload: CopilotModelsResponse =
                serde_json::from_value(value).map_err(|_| ResourceError::InvalidPayload)?;
            if payload.data.iter().any(|model| model.id.trim().is_empty()) {
                return Err(ResourceError::InvalidPayload.into());
            }
            Ok(payload
                .data
                .into_iter()
                .filter(|model| model.model_picker_enabled)
                .map(|model| CopilotModel {
                    id: model.id,
                    name: model.name,
                    vendor: model.vendor,
                })
                .collect())
        })
        .await
        .map_err(|_| CopilotAuthError::QueryTimeout)
        .and_then(|value| value);
        self.validate_account().await?;
        result
    }

    async fn invalidate_token(&self, lease: &TokenLease) -> Result<(), CopilotAuthError> {
        let _guard = self.manager.mutation_lock.lock().await;
        self.validate(lease).await?;
        let mut tokens = self.manager.copilot_tokens.write().await;
        // A late 401 may not evict a newer token from the same login.
        if tokens
            .get(&lease.account_id)
            .is_some_and(|token| token.token == lease.token)
        {
            tokens.remove(&lease.account_id);
        }
        Ok(())
    }
}

impl TokenProvider for OwnedAccount<'_> {
    type Error = CopilotAuthError;

    async fn lease(&self, _: Option<&str>) -> Result<TokenLease, Self::Error> {
        self.manager
            .lease_for_revision(&self.account.id, &self.account.revision)
            .await
    }

    async fn validate(&self, lease: &TokenLease) -> Result<(), Self::Error> {
        if lease.account_id != self.account.id || lease.revision != self.account.revision {
            return Err(CopilotAuthError::AccountChanged);
        }
        self.validate_account().await
    }

    async fn recover(&self, lease: &TokenLease) -> Result<TokenLease, Self::Error> {
        self.invalidate_token(lease).await?;
        self.lease(Some(&self.account.id)).await
    }

    async fn expire(&self, lease: &TokenLease) -> Result<(), Self::Error> {
        self.invalidate_token(lease).await
    }
}

impl From<ResourceError> for CopilotAuthError {
    fn from(error: ResourceError) -> Self {
        match error {
            ResourceError::Transport => Self::Network("Copilot resource request failed".into()),
            ResourceError::Timeout => Self::QueryTimeout,
            ResourceError::Http(status) => Self::QueryHttp(status),
            ResourceError::InvalidPayload => {
                Self::Parse("Copilot API returned invalid JSON".into())
            }
            ResourceError::TooLarge => {
                Self::Parse("Copilot response exceeds the 2 MiB limit".into())
            }
        }
    }
}

impl From<&CopilotAuthError> for CopilotResourceFailure {
    fn from(error: &CopilotAuthError) -> Self {
        match error {
            CopilotAuthError::GitHubTokenInvalid | CopilotAuthError::QueryHttp(401) => {
                Self::SignInRequired
            }
            CopilotAuthError::NoCopilotSubscription | CopilotAuthError::QueryHttp(403) => {
                Self::SubscriptionUnavailable
            }
            CopilotAuthError::QueryHttp(429) => Self::RateLimited,
            CopilotAuthError::QueryTimeout => Self::Timeout,
            CopilotAuthError::Parse(_) => Self::InvalidResponse,
            _ => Self::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests;
