use std::time::Instant;
use tauri::AppHandle;

use crate::provider_proxy::cost::log_proxy_request;
use crate::provider_proxy::{ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};

pub(super) struct StreamRequestLog<R: tauri::Runtime> {
    pub app_handle: AppHandle<R>,
    pub request_id: String,
    pub tool_id: String,
    pub upstream: UpstreamTarget,
    pub insights: ProxyRequestInsights,
    pub started_at: Instant,
    pub upstream_status: u16,
    pub status_code: u16,
    pub error_message: Option<String>,
    pub usage: ProxyUsageMetrics,
}

impl<R: tauri::Runtime> StreamRequestLog<R> {
    pub(super) fn fail(&mut self, message: String) {
        self.status_code = if (200..300).contains(&self.upstream_status) {
            502
        } else {
            self.upstream_status
        };
        self.error_message = Some(message);
    }

    pub(super) fn complete(&mut self) {
        if self.status_code != 499 {
            return;
        }
        self.status_code = self.upstream_status;
        self.error_message = if (200..300).contains(&self.upstream_status) {
            None
        } else {
            Some(format!("Upstream returned HTTP {}", self.upstream_status))
        };
    }
}

impl<R: tauri::Runtime> Drop for StreamRequestLog<R> {
    fn drop(&mut self) {
        log_proxy_request(
            &self.app_handle,
            &self.request_id,
            &self.tool_id,
            &self.upstream,
            &self.insights,
            Some(&self.usage),
            self.started_at
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
            self.status_code,
            self.error_message.as_deref(),
        );
    }
}
