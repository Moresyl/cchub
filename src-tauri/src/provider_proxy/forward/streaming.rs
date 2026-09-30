use axum::body::Body;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use std::pin::Pin;
use tauri::AppHandle;

use super::streaming_health::{observe, StreamHealth};
use crate::provider_proxy::desktop;
use crate::provider_proxy::usage::create_usage_tracking_stream;
use crate::provider_proxy::{ClaudeApiFormat, ProxyRequestInsights, UpstreamTarget};
use crate::provider_proxy_transform::{
    create_anthropic_sse_stream, create_anthropic_sse_stream_from_gemini,
    create_anthropic_sse_stream_from_responses, normalize_sse_stream,
};
use crate::proxy_optimizer::OptimizerConfig;

type ResponseStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

fn boxed<S, E>(stream: S) -> ResponseStream
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    Box::pin(stream.map(|chunk| chunk.map_err(|error| std::io::Error::other(error.to_string()))))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn streaming_body<R: tauri::Runtime>(
    response: reqwest::Response,
    transform: Option<ClaudeApiFormat>,
    app_handle: AppHandle<R>,
    request_id: String,
    tool_id: String,
    upstream: UpstreamTarget,
    insights: ProxyRequestInsights,
    config: &OptimizerConfig,
    is_desktop: bool,
    desktop_model: Option<String>,
    health: StreamHealth,
    started_at: std::time::Instant,
) -> Body {
    let upstream_status = response.status().as_u16();
    let is_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"));
    let source = if transform.is_some() || is_sse {
        boxed(normalize_sse_stream(response.bytes_stream()))
    } else {
        boxed(response.bytes_stream())
    };
    let observed = observe(source, health.clone());
    let stream = match transform {
        Some(ClaudeApiFormat::OpenAiChat) => boxed(create_anthropic_sse_stream(observed)),
        Some(ClaudeApiFormat::OpenAiResponses) => {
            boxed(create_anthropic_sse_stream_from_responses(observed))
        }
        Some(ClaudeApiFormat::GeminiNative) => boxed(create_anthropic_sse_stream_from_gemini(
            observed,
            insights
                .request_model
                .clone()
                .unwrap_or_else(|| "gemini-3.6-flash".to_string()),
        )),
        _ => boxed(observed),
    };
    let body = Body::from_stream(create_usage_tracking_stream(
        stream,
        app_handle,
        request_id,
        tool_id,
        upstream,
        insights,
        config.streaming_first_byte_timeout,
        config.streaming_idle_timeout,
        upstream_status,
        started_at,
        health,
    ));
    if is_desktop {
        Body::from_stream(desktop::restore_stream_model(
            body.into_data_stream(),
            desktop_model,
        ))
    } else {
        body
    }
}
