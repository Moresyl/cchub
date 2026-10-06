// 用量记账、定价解析、按日聚合：把转发完一次请求后该写的 DB 行集中起来。
use axum::http::StatusCode;
use rusqlite::Connection;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::DbState;
use crate::provider_proxy_transform::{
    openai_error_to_anthropic, openai_to_anthropic, responses_to_anthropic,
};

use super::{ClaudeApiFormat, ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};

mod attempts;
pub(crate) use attempts::StreamAttempt;
mod dispatch;
mod lifecycle;
mod shutdown;
mod worker;
pub(super) use dispatch::drain as drain_accounting;
pub(super) use dispatch::{reserve as reserve_accounting, AccountingLease};
pub(super) use lifecycle::lifecycle;
pub(super) use lifecycle::Lifecycle;
pub(crate) use shutdown::handle_exit as handle_accounting_exit;
mod pending;
use pending::PendingRecord;

pub(super) fn extract_error_message_from_response(body: &Value) -> Option<String> {
    body.get("error")
        .and_then(|value| value.get("message"))
        .and_then(|value| value.as_str())
        .or_else(|| body.get("message").and_then(|value| value.as_str()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn transform_claude_response_body(
    api_format: ClaudeApiFormat,
    status: StatusCode,
    body: Value,
    request_model: Option<&str>,
) -> Result<Value, String> {
    if status.is_success() {
        match api_format {
            ClaudeApiFormat::Anthropic => Ok(body),
            ClaudeApiFormat::OpenAiChat => openai_to_anthropic(body),
            ClaudeApiFormat::OpenAiResponses => responses_to_anthropic(body),
            ClaudeApiFormat::GeminiNative => {
                let model = request_model.unwrap_or("gemini-3.6-flash");
                crate::gemini_transform::gemini_to_anthropic(body, model)
            }
        }
    } else {
        Ok(openai_error_to_anthropic(status.as_u16(), Some(&body)))
    }
}

#[derive(Debug, Clone, Default)]
struct ModelPricingEntry {
    input_cost_per_million: f64,
    output_cost_per_million: f64,
    cache_read_cost_per_million: f64,
    cache_write_cost_per_million: f64,
}

fn normalize_model_pricing_id(model_id: &str) -> String {
    let trimmed = model_id.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return String::new();
    }

    let without_prefix = trimmed
        .strip_prefix("models/")
        .unwrap_or(&trimmed)
        .to_string();
    let preferred = without_prefix
        .rsplit('/')
        .next()
        .unwrap_or(without_prefix.as_str())
        .trim()
        .to_string();

    if preferred.is_empty() {
        without_prefix
    } else {
        preferred
    }
}

pub(super) fn parse_cost_text(value: &str) -> f64 {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
#[test]
fn cache_usage_pricing_ignores_nonfinite_negative_and_invalid_rates() {
    for value in ["NaN", "inf", "-inf", "1e999", "-1", "invalid", ""] {
        assert_eq!(parse_cost_text(value), 0.0);
    }
    assert_eq!(parse_cost_text(" 0.5 "), 0.5);
    assert_eq!(parse_cost_text("0"), 0.0);
}

fn lookup_model_pricing(conn: &Connection, model_id: Option<&str>) -> Option<ModelPricingEntry> {
    let model_id = model_id.map(str::trim).filter(|value| !value.is_empty())?;
    let normalized = normalize_model_pricing_id(model_id);

    conn.query_row(
        "SELECT
            input_cost_per_million,
            output_cost_per_million,
            cache_read_cost_per_million,
            cache_write_cost_per_million
         FROM model_pricing
         WHERE model_id = ?1 OR normalized_model_id = ?2
         ORDER BY CASE WHEN model_id = ?1 THEN 0 ELSE 1 END
         LIMIT 1",
        rusqlite::params![model_id, normalized],
        |row| {
            Ok(ModelPricingEntry {
                input_cost_per_million: parse_cost_text(&row.get::<_, String>(0)?),
                output_cost_per_million: parse_cost_text(&row.get::<_, String>(1)?),
                cache_read_cost_per_million: parse_cost_text(&row.get::<_, String>(2)?),
                cache_write_cost_per_million: parse_cost_text(&row.get::<_, String>(3)?),
            })
        },
    )
    .ok()
}

pub(super) fn calculate_proxy_total_cost(
    conn: &Connection,
    upstream: &UpstreamTarget,
    insights: &ProxyRequestInsights,
    usage: &ProxyUsageMetrics,
) -> f64 {
    let model_id = insights.pricing_model(usage.response_model.as_deref());
    let Some(pricing) = lookup_model_pricing(conn, model_id) else {
        return 0.0;
    };

    let regular_input_tokens = usage.input_basis.ordinary(
        usage.input_tokens,
        usage.cache_read_tokens,
        usage.cache_creation_tokens,
    );
    let mut total_cost = 0.0;
    total_cost += regular_input_tokens as f64 * pricing.input_cost_per_million / 1_000_000.0;
    total_cost += usage.output_tokens as f64 * pricing.output_cost_per_million / 1_000_000.0;
    total_cost +=
        usage.cache_read_tokens as f64 * pricing.cache_read_cost_per_million / 1_000_000.0;
    total_cost +=
        usage.cache_creation_tokens as f64 * pricing.cache_write_cost_per_million / 1_000_000.0;

    total_cost * upstream.cost_multiplier
}

#[path = "cost/accounting.rs"]
mod accounting;

#[allow(clippy::too_many_arguments)]
pub(super) fn log_proxy_request<R: tauri::Runtime>(
    app_handle: &AppHandle<R>,
    request_id: &str,
    tool_id: &str,
    upstream: &UpstreamTarget,
    insights: &ProxyRequestInsights,
    usage: Option<&ProxyUsageMetrics>,
    timing: Option<&super::usage::StreamTiming>,
    latency_ms: u64,
    status_code: u16,
    error_message: Option<&str>,
    lease: &AccountingLease,
) -> impl std::future::Future<Output = ()> + Send + 'static {
    let app = app_handle.clone();
    let record = PendingRecord::new(
        request_id,
        tool_id,
        upstream,
        insights,
        usage,
        timing,
        latency_ms,
        status_code,
        error_message,
    );
    let writing = dispatch::write(lease, move || {
        let result = record.persist(&app);
        if let Err(error) = &result {
            warn_accounting(error);
        }
        result
    });
    async move {
        if let Err(error) = writing.await {
            warn_accounting(&error);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn log_failed_stream_attempt<R: tauri::Runtime>(
    app_handle: &AppHandle<R>,
    request_id: &str,
    tool_id: &str,
    upstream: &UpstreamTarget,
    insights: &ProxyRequestInsights,
    usage: Option<&ProxyUsageMetrics>,
    latency_ms: u64,
    status_code: u16,
    error_message: &str,
    lease: &AccountingLease,
) -> impl std::future::Future<Output = Result<(), String>> + Send + 'static {
    let app = app_handle.clone();
    let has_usage = usage.is_some();
    let record = PendingRecord::new(
        request_id,
        tool_id,
        upstream,
        insights,
        usage,
        None,
        latency_ms,
        status_code,
        Some(error_message),
    );
    let writing = dispatch::write(lease, move || {
        let result: Result<(), String> = (|| {
            // Both steps survive cancellation of the request. A failed parent
            // write must never append usage to an earlier attempt's record.
            record.persist(&app)?;
            if has_usage {
                attempts::record_stream_attempt(
                    &app,
                    &record.request_id,
                    &record.upstream,
                    &record.insights,
                    record.status_code,
                    Some(&record.usage),
                )?;
            }
            Ok(())
        })();
        if let Err(error) = &result {
            warn_accounting(error);
        }
        result
    });
    async move {
        let result = writing.await;
        result.map_err(|error| {
            warn_accounting(&error);
            "Failed stream accounting could not be retained; no further provider was requested"
                .into()
        })
    }
}

pub(super) struct AttemptLog<'a, R: tauri::Runtime> {
    pub app: &'a AppHandle<R>,
    pub request_id: &'a str,
    pub tool_id: &'a str,
    pub upstream: &'a UpstreamTarget,
    pub insights: &'a ProxyRequestInsights,
    pub started_at: std::time::Instant,
    pub lease: &'a AccountingLease,
}

impl<R: tauri::Runtime> AttemptLog<'_, R> {
    pub(super) fn log(
        &self,
        usage: Option<&ProxyUsageMetrics>,
        status: u16,
        error: Option<&str>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        log_proxy_request(
            self.app,
            self.request_id,
            self.tool_id,
            self.upstream,
            self.insights,
            usage,
            None,
            self.started_at
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
            status,
            error,
            self.lease,
        )
    }
}

fn warn_accounting(error: &str) {
    crate::utils::append_runtime_log(
        "warn",
        "provider_proxy",
        &format!("Failed to persist proxy accounting: {error}"),
    );
}

fn persist_record<R: tauri::Runtime>(
    app_handle: &AppHandle<R>,
    pending: &PendingRecord,
) -> Result<(), String> {
    let db = app_handle.state::<DbState>();
    let conn =
        db.0.lock()
            .map_err(|_| "Could not acquire proxy accounting database")?;
    let record = accounting::RequestRecord {
        request_id: &pending.request_id,
        tool_id: &pending.tool_id,
        upstream: &pending.upstream,
        insights: &pending.insights,
        usage: &pending.usage,
        timing: pending.timing,
        latency_ms: pending.latency_ms,
        status_code: pending.status_code,
        error_message: pending.error_message.as_deref(),
        created_at: &pending.created_at,
        total_cost_usd: calculate_proxy_total_cost(
            &conn,
            &pending.upstream,
            &pending.insights,
            &pending.usage,
        ),
    };
    accounting::persist_request(&conn, &record)
        .map_err(|_| "Could not persist proxy accounting")?;
    if (200..300).contains(&pending.status_code) {
        super::affinity::commit(
            app_handle,
            &conn,
            &pending.tool_id,
            &pending.upstream,
            &pending.usage,
        );
    }
    drop(conn);
    let _ = app_handle.emit(
        "usage-log-recorded",
        serde_json::json!({ "toolId": pending.tool_id, "statusCode": pending.status_code }),
    );
    Ok(())
}
