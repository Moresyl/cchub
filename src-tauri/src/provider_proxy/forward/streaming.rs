use axum::body::Body;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use tauri::AppHandle;

use super::streaming_errors::{terminate, Protocol};
use super::streaming_health::{observe, observe_delivery, StreamHealth};
use super::timeouts::{prepare_raw_stream, Deadline, ResponseStream};
use crate::provider_proxy::desktop;
use crate::provider_proxy::usage::{
    capture_stream_usage, create_usage_tracking_stream, UsageCapture,
};
use crate::provider_proxy::{ClaudeApiFormat, ProxyRequestInsights, UpstreamTarget};
use crate::provider_proxy_transform::{
    create_anthropic_sse_stream, create_anthropic_sse_stream_from_gemini,
    create_anthropic_sse_stream_from_responses, normalize_sse_stream,
};
use crate::proxy_optimizer::OptimizerConfig;

fn boxed<S, E>(stream: S) -> ResponseStream
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    Box::pin(stream.map(|chunk| chunk.map_err(|error| std::io::Error::other(error.to_string()))))
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn streaming_body<R: tauri::Runtime>(
    response: reqwest::Response,
    relative_path: &str,
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
    first_deadline: Deadline,
) -> Result<Body, String> {
    let upstream_status = response.status().as_u16();
    let is_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"));
    let raw = prepare_raw_stream(response, first_deadline, config.streaming_idle_timeout).await?;
    let source = if transform.is_some() || is_sse {
        boxed(normalize_sse_stream(raw))
    } else {
        raw
    };
    let capture = UsageCapture::default();
    let observed = capture_stream_usage(observe(source, health.clone()), capture.clone());
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
        _ => match Protocol::for_path(relative_path) {
            Some(protocol) => boxed(terminate(observed, protocol, upstream.profile_name.clone())),
            None => boxed(observed),
        },
    };
    let body = Body::from_stream(create_usage_tracking_stream(
        observe_delivery(stream, health.clone()),
        app_handle,
        request_id,
        tool_id,
        upstream,
        insights,
        upstream_status,
        started_at,
        health,
        capture,
    ));
    if is_desktop {
        Ok(Body::from_stream(desktop::restore_stream_model(
            body.into_data_stream(),
            desktop_model,
        )))
    } else {
        Ok(body)
    }
}
