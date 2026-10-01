use crate::codex_oauth::CodexOAuthState;
use crate::copilot_auth::{self, CopilotAuthState};
use crate::shared::oauth_request::{TokenLease, TokenProvider};
use crate::xai_oauth::XaiOAuthState;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::profiles::{extract_bound_account_id, extract_copilot_account_id};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AuthProvider {
    Copilot,
    Codex,
    Xai,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ManagedPrincipal {
    pub(super) provider: AuthProvider,
    pub(super) account_id: String,
    pub(super) revision: String,
}

impl std::fmt::Debug for ManagedPrincipal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedPrincipal")
            .field("provider", &self.provider)
            .field("account_id", &"[redacted]")
            .field("revision", &self.revision)
            .finish()
    }
}

impl ManagedPrincipal {
    pub(super) fn if_current<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        action: impl FnOnce(),
    ) -> bool {
        match self.provider {
            AuthProvider::Codex => app.try_state::<CodexOAuthState>().is_some_and(|state| {
                state
                    .0
                    .with_account_revision(&self.account_id, &self.revision, action)
            }),
            AuthProvider::Xai => app.try_state::<XaiOAuthState>().is_some_and(|state| {
                state
                    .0
                    .with_account_revision(&self.account_id, &self.revision, action)
            }),
            AuthProvider::Copilot => app.try_state::<CopilotAuthState>().is_some_and(|state| {
                state
                    .0
                    .with_account_revision(&self.account_id, &self.revision, action)
            }),
        }
    }
}

pub(super) struct ManagedCredentials {
    provider: AuthProvider,
    lease: TokenLease,
}

impl ManagedCredentials {
    pub(super) fn into_parts(self) -> (Vec<(String, String)>, ManagedPrincipal) {
        let headers = match self.provider {
            AuthProvider::Copilot => copilot_auth::copilot_request_headers(&self.lease.token),
            AuthProvider::Codex => vec![
                (
                    "authorization".into(),
                    format!("Bearer {}", self.lease.token),
                ),
                ("chatgpt-account-id".into(), self.lease.account_id.clone()),
                ("originator".into(), "cchub".into()),
            ],
            AuthProvider::Xai => vec![(
                "authorization".into(),
                format!("Bearer {}", self.lease.token),
            )],
        };
        (
            headers,
            ManagedPrincipal {
                provider: self.provider,
                account_id: self.lease.account_id,
                revision: self.lease.revision,
            },
        )
    }
}

pub(super) async fn resolve<R: tauri::Runtime>(
    app: &AppHandle<R>,
    provider: Option<&str>,
    snapshot: &Value,
    profile_name: &str,
) -> Result<Option<ManagedCredentials>, String> {
    let unavailable =
        || format!("Managed authentication is unavailable for provider {profile_name}");
    let (provider, lease) = match provider {
        Some("codex_oauth") => {
            let account = extract_bound_account_id(snapshot, "codex_oauth");
            let manager = app.try_state::<CodexOAuthState>().ok_or_else(unavailable)?;
            (
                AuthProvider::Codex,
                manager.0.lease(account.as_deref()).await.map_err(|error| {
                    format!("Codex OAuth is not ready for provider {profile_name}: {error}")
                })?,
            )
        }
        Some("xai_oauth") => {
            let account = extract_bound_account_id(snapshot, "xai_oauth");
            let manager = app.try_state::<XaiOAuthState>().ok_or_else(unavailable)?;
            (
                AuthProvider::Xai,
                manager.0.lease(account.as_deref()).await.map_err(|error| {
                    format!("xAI OAuth is not ready for provider {profile_name}: {error}")
                })?,
            )
        }
        Some("github_copilot") => {
            let account = extract_copilot_account_id(snapshot);
            let manager = app
                .try_state::<CopilotAuthState>()
                .ok_or_else(unavailable)?;
            (
                AuthProvider::Copilot,
                manager.0.lease(account.as_deref()).await.map_err(|error| {
                    format!("GitHub Copilot auth is not ready for provider {profile_name}: {error}")
                })?,
            )
        }
        _ => return Ok(None),
    };
    Ok(Some(ManagedCredentials { provider, lease }))
}

#[cfg(test)]
mod tests;
