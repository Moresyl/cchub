// 解析单次响应/流式响应里的 input/output/cache token 用量，并落库为整体合计 + 当日 rollup。
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tauri::AppHandle;

use super::{ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};

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

fn parse_partial_stream_usage(
    usage: &Value,
    response_model: Option<String>,
) -> Option<ProxyUsageMetrics> {
    let has_counter = [
        "input_tokens",
        "prompt_tokens",
        "output_tokens",
        "completion_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ]
    .iter()
    .any(|key| usage.get(key).and_then(Value::as_u64).is_some())
        || usage
            .pointer("/input_tokens_details/cached_tokens")
            .and_then(Value::as_u64)
            .is_some()
        || usage
            .pointer("/prompt_tokens_details/cached_tokens")
            .and_then(Value::as_u64)
            .is_some();
    if !has_counter {
        return None;
    }
    let mut usage = usage.as_object()?.clone();
    if !usage.contains_key("input_tokens") && !usage.contains_key("prompt_tokens") {
        usage.insert("input_tokens".into(), Value::from(0));
    }
    if !usage.contains_key("output_tokens") && !usage.contains_key("completion_tokens") {
        usage.insert("output_tokens".into(), Value::from(0));
    }
    parse_usage_metrics_from_response(
        &serde_json::json!({ "model": response_model, "usage": usage }),
    )
}

pub(super) fn extract_stream_usage_metrics_from_event(body: &Value) -> Option<ProxyUsageMetrics> {
    let response_model = body
        .pointer("/message/model")
        .or_else(|| body.pointer("/response/model"))
        .or_else(|| body.get("model"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    if let Some(usage) = body.pointer("/message/usage") {
        return parse_partial_stream_usage(usage, response_model);
    }

    if let Some(usage) = body.pointer("/response/usage") {
        return parse_partial_stream_usage(usage, response_model);
    }

    if let Some(usage) = body.get("usage") {
        return parse_partial_stream_usage(usage, response_model);
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

#[path = "usage/stream_log.rs"]
mod stream_log;

#[allow(clippy::too_many_arguments)]
pub(super) fn create_usage_tracking_stream<R: tauri::Runtime, S, E>(
    stream: S,
    app_handle: AppHandle<R>,
    request_id: String,
    tool_id: String,
    upstream: UpstreamTarget,
    insights: ProxyRequestInsights,
    first_byte_timeout_secs: u64,
    idle_timeout_secs: u64,
    upstream_status: u16,
    started_at: std::time::Instant,
    health: super::forward::streaming_health::StreamHealth,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    // Created outside the generator so an unpolled body also records cancellation.
    let log = stream_log::StreamRequestLog {
        app_handle,
        request_id,
        tool_id,
        upstream,
        insights,
        started_at,
        upstream_status,
        status_code: 499,
        error_message: Some("Client disconnected before the upstream response completed".into()),
        usage: ProxyUsageMetrics::default(),
    };
    async_stream::stream! {
        let mut log = log;
        let mut buffer = String::new();
        let mut is_first_chunk = true;
        tokio::pin!(stream);
        loop {
            let timeout_secs = if is_first_chunk { first_byte_timeout_secs } else { idle_timeout_secs };
            let next_chunk = if timeout_secs > 0 {
                match tokio::time::timeout(Duration::from_secs(timeout_secs), stream.next()).await {
                    Ok(chunk) => chunk,
                    Err(_) => {
                        let kind = if is_first_chunk { "first byte" } else { "idle" };
                        let msg = format!("Stream {kind} timeout after {timeout_secs}s");
                        log.fail(msg.clone());
                        yield Err(std::io::Error::new(std::io::ErrorKind::TimedOut, msg));
                        return;
                    }
                }
            } else { stream.next().await };
            match next_chunk {
                Some(Ok(bytes)) => {
                    is_first_chunk = false;
                    let normalized = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
                    scan_stream_usage_buffer(&mut buffer, &normalized, &mut log.usage);
                    if buffer.len() > 1024 * 1024 { buffer.clear(); }
                    if health.failed() { log.fail("Upstream returned a streaming error".into()); }
                    yield Ok(bytes);
                }
                Some(Err(error)) => {
                    log.fail(error.to_string());
                    yield Err(std::io::Error::other(error.to_string()));
                    return;
                }
                None => break,
            }
        }
        if !buffer.trim().is_empty() {
            scan_stream_usage_buffer(&mut buffer, "\n\n", &mut log.usage);
        }
        if health.failed() { log.fail("Upstream returned a streaming error".into()); }
        else { log.complete(); }
    }
}
