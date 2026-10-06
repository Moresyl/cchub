use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::AppHandle;

use crate::provider_proxy::cost::{log_proxy_request, AccountingLease};
use crate::provider_proxy::{ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};

pub(super) struct StreamRecord<R: tauri::Runtime> {
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
    pub health: crate::provider_proxy::forward::streaming_health::StreamHealth,
    pub capture: super::UsageCapture,
    pub timing: super::StreamTimingCapture,
    pub accounting: Option<AccountingLease>,
}

impl<R: tauri::Runtime> StreamRecord<R> {
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

    fn submit(&mut self) -> Option<impl std::future::Future<Output = ()> + Send + 'static> {
        let accounting = self.accounting.take()?;
        self.usage = self.capture.snapshot();
        if self.health.failed() {
            self.fail("Upstream returned a streaming error".into());
        } else if self.health.delivered_successfully() {
            self.complete();
        }
        Some(log_proxy_request(
            &self.app_handle,
            &self.request_id,
            &self.tool_id,
            &self.upstream,
            &self.insights,
            Some(&self.usage),
            Some(&self.timing.snapshot()),
            self.started_at
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
            self.status_code,
            self.error_message.as_deref(),
            &accounting,
        ))
    }
}

pub(super) struct StreamRequestLog<R: tauri::Runtime> {
    record: Arc<Mutex<StreamRecord<R>>>,
    id: String,
    lifecycle: Arc<crate::provider_proxy::cost::Lifecycle>,
}

impl<R: tauri::Runtime> StreamRequestLog<R> {
    pub(super) fn new(record: StreamRecord<R>) -> Self {
        Self::new_in(record, crate::provider_proxy::cost::lifecycle().clone())
    }

    fn new_in(
        record: StreamRecord<R>,
        lifecycle: Arc<crate::provider_proxy::cost::Lifecycle>,
    ) -> Self {
        let id = record.request_id.clone();
        let record = Arc::new(Mutex::new(record));
        let weak = Arc::downgrade(&record);
        lifecycle.register(
            id.clone(),
            Box::new(move || {
                if let Some(record) = weak.upgrade() {
                    let mut record = record.lock().unwrap_or_else(|error| error.into_inner());
                    drop(record.submit());
                }
            }),
        );
        Self {
            record,
            id,
            lifecycle,
        }
    }

    pub(super) fn fail(&self, message: String) {
        self.record
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .fail(message);
    }

    pub(super) fn complete(&self) {
        self.record
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .complete();
    }

    pub(super) fn submit(&self) -> impl std::future::Future<Output = ()> + Send + 'static {
        let writing = self
            .record
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .submit();
        async move {
            if let Some(writing) = writing {
                writing.await;
            }
        }
    }
}

impl<R: tauri::Runtime> Drop for StreamRequestLog<R> {
    fn drop(&mut self) {
        drop(self.submit());
        self.lifecycle.remove(&self.id);
    }
}

#[cfg(test)]
#[path = "stream_log_tests.rs"]
mod tests;
