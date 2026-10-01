use crate::provider_proxy::{
    build_proxy_error, desktop, extract_request_insights,
    model_aliases::ModelAliases,
    optimizer::apply_proxy_optimizers,
    profiles::{is_claude_messages_path, rewrite_claude_request_target},
    transform_claude_request_body, ProxyRequestInsights, UpstreamTarget,
};
use crate::proxy_optimizer::OptimizerConfig;
use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, Response, StatusCode},
};
use bytes::Bytes;

pub(super) struct Input<'a> {
    pub tool: &'a str,
    pub desktop: bool,
    pub path: &'a str,
    pub query: &'a Option<String>,
    pub body: &'a Bytes,
    pub headers: &'a [(HeaderName, HeaderValue)],
    pub optimizer: &'a OptimizerConfig,
    pub upstream: &'a UpstreamTarget,
    pub snapshot: &'a str,
    pub aliases: &'a ModelAliases,
}

pub(super) struct Prepared {
    pub path: String,
    pub query: Option<String>,
    pub body: Bytes,
    pub extra_headers: Vec<(String, String)>,
    pub insights: ProxyRequestInsights,
}

pub(super) fn prepare(input: Input<'_>) -> Result<Prepared, Response<Body>> {
    let upstream = input.upstream;
    let profile_body = if input.desktop && is_claude_messages_path(input.path) {
        Bytes::from(
            desktop::rewrite_model(input.body, input.snapshot)
                .map_err(|error| build_proxy_error(StatusCode::BAD_REQUEST, error))?,
        )
    } else {
        input.body.clone()
    };
    let (path, query, body) = match upstream.claude_api_format {
        Some(format) if format.needs_transform() && is_claude_messages_path(input.path) => {
            let (path, query) = rewrite_claude_request_target(
                input.path,
                input.query.as_deref(),
                format,
                upstream.is_github_copilot,
                upstream.is_codex_oauth,
                Some(&profile_body),
            );
            let body =
                transform_claude_request_body(format, &profile_body, upstream.is_codex_oauth)
                    .map_err(|error| {
                        super::body::conversion_error_response(StatusCode::BAD_REQUEST, &error)
                    })?;
            (path, query, body)
        }
        _ => (input.path.into(), input.query.clone(), profile_body),
    };
    let body =
        super::body::apply_local_proxy_body_override(body, upstream.request_body_override.as_ref());
    let optimized = apply_proxy_optimizers(
        if input.desktop { "claude" } else { input.tool },
        upstream.is_codex_oauth,
        body,
        input.headers,
        input.optimizer,
    );
    let insights = extract_request_insights(input.tool, &path, &optimized.body);
    let (path, body, insights) = input
        .aliases
        .apply(path, optimized.body, insights)
        .map_err(|error| build_proxy_error(StatusCode::BAD_REQUEST, error))?;
    let body = super::responses_history::repair(&path, body);
    Ok(Prepared {
        path,
        query,
        body,
        extra_headers: optimized.extra_headers,
        insights,
    })
}
