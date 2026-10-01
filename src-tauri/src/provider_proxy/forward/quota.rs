use crate::codex_oauth::CodexOAuthState;
use crate::copilot_auth::CopilotAuthState;
use crate::provider_proxy::{build_proxy_error, managed_auth::AuthProvider, UpstreamTarget};
use axum::{
    body::Body,
    http::{header::RETRY_AFTER, HeaderValue, Method, Response, StatusCode},
};
use tauri::{AppHandle, Manager};

pub(super) fn blocked<R: tauri::Runtime>(
    app: &AppHandle<R>,
    upstream: &UpstreamTarget,
    method: &Method,
    path: &str,
    model: Option<&str>,
) -> Option<u64> {
    let path = path.trim_matches('/');
    if method != Method::POST
        || !["messages", "chat/completions", "responses"]
            .iter()
            .any(|endpoint| path == *endpoint || path.ends_with(&format!("/{endpoint}")))
    {
        return None;
    }
    let owner = upstream.managed_principal.as_ref()?;
    match owner.provider {
        AuthProvider::Codex => app.try_state::<CodexOAuthState>()?.0.quota_blocked(
            &owner.account_id,
            &owner.revision,
            model,
        ),
        AuthProvider::Copilot => app.try_state::<CopilotAuthState>()?.0.quota_blocked(
            &owner.account_id,
            &owner.revision,
            model,
        ),
        AuthProvider::Xai => None,
    }
}

pub(super) struct Attempts {
    budget: usize,
    charged: usize,
    quota_skips: usize,
    retry_after: u64,
}

impl Attempts {
    pub(super) fn new(budget: usize) -> Self {
        Self {
            budget,
            charged: 0,
            quota_skips: 0,
            retry_after: 300,
        }
    }
    pub(super) fn reserve(&mut self) -> bool {
        if self.charged >= self.budget {
            return false;
        }
        self.charged += 1;
        true
    }
    pub(super) fn skip_quota(&mut self, retry_after: u64) {
        self.charged -= 1;
        self.quota_skips += 1;
        self.retry_after = self.retry_after.min(retry_after);
    }
    pub(super) fn can_continue(&self, index: usize, count: usize) -> bool {
        index + 1 < count && self.charged < self.budget
    }
    pub(super) fn exhausted_response(&self, count: usize) -> Option<Response<Body>> {
        if self.quota_skips == 0 || self.quota_skips != count {
            return None;
        }
        let mut response = build_proxy_error(StatusCode::TOO_MANY_REQUESTS,
            "The selected providers have no confirmed quota available. Refresh account usage or wait before retrying.".into());
        if let Ok(value) = HeaderValue::from_str(&self.retry_after.max(1).to_string()) {
            response.headers_mut().insert(RETRY_AFTER, value);
        }
        Some(response)
    }
}
