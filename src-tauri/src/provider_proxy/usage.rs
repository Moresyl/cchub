// 解析单次响应/流式响应里的 input/output/cache token 用量，并落库为整体合计 + 当日 rollup。
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::db::DbState;

use super::{
    calculate_proxy_total_cost, parse_cost_text, ProxyRequestInsights, ProxyUsageMetrics,
    UpstreamTarget,
};

pub(super) fn parse_usage_metrics_from_response(body: &Value) -> Option<ProxyUsageMetrics> {
    if let Some(usage) = body.get("usageMetadata") {
        let input_tokens = usage.get("promptTokenCount")?.as_u64()?;
        let total_tokens = usage.get("totalTokenCount")?.as_u64()?;
        return Some(ProxyUsageMetrics {
            response_model: body
                .get("modelVersion")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            input_tokens,
            output_tokens: total_tokens.saturating_sub(input_tokens),
            cache_read_tokens: usage
                .get("cachedContentTokenCount")
                .and_then(|value| value.as_u64())
                .unwrap_or(0),
            cache_creation_tokens: 0,
        });
    }

    let usage = body.get("usage")?;
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|value| value.as_u64())?;
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|value| value.as_u64())?;
    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|value| value.as_u64())
        .or_else(|| {
            usage
                .get("input_tokens_details")
                .and_then(|value| value.get("cached_tokens"))
                .and_then(|value| value.as_u64())
        })
        .or_else(|| {
            usage
                .get("prompt_tokens_details")
                .and_then(|value| value.get("cached_tokens"))
                .and_then(|value| value.as_u64())
        })
        .unwrap_or(0);

    Some(ProxyUsageMetrics {
        response_model: body
            .get("model")
            .or_else(|| body.get("modelVersion"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
    })
}

pub(super) fn merge_proxy_usage_metrics(current: &mut ProxyUsageMetrics, next: &ProxyUsageMetrics) {
    if next
        .response_model
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        current.response_model = next.response_model.clone();
    }
    current.input_tokens = current.input_tokens.max(next.input_tokens);
    current.output_tokens = current.output_tokens.max(next.output_tokens);
    current.cache_read_tokens = current.cache_read_tokens.max(next.cache_read_tokens);
    current.cache_creation_tokens = current
        .cache_creation_tokens
        .max(next.cache_creation_tokens);
}

