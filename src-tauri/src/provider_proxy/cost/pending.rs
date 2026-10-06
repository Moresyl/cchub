use tauri::AppHandle;

use super::{ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};

pub(super) struct PendingRecord {
    pub request_id: String,
    pub tool_id: String,
    pub upstream: UpstreamTarget,
    pub insights: ProxyRequestInsights,
    pub usage: ProxyUsageMetrics,
    pub timing: super::super::usage::StreamTiming,
    pub latency_ms: u64,
    pub status_code: u16,
    pub error_message: Option<String>,
    pub created_at: String,
}

impl PendingRecord {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        request_id: &str,
        tool_id: &str,
        upstream: &UpstreamTarget,
        insights: &ProxyRequestInsights,
        usage: Option<&ProxyUsageMetrics>,
        timing: Option<&super::super::usage::StreamTiming>,
        latency_ms: u64,
        status_code: u16,
        error_message: Option<&str>,
    ) -> Self {
        // Accounting needs identities and affinity, never request credentials
        // or a potentially large request body override.
        let target = UpstreamTarget {
            profile_id: upstream.profile_id.clone(),
            profile_name: upstream.profile_name.clone(),
            cost_multiplier: upstream.cost_multiplier,
            affinity: upstream.affinity.clone(),
            managed_principal: upstream.managed_principal.clone(),
            base_url: String::new(),
            use_full_url: false,
            candidate_base_urls: vec![],
            headers: vec![],
            request_header_overrides: vec![],
            request_body_override: None,
            claude_api_format: None,
            is_github_copilot: false,
            is_codex_oauth: false,
        };
        Self {
            request_id: request_id.into(),
            tool_id: tool_id.into(),
            upstream: target,
            insights: insights.clone(),
            usage: usage.cloned().unwrap_or_default(),
            timing: timing.copied().unwrap_or_default(),
            latency_ms,
            status_code,
            error_message: error_message.map(str::to_owned),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub(super) fn persist<R: tauri::Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        super::persist_record(app, self)
    }
}

#[cfg(test)]
#[path = "pending_tests.rs"]
mod tests;
