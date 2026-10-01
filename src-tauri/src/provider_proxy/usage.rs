// 解析单次响应/流式响应里的 input/output/cache token 用量，并落库为整体合计 + 当日 rollup。
use crate::shared::gemini_usage::GeminiUsage;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::Value;
use tauri::AppHandle;

use super::{ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};

pub(super) fn parse_usage_metrics_from_response(body: &Value) -> Option<ProxyUsageMetrics> {
    if body.get("usageMetadata").is_some() {
        return parse_gemini_usage(body, &mut GeminiUsage::default());
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

fn parse_gemini_usage(body: &Value, gemini: &mut GeminiUsage) -> Option<ProxyUsageMetrics> {
    if !gemini.observe(body.get("usageMetadata")?) {
        return None;
    }
    Some(ProxyUsageMetrics {
        response_model: body
            .get("modelVersion")
            .and_then(Value::as_str)
            .map(str::to_string),
        input_tokens: gemini.input.unwrap_or(0),
        output_tokens: gemini.output(),
        cache_read_tokens: gemini.cached.unwrap_or(0),
        cache_creation_tokens: 0,
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

#[cfg(test)]
pub(super) fn extract_stream_usage_metrics_from_event(body: &Value) -> Option<ProxyUsageMetrics> {
    extract_stream_usage_with_gemini(body, &mut GeminiUsage::default())
}

fn extract_stream_usage_with_gemini(
    body: &Value,
    gemini: &mut GeminiUsage,
) -> Option<ProxyUsageMetrics> {
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
        return parse_gemini_usage(body, gemini);
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
    gemini: &mut GeminiUsage,
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
        if let Some(metrics) = extract_stream_usage_with_gemini(&parsed, gemini) {
            merge_proxy_usage_metrics(usage, &metrics);
            changed = true;
        }
    }

    changed
}

#[path = "usage/capture.rs"]
mod capture;
#[path = "usage/stream_log.rs"]
mod stream_log;
pub(super) use capture::{capture_stream_usage, UsageCapture};

#[cfg(test)]
#[path = "usage/partial_tests.rs"]
mod partial_tests;

#[allow(clippy::too_many_arguments)]
pub(super) fn create_usage_tracking_stream<R: tauri::Runtime, S, E>(
    stream: S,
    app_handle: AppHandle<R>,
    request_id: String,
    tool_id: String,
    upstream: UpstreamTarget,
    insights: ProxyRequestInsights,
    upstream_status: u16,
    started_at: std::time::Instant,
    health: super::forward::streaming_health::StreamHealth,
    capture: UsageCapture,
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
        health: health.clone(),
        capture,
    };
    async_stream::stream! {
        let mut log = log;
        tokio::pin!(stream);
        loop {
            match stream.next().await {
                Some(Ok(bytes)) => {
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
        if health.failed() { log.fail("Upstream returned a streaming error".into()); }
        else { log.complete(); }
    }
}
