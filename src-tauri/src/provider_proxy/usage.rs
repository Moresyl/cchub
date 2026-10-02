// 解析单次响应/流式响应里的 input/output/cache token 用量，并落库为整体合计 + 当日 rollup。
use crate::shared::gemini_usage::GeminiUsage;
use crate::shared::token_usage::{InputTokenBasis, TokenUsage};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::Value;
use tauri::AppHandle;

use super::{ClaudeApiFormat, ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};

pub(super) fn source_input_basis(
    path: &str,
    transform: Option<ClaudeApiFormat>,
) -> InputTokenBasis {
    if matches!(
        transform,
        Some(
            ClaudeApiFormat::OpenAiChat
                | ClaudeApiFormat::OpenAiResponses
                | ClaudeApiFormat::GeminiNative
        )
    ) {
        return InputTokenBasis::IncludesCache;
    }
    let path = path.trim_matches('/');
    if path == "messages" || path.ends_with("/messages") {
        InputTokenBasis::ExcludesCache
    } else {
        InputTokenBasis::IncludesCache
    }
}

impl ProxyUsageMetrics {
    pub(super) fn total_input_tokens(&self) -> u64 {
        self.input_basis.total(
            self.input_tokens,
            self.cache_read_tokens,
            self.cache_creation_tokens,
        )
    }
}

#[cfg(test)]
pub(super) fn parse_usage_metrics_from_response(body: &Value) -> Option<ProxyUsageMetrics> {
    let basis = if body.get("type").and_then(Value::as_str) == Some("message") {
        InputTokenBasis::ExcludesCache
    } else {
        InputTokenBasis::IncludesCache
    };
    parse_usage_metrics_with_basis(body, basis)
}

pub(super) fn parse_usage_metrics_with_basis(
    body: &Value,
    basis: InputTokenBasis,
) -> Option<ProxyUsageMetrics> {
    if body.get("usageMetadata").is_some() {
        return parse_gemini_usage(body, &mut GeminiUsage::default());
    }

    let usage = body.get("usage")?;
    let counts = TokenUsage::parse(usage)?;

    Some(ProxyUsageMetrics {
        input_basis: basis,
        response_model: body
            .get("model")
            .or_else(|| body.get("modelVersion"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        input_tokens: counts.input.unwrap_or(0),
        output_tokens: counts.output.unwrap_or(0),
        cache_read_tokens: counts.cache_read.unwrap_or(0),
        cache_creation_tokens: counts.cache_write.unwrap_or(0),
    })
}

fn parse_gemini_usage(body: &Value, gemini: &mut GeminiUsage) -> Option<ProxyUsageMetrics> {
    if !gemini.observe(body.get("usageMetadata")?) {
        return None;
    }
    Some(ProxyUsageMetrics {
        input_basis: InputTokenBasis::IncludesCache,
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
    current.input_basis = next.input_basis;
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
    basis: InputTokenBasis,
) -> Option<ProxyUsageMetrics> {
    parse_usage_metrics_with_basis(
        &serde_json::json!({ "model": response_model, "usage": usage }),
        basis,
    )
}

#[cfg(test)]
pub(super) fn extract_stream_usage_metrics_from_event(body: &Value) -> Option<ProxyUsageMetrics> {
    extract_stream_usage_with_gemini(
        body,
        &mut GeminiUsage::default(),
        InputTokenBasis::IncludesCache,
    )
}

fn extract_stream_usage_with_gemini(
    body: &Value,
    gemini: &mut GeminiUsage,
    basis: InputTokenBasis,
) -> Option<ProxyUsageMetrics> {
    let response_model = body
        .pointer("/message/model")
        .or_else(|| body.pointer("/response/model"))
        .or_else(|| body.get("model"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    if let Some(usage) = body.pointer("/message/usage") {
        return parse_partial_stream_usage(usage, response_model, basis);
    }

    if let Some(usage) = body.pointer("/response/usage") {
        return parse_partial_stream_usage(usage, response_model, basis);
    }

    if let Some(usage) = body.get("usage") {
        return parse_partial_stream_usage(usage, response_model, basis);
    }

    if body.get("usageMetadata").is_some() {
        return parse_gemini_usage(body, gemini);
    }

    if let Some(response) = body.get("response") {
        return parse_usage_metrics_with_basis(response, basis);
    }

    parse_usage_metrics_with_basis(body, basis)
}

pub(super) fn scan_stream_usage_buffer(
    buffer: &mut String,
    text: &str,
    usage: &mut ProxyUsageMetrics,
    gemini: &mut GeminiUsage,
    basis: InputTokenBasis,
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
        if let Some(metrics) = extract_stream_usage_with_gemini(&parsed, gemini, basis) {
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
#[path = "usage/timing.rs"]
mod timing;
pub(super) use timing::{observe_stream_timing, StreamTiming, StreamTimingCapture};

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
    timing: StreamTimingCapture,
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
        timing,
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
