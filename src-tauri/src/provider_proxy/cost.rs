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
    value.trim().parse::<f64>().unwrap_or(0.0).max(0.0)
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
    let model_id = usage
        .response_model
        .as_deref()
        .or(insights.request_model.as_deref());
    let Some(pricing) = lookup_model_pricing(conn, model_id) else {
        return 0.0;
    };

    let regular_input_tokens = usage
        .input_tokens
        .saturating_sub(usage.cache_read_tokens)
        .saturating_sub(usage.cache_creation_tokens);
    let mut total_cost = 0.0;
    total_cost += regular_input_tokens as f64 * pricing.input_cost_per_million / 1_000_000.0;
    total_cost += usage.output_tokens as f64 * pricing.output_cost_per_million / 1_000_000.0;
    total_cost +=
        usage.cache_read_tokens as f64 * pricing.cache_read_cost_per_million / 1_000_000.0;
    total_cost +=
        usage.cache_creation_tokens as f64 * pricing.cache_write_cost_per_million / 1_000_000.0;

    total_cost * upstream.cost_multiplier
}

fn update_daily_proxy_usage_rollup(
    conn: &Connection,
    tool_id: &str,
    created_at: &str,
    usage: &ProxyUsageMetrics,
    latency_ms: u64,
    status_code: u16,
    total_cost_usd: f64,
) -> Result<(), String> {
    let day = created_at
        .get(..10)
        .ok_or_else(|| format!("Invalid proxy request timestamp: {created_at}"))?;
    conn.execute(
        "INSERT INTO proxy_usage_daily_rollups (
            day,
            tool_id,
            total_requests,
            success_requests,
            total_input_tokens,
            total_output_tokens,
            total_cache_read_tokens,
            total_cache_creation_tokens,
            total_cost_usd,
            avg_latency_ms,
            updated_at
        ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(day, tool_id) DO UPDATE SET
            total_requests = proxy_usage_daily_rollups.total_requests + 1,
            success_requests = proxy_usage_daily_rollups.success_requests + excluded.success_requests,
            total_input_tokens = proxy_usage_daily_rollups.total_input_tokens + excluded.total_input_tokens,
            total_output_tokens = proxy_usage_daily_rollups.total_output_tokens + excluded.total_output_tokens,
            total_cache_read_tokens = proxy_usage_daily_rollups.total_cache_read_tokens + excluded.total_cache_read_tokens,
            total_cache_creation_tokens = proxy_usage_daily_rollups.total_cache_creation_tokens + excluded.total_cache_creation_tokens,
            total_cost_usd = printf('%.6f', CAST(proxy_usage_daily_rollups.total_cost_usd AS REAL) + CAST(excluded.total_cost_usd AS REAL)),
            avg_latency_ms = (
                (proxy_usage_daily_rollups.avg_latency_ms * proxy_usage_daily_rollups.total_requests)
                + excluded.avg_latency_ms
            ) / (proxy_usage_daily_rollups.total_requests + 1),
            updated_at = excluded.updated_at",
        rusqlite::params![
            day,
            tool_id,
            if (200..300).contains(&(status_code as i32)) { 1i64 } else { 0i64 },
            usage.input_tokens as i64,
            usage.output_tokens as i64,
            usage.cache_read_tokens as i64,
            usage.cache_creation_tokens as i64,
            format!("{total_cost_usd:.6}"),
            latency_ms as f64,
            created_at,
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn log_proxy_request(
    app_handle: &AppHandle,
    request_id: &str,
    tool_id: &str,
    upstream: &UpstreamTarget,
    insights: &ProxyRequestInsights,
    usage: Option<&ProxyUsageMetrics>,
    latency_ms: u64,
    status_code: u16,
    error_message: Option<&str>,
) {
    let db = app_handle.state::<DbState>();
    let Ok(conn) = db.0.lock() else {
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            "Failed to acquire database lock while logging proxy request",
        );
        return;
    };

    let usage = usage.cloned().unwrap_or_default();
    let created_at = chrono::Utc::now().to_rfc3339();
    let total_cost_usd = calculate_proxy_total_cost(&conn, upstream, insights, &usage);
    if let Err(error) = conn.execute(
        "INSERT OR REPLACE INTO proxy_request_logs (
            request_id,
            tool_id,
            profile_id,
            provider_name,
            request_model,
            response_model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            total_cost_usd,
            latency_ms,
            status_code,
            is_streaming,
            error_message,
            created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        rusqlite::params![
            request_id,
            tool_id,
            &upstream.profile_id,
            &upstream.profile_name,
            insights.request_model.as_deref(),
            usage.response_model.as_deref(),
            usage.input_tokens as i64,
            usage.output_tokens as i64,
            usage.cache_read_tokens as i64,
            usage.cache_creation_tokens as i64,
            format!("{total_cost_usd:.6}"),
            latency_ms as i64,
            status_code as i64,
            insights.is_streaming as i64,
            error_message,
            &created_at,
        ],
    ) {
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            &format!("Failed to persist proxy usage log: {error}"),
        );
        return;
    }

    if let Err(error) = update_daily_proxy_usage_rollup(
        &conn,
        tool_id,
        &created_at,
        &usage,
        latency_ms,
        status_code,
        total_cost_usd,
    ) {
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            &format!("Failed to update proxy usage rollup: {error}"),
        );
    }
    let _ = app_handle.emit(
        "usage-log-recorded",
        serde_json::json!({
            "toolId": tool_id,
            "statusCode": status_code,
        }),
    );
}