pub(super) fn extract_stream_usage_metrics_from_event(body: &Value) -> Option<ProxyUsageMetrics> {
    let response_model = body
        .pointer("/message/model")
        .or_else(|| body.pointer("/response/model"))
        .or_else(|| body.get("model"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    if let Some(usage) = body.pointer("/message/usage") {
        return parse_usage_metrics_from_response(&serde_json::json!({
            "model": response_model,
            "usage": usage,
        }));
    }

    if let Some(usage) = body.pointer("/response/usage") {
        return parse_usage_metrics_from_response(&serde_json::json!({
            "model": response_model,
            "usage": usage,
        }));
    }

    if let Some(usage) = body.get("usage") {
        return parse_usage_metrics_from_response(&serde_json::json!({
            "model": response_model,
            "usage": usage,
        }));
    }

    if body.get("usageMetadata").is_some() {
        return parse_usage_metrics_from_response(body);
    }

    if let Some(response) = body.get("response") {
        return parse_usage_metrics_from_response(response);
    }

    parse_usage_metrics_from_response(body)
}

pub(super) fn scan_stream_usage_buffer(
    buffer: &mut String,
    text: &str,
    usage: &mut ProxyUsageMetrics,
) -> bool {
    buffer.push_str(text);
    let mut changed = false;

    while let Some(pos) = buffer.find("\n\n") {
        let block = buffer[..pos].to_string();
        buffer.drain(..pos + 2);
        if block.trim().is_empty() {
            continue;
        }

        let mut data_parts = Vec::new();
        for line in block.lines() {
            if let Some(value) = line.strip_prefix("data:") {
                data_parts.push(value.trim_start().to_string());
            }
        }
        if data_parts.is_empty() {
            continue;
        }

        let payload = data_parts.join("\n");
        if payload.trim().is_empty() || payload.trim() == "[DONE]" {
            continue;
        }

        let Ok(parsed) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };
        if let Some(metrics) = extract_stream_usage_metrics_from_event(&parsed) {
            merge_proxy_usage_metrics(usage, &metrics);
            changed = true;
        }
    }

    changed
}

pub(super) fn finalize_stream_usage_log(
    app_handle: &AppHandle,
    request_id: &str,
    tool_id: &str,
    upstream: &UpstreamTarget,
    insights: &ProxyRequestInsights,
    usage: &ProxyUsageMetrics,
) {
    let db = app_handle.state::<DbState>();
    let Ok(conn) = db.0.lock() else {
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            "Failed to acquire database lock while finalizing proxy stream usage",
        );
        return;
    };

    let existing = conn.query_row(
        "SELECT
            response_model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            total_cost_usd,
            created_at
         FROM proxy_request_logs
         WHERE request_id = ?1",
        rusqlite::params![request_id],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, i64>(1)?.max(0) as u64,
                row.get::<_, i64>(2)?.max(0) as u64,
                row.get::<_, i64>(3)?.max(0) as u64,
                row.get::<_, i64>(4)?.max(0) as u64,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        },
    );

    let Ok((
        existing_response_model,
        existing_input_tokens,
        existing_output_tokens,
        existing_cache_read_tokens,
        existing_cache_creation_tokens,
        existing_total_cost_usd,
        created_at,
    )) = existing
    else {
        return;
    };

    let previous_response_model = existing_response_model.clone();
    let mut merged_usage = ProxyUsageMetrics {
        response_model: existing_response_model,
        input_tokens: existing_input_tokens,
        output_tokens: existing_output_tokens,
        cache_read_tokens: existing_cache_read_tokens,
        cache_creation_tokens: existing_cache_creation_tokens,
    };
    merge_proxy_usage_metrics(&mut merged_usage, usage);

    let existing_total_cost = parse_cost_text(&existing_total_cost_usd);
    let next_total_cost = calculate_proxy_total_cost(&conn, upstream, insights, &merged_usage);
    let delta_usage = ProxyUsageMetrics {
        response_model: None,
        input_tokens: merged_usage
            .input_tokens
            .saturating_sub(existing_input_tokens),
        output_tokens: merged_usage
            .output_tokens
            .saturating_sub(existing_output_tokens),
        cache_read_tokens: merged_usage
            .cache_read_tokens
            .saturating_sub(existing_cache_read_tokens),
        cache_creation_tokens: merged_usage
            .cache_creation_tokens
            .saturating_sub(existing_cache_creation_tokens),
    };
    let delta_cost = (next_total_cost - existing_total_cost).max(0.0);

    let has_usage_delta = delta_usage.input_tokens > 0
        || delta_usage.output_tokens > 0
        || delta_usage.cache_read_tokens > 0
        || delta_usage.cache_creation_tokens > 0
        || delta_cost > 0.0;
    let response_model_changed =
        merged_usage.response_model.as_deref() != previous_response_model.as_deref();
    if !has_usage_delta && !response_model_changed {
        return;
    }

    if let Err(error) = conn.execute(
        "UPDATE proxy_request_logs
         SET response_model = ?2,
             input_tokens = ?3,
             output_tokens = ?4,
             cache_read_tokens = ?5,
             cache_creation_tokens = ?6,
             total_cost_usd = ?7
         WHERE request_id = ?1",
        rusqlite::params![
            request_id,
            merged_usage.response_model.as_deref(),
            merged_usage.input_tokens as i64,
            merged_usage.output_tokens as i64,
            merged_usage.cache_read_tokens as i64,
            merged_usage.cache_creation_tokens as i64,
            format!("{next_total_cost:.6}"),
        ],
    ) {
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            &format!("Failed to finalize proxy stream usage log: {error}"),
        );
        return;
    }

    if !has_usage_delta {
        return;
    }

    if let Err(error) = conn.execute(
        "UPDATE proxy_usage_daily_rollups
         SET total_input_tokens = total_input_tokens + ?3,
             total_output_tokens = total_output_tokens + ?4,
             total_cache_read_tokens = total_cache_read_tokens + ?5,
             total_cache_creation_tokens = total_cache_creation_tokens + ?6,
             total_cost_usd = printf('%.6f', CAST(total_cost_usd AS REAL) + ?7),
             updated_at = ?8
         WHERE day = ?1 AND tool_id = ?2",
        rusqlite::params![
            created_at.get(..10).unwrap_or_default(),
            tool_id,
            delta_usage.input_tokens as i64,
            delta_usage.output_tokens as i64,
            delta_usage.cache_read_tokens as i64,
            delta_usage.cache_creation_tokens as i64,
            delta_cost,
            chrono::Utc::now().to_rfc3339(),
        ],
    ) {
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            &format!("Failed to finalize proxy usage rollup: {error}"),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create_usage_tracking_stream<S, E>(
    stream: S,
    app_handle: AppHandle,
    request_id: String,
    tool_id: String,
    upstream: UpstreamTarget,
    insights: ProxyRequestInsights,
    first_byte_timeout_secs: u64,
    idle_timeout_secs: u64,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    async_stream::stream! {
        let mut buffer = String::new();
        let mut merged_usage = ProxyUsageMetrics::default();
        let mut saw_usage = false;
        let mut is_first_chunk = true;

        tokio::pin!(stream);
        loop {
            let timeout_secs = if is_first_chunk { first_byte_timeout_secs } else { idle_timeout_secs };
            let next_chunk = if timeout_secs > 0 {
                match tokio::time::timeout(
                    Duration::from_secs(timeout_secs),
                    stream.next(),
                ).await {
                    Ok(chunk) => chunk,
                    Err(_) => {
                        let kind = if is_first_chunk { "first byte" } else { "idle" };
                        let msg = format!("Stream {kind} timeout after {timeout_secs}s");
                        crate::utils::append_runtime_log("warn", "provider_proxy", &msg);
                        yield Err(std::io::Error::new(std::io::ErrorKind::TimedOut, msg));
                        break;
                    }
                }
            } else {
                stream.next().await
            };

            match next_chunk {
                Some(Ok(bytes)) => {
                    is_first_chunk = false;
                    let normalized = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
                    if scan_stream_usage_buffer(&mut buffer, &normalized, &mut merged_usage) {
                        saw_usage = true;
                    }
                    yield Ok(bytes);
                }
                Some(Err(error)) => {
                    yield Err(std::io::Error::other(error.to_string()));
                    break;
                }
                None => break,
            }
        }

        if !buffer.trim().is_empty() && scan_stream_usage_buffer(&mut buffer, "\n\n", &mut merged_usage) {
            saw_usage = true;
        }

        if saw_usage {
            finalize_stream_usage_log(
                &app_handle,
                &request_id,
                &tool_id,
                &upstream,
                &insights,
                &merged_usage,
            );
        }
    }
}
