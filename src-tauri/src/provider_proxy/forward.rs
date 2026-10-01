// 把 proxy 上下文路由到对应的 upstream URL，按顺序尝试候选并落配额日志。
use axum::body::{to_bytes, Body};
use axum::http::{Request, Response, StatusCode};
use bytes::Bytes;
use serde_json::Value;
use std::time::Instant;
use tauri::{AppHandle, Manager};

use crate::db::DbState;
use crate::provider_proxy_transform::{openai_error_to_anthropic, rectify_anthropic_request_bytes};

use super::circuits::{
    profile_available, retry_after_seconds, track_body, CircuitLease, CircuitScope,
};
use super::cost::{
    extract_error_message_from_response, log_proxy_request, transform_claude_response_body,
};
use super::desktop;
#[path = "forward/body.rs"]
mod body;
#[path = "forward/context.rs"]
mod context;
#[path = "forward/streaming.rs"]
mod streaming;
#[path = "forward/streaming_errors.rs"]
mod streaming_errors;
#[path = "forward/streaming_health.rs"]
pub(super) mod streaming_health;
#[path = "forward/timeouts.rs"]
mod timeouts;
use super::optimizer::{apply_proxy_optimizers, read_optimizer_config, read_rectifier_config};
use super::profiles::{
    endpoint_circuit_key, is_claude_messages_path, ordered_upstream_base_urls, profile_circuit_key,
    read_profile_candidates_for_tool, rewrite_claude_request_target, route_succeeded,
    should_strip_claude_transform_header,
};
use super::usage::parse_usage_metrics_from_response;
use super::{
    build_forward_response_from_parts, build_json_response_from_value, build_proxy_error,
    build_upstream_request_url, extract_request_insights, extract_upstream_target,
    is_hop_by_hop_header, is_retryable_upstream_status, next_proxy_request_id, parse_json_bytes,
    reqwest_client, transform_claude_request_body, ClaudeApiFormat, LocalProviderProxyRuntime,
    MAX_PROXY_BODY_BYTES, MAX_PROXY_RESPONSE_BODY_BYTES,
};
use body::apply_local_proxy_body_override;
use timeouts::{read_response_body_limited, AttemptBudget};

pub(super) async fn forward_proxy_request<R: tauri::Runtime>(
    app_handle: AppHandle<R>,
    tool_id: String,
    relative_path: String,
    request: Request<Body>,
) -> Response<Body> {
    forward_proxy_request_with_client(app_handle, tool_id, relative_path, request, None).await
}

async fn forward_proxy_request_with_client<R: tauri::Runtime>(
    app_handle: AppHandle<R>,
    tool_id: String,
    relative_path: String,
    mut request: Request<Body>,
    client: Option<reqwest::Client>,
) -> Response<Body> {
    let is_desktop = tool_id == "claude-desktop";
    let proxy_url = match context::setup(&app_handle, &tool_id, &mut request) {
        Ok(proxy_url) => proxy_url,
        Err(response) => return response,
    };

    let request_query = request.uri().query().map(str::to_string);
    let profile_candidates = {
        let db = app_handle.state::<DbState>();
        let conn = match db.0.lock() {
            Ok(conn) => conn,
            Err(error) => {
                return build_proxy_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Database lock failed: {error}"),
                );
            }
        };

        let candidates = if is_desktop {
            desktop::profile_candidates(&conn)
        } else {
            read_profile_candidates_for_tool(&conn, &tool_id)
        };
        match candidates {
            Ok(value) => value,
            Err(error) => return build_proxy_error(StatusCode::BAD_GATEWAY, error),
        }
    };
    let method = request.method().clone();
    let client = match client
        .map(Ok)
        .unwrap_or_else(|| reqwest_client(proxy_url.as_deref()))
    {
        Ok(client) => client,
        Err(error) => return build_proxy_error(StatusCode::BAD_GATEWAY, error),
    };

    let original_relative_path = relative_path;
    let original_headers: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> = request
        .headers()
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    let body_bytes = match to_bytes(request.into_body(), MAX_PROXY_BODY_BYTES).await {
        Ok(body) => body,
        Err(error) => {
            return build_proxy_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("Failed to read request body: {error}"),
            )
        }
    };

    let (mut profile_candidates, routed, routing) = if is_desktop {
        (profile_candidates, false, None)
    } else {
        match super::routing::apply(
            &app_handle,
            &tool_id,
            &original_relative_path,
            &body_bytes,
            profile_candidates,
        ) {
            Ok(plan) => plan,
            Err(error) => return build_proxy_error(StatusCode::BAD_GATEWAY, error),
        }
    };
    let request_id = next_proxy_request_id();
    let started_at = Instant::now();
    let mut last_error: Option<String> = None;
    let mut last_response: Option<body::RetainedReply> = None;
    let rectifier_config = read_rectifier_config(&app_handle);
    let optimizer_config = read_optimizer_config(&app_handle);
    if let Err(error) = optimizer_config.validate_timeouts() {
        return build_proxy_error(StatusCode::INTERNAL_SERVER_ERROR, error);
    }
    let affinity = super::affinity::prepare(
        &app_handle,
        &tool_id,
        &original_relative_path,
        &original_headers,
        &body_bytes,
        routing.as_ref(),
        &mut profile_candidates,
        &optimizer_config,
    );

    let runtime = app_handle.state::<LocalProviderProxyRuntime>().0.clone();
    let profile_budget = if optimizer_config.failover_enabled {
        1usize.saturating_add(optimizer_config.max_profile_retries as usize)
    } else {
        1
    };
    let profile_candidates: Vec<_> = profile_candidates
        .into_iter()
        .take(profile_budget)
        .collect();
    let profile_candidate_count = profile_candidates.len();
    let profile_ids: Vec<String> = profile_candidates
        .iter()
        .map(|candidate| candidate.profile_id.clone())
        .collect();

    'profiles: for (profile_index, candidate) in profile_candidates.into_iter().enumerate() {
        if !profile_available(
            &runtime,
            &profile_circuit_key(&tool_id, &candidate.profile_id),
        ) {
            continue;
        }
        let snapshot = match affinity
            .as_ref()
            .map(|affinity| affinity.snapshot(&candidate))
            .transpose()
        {
            Ok(snapshot) => snapshot.unwrap_or_else(|| candidate.snapshot.clone()),
            Err(error) => return build_proxy_error(StatusCode::CONFLICT, error),
        };
        let mut upstream = match extract_upstream_target(
            &app_handle,
            &tool_id,
            candidate.profile_id.clone(),
            candidate.profile_name.clone(),
            &snapshot,
        )
        .await
        {
            Ok(target) => target,
            Err(error) => {
                last_error = Some(error.clone());
                if profile_index + 1 < profile_candidate_count {
                    crate::utils::append_runtime_log(
                        "warn",
                        "provider_proxy",
                        &format!(
                            "Skipping unavailable provider [{tool_id}] {} ({}): {error}",
                            candidate.profile_name, candidate.profile_id
                        ),
                    );
                    continue;
                }
                return last_response
                    .map(|reply| reply.finish(&app_handle, &request_id, &tool_id, started_at))
                    .unwrap_or_else(|| build_proxy_error(StatusCode::BAD_GATEWAY, error));
            }
        };
        if let Some(affinity) = &affinity {
            if let Err(error) = affinity.attach(&candidate, &mut upstream) {
                return build_proxy_error(StatusCode::CONFLICT, error);
            }
        }

        let forwarded_headers: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> =
            original_headers
                // Header filtering depends on the selected upstream profile and transform mode.
                // Clone from the original request snapshot because the body has already been moved.
                .iter()
                .filter_map(|(name, value)| {
                    if is_hop_by_hop_header(name.as_str())
                        || should_strip_claude_transform_header(
                            name.as_str(),
                            upstream.claude_api_format,
                            &original_relative_path,
                        )
                    {
                        None
                    } else {
                        Some((name.clone(), value.clone()))
                    }
                })
                .collect();
        let has_accept_encoding_header = forwarded_headers
            .iter()
            .any(|(name, _)| name.as_str().eq_ignore_ascii_case("accept-encoding"));

        let profile_body_bytes = if is_desktop && is_claude_messages_path(&original_relative_path) {
            match desktop::rewrite_model(&body_bytes, &candidate.snapshot) {
                Ok(body) => Bytes::from(body),
                Err(error) => return build_proxy_error(StatusCode::BAD_REQUEST, error),
            }
        } else {
            body_bytes.clone()
        };

        let (effective_relative_path, effective_request_query, effective_body_bytes) =
            match upstream.claude_api_format {
                Some(api_format)
                    if api_format.needs_transform()
                        && is_claude_messages_path(&original_relative_path) =>
                {
                    let (rewritten_path, rewritten_query) = rewrite_claude_request_target(
                        &original_relative_path,
                        request_query.as_deref(),
                        api_format,
                        upstream.is_github_copilot,
                        upstream.is_codex_oauth,
                        Some(profile_body_bytes.as_ref()),
                    );
                    let transformed_body = match transform_claude_request_body(
                        api_format,
                        profile_body_bytes.as_ref(),
                        upstream.is_codex_oauth,
                    ) {
                        Ok(body) => body,
                        Err(error) => {
                            return body::conversion_error_response(StatusCode::BAD_REQUEST, &error)
                        }
                    };
                    (rewritten_path, rewritten_query, transformed_body)
                }
                _ => (
                    original_relative_path.clone(),
                    request_query.clone(),
                    profile_body_bytes.clone(),
                ),
            };

        let effective_body_bytes = apply_local_proxy_body_override(
            effective_body_bytes,
            upstream.request_body_override.as_ref(),
        );
        let optimizer_result = apply_proxy_optimizers(
            if is_desktop { "claude" } else { &tool_id },
            upstream.is_codex_oauth,
            effective_body_bytes,
            &original_headers,
            &optimizer_config,
        );
        let effective_body_bytes = optimizer_result.body;
        let optimizer_extra_headers = optimizer_result.extra_headers;

        let request_insights = extract_request_insights(
            &tool_id,
            &effective_relative_path,
            effective_body_bytes.as_ref(),
        );
        let log_attempt =
            |usage: Option<&super::ProxyUsageMetrics>, status: u16, error: Option<&str>| {
                log_proxy_request(
                    &app_handle,
                    &request_id,
                    &tool_id,
                    &upstream,
                    &request_insights,
                    usage,
                    started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                    status,
                    error,
                );
            };
        let ordered_base_urls = ordered_upstream_base_urls(&app_handle, &upstream);
        let attempt_count = ordered_base_urls.len();
        let Some(mut profile_lease) = CircuitLease::acquire(
            runtime.clone(),
            CircuitScope::Profile,
            profile_circuit_key(&tool_id, &upstream.profile_id),
            &optimizer_config,
        ) else {
            continue;
        };
        let mut endpoint_failed = false;

        'endpoints: for (index, base_url) in ordered_base_urls.iter().enumerate() {
            let Some(mut endpoint_lease) = CircuitLease::acquire(
                runtime.clone(),
                CircuitScope::Endpoint,
                endpoint_circuit_key(&upstream.profile_id, base_url),
                &optimizer_config,
            ) else {
                continue;
            };
            let mut request_body_bytes = effective_body_bytes.clone();
            let mut rectifier_attempts = 0usize;

            loop {
                let upstream_url = match build_upstream_request_url(
                    base_url,
                    &effective_relative_path,
                    effective_request_query.as_deref(),
                    upstream.use_full_url,
                ) {
                    Ok(url) => url,
                    Err(error) => return build_proxy_error(StatusCode::BAD_REQUEST, error),
                };
                let mut builder = client.request(method.clone(), upstream_url.clone());
                for (name, value) in &forwarded_headers {
                    builder = builder.header(name, value);
                }
                for (name, value) in &upstream.headers {
                    builder = builder.header(name, value);
                }
                for (name, value) in &upstream.request_header_overrides {
                    builder = builder.header(name, value);
                }
                for (name, value) in &optimizer_extra_headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                if !request_insights.is_streaming && !has_accept_encoding_header {
                    builder = builder.header(reqwest::header::ACCEPT_ENCODING, "gzip, deflate, br");
                }
                if !request_body_bytes.is_empty() {
                    builder = builder.body(request_body_bytes.clone());
                }

                let budget = AttemptBudget::new(&optimizer_config, request_insights.is_streaming);
                match timeouts::send(builder, budget).await {
                    Ok(response) => {
                        let status = response.status();
                        let is_retryable_status = is_retryable_upstream_status(status);
                        if is_retryable_status {
                            endpoint_lease.failure();
                            endpoint_failed = true;
                            if index + 1 < attempt_count
                                || profile_index + 1 < profile_candidate_count
                            {
                                // Preserve the last vendor reply if all later candidates become blocked.
                                match read_response_body_limited(
                                    response,
                                    MAX_PROXY_RESPONSE_BODY_BYTES,
                                    budget,
                                )
                                .await
                                {
                                    Ok((status, headers, bytes)) => {
                                        let error_message = parse_json_bytes(&bytes)
                                            .as_ref()
                                            .and_then(extract_error_message_from_response)
                                            .unwrap_or_else(|| {
                                                format!(
                                                    "Upstream returned HTTP {}",
                                                    status.as_u16()
                                                )
                                            });
                                        let response = body::failed_response(
                                            status,
                                            &headers,
                                            bytes,
                                            upstream.claude_api_format.filter(|format| {
                                                format.needs_transform()
                                                    && is_claude_messages_path(
                                                        &original_relative_path,
                                                    )
                                            }),
                                        );
                                        last_response = Some(body::RetainedReply {
                                            response,
                                            upstream: upstream.clone(),
                                            insights: request_insights.clone(),
                                            error_message,
                                            usage: None,
                                        });
                                    }
                                    Err(error) => last_error = Some(error),
                                }
                                continue 'endpoints;
                            }
                            profile_lease.failure();
                        }

                        let headers = response.headers().clone();
                        let content_type = headers
                            .get(reqwest::header::CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                            .map(|value| value.to_ascii_lowercase())
                            .unwrap_or_default();
                        let is_json_response = content_type.contains("application/json")
                            || content_type.contains("+json");
                        let is_stream_response = content_type.contains("text/event-stream")
                            || (request_insights.is_streaming && !is_json_response);
                        let claude_transform = upstream.claude_api_format.filter(|format| {
                            format.needs_transform()
                                && is_claude_messages_path(&original_relative_path)
                        });

                        if is_json_response && (!is_stream_response || !status.is_success()) {
                            match read_response_body_limited(
                                response,
                                MAX_PROXY_RESPONSE_BODY_BYTES,
                                budget,
                            )
                            .await
                            {
                                Ok((_response_status, headers, bytes)) => {
                                    let parsed = parse_json_bytes(&bytes);
                                    if status.is_success() && parsed.is_none() {
                                        endpoint_lease.failure();
                                        endpoint_failed = true;
                                        last_error =
                                            Some("Upstream returned invalid JSON".to_string());
                                        log_attempt(None, 502, last_error.as_deref());
                                        if claude_transform.is_some() && last_response.is_none() {
                                            last_response =
                                                Some(body::RetainedReply::conversion_failed(
                                                    upstream.clone(),
                                                    request_insights.clone(),
                                                    None,
                                                    "Upstream returned invalid JSON".into(),
                                                ));
                                        }
                                        continue 'endpoints;
                                    }
                                    if status.is_success()
                                        && parsed.as_ref().is_some_and(|value| {
                                            value.get("error").is_some_and(|error| !error.is_null())
                                                || value.get("type").and_then(Value::as_str)
                                                    == Some("error")
                                        })
                                    {
                                        endpoint_lease.failure();
                                        endpoint_failed = true;
                                        last_error = Some(parsed.as_ref().and_then(extract_error_message_from_response)
                                            .unwrap_or_else(|| "Upstream returned an error in a success response".to_string()));
                                        log_attempt(None, 502, last_error.as_deref());
                                        continue 'endpoints;
                                    }
                                    let upstream_error_message = parsed
                                        .as_ref()
                                        .and_then(extract_error_message_from_response);

                                    if status == StatusCode::BAD_REQUEST
                                        && rectifier_attempts < 2
                                        && matches!(
                                            upstream.claude_api_format,
                                            Some(ClaudeApiFormat::Anthropic)
                                        )
                                        && is_claude_messages_path(&original_relative_path)
                                    {
                                        match rectify_anthropic_request_bytes(
                                            request_body_bytes.as_ref(),
                                            upstream_error_message.as_deref(),
                                            &rectifier_config,
                                        ) {
                                            Ok(Some(rectified_body)) => {
                                                rectifier_attempts += 1;
                                                request_body_bytes = Bytes::from(rectified_body);
                                                crate::utils::append_runtime_log(
                                                    "info",
                                                    "provider_proxy",
                                                    &format!(
                                                        "Applied Claude request rectifier [{tool_id}] {} ({}) after upstream 400: {}",
                                                        upstream.profile_name,
                                                        upstream.profile_id,
                                                        upstream_error_message
                                                            .as_deref()
                                                            .unwrap_or("unknown error")
                                                    ),
                                                );
                                                continue;
                                            }
                                            Ok(None) => {}
                                            Err(error) => {
                                                crate::utils::append_runtime_log(
                                                    "warn",
                                                    "provider_proxy",
                                                    &format!(
                                                        "Failed to apply Claude request rectifier [{tool_id}] {} ({}): {error}",
                                                        upstream.profile_name, upstream.profile_id
                                                    ),
                                                );
                                            }
                                        }
                                    }

                                    let transform_usage =
                                        parsed.as_ref().and_then(parse_usage_metrics_from_response);
                                    let transformed_body = match (claude_transform, parsed) {
                                        (Some(api_format), Some(parsed)) => {
                                            match transform_claude_response_body(
                                                api_format,
                                                status,
                                                parsed,
                                                request_insights.request_model.as_deref(),
                                            ) {
                                                Ok(value) => Some(value),
                                                Err(error) => {
                                                    let message = format!(
                                                        "Failed to transform upstream response for {} ({}/{}): {error}",
                                                        upstream.profile_name, tool_id, upstream.profile_id
                                                    );
                                                    log_attempt(
                                                        transform_usage.as_ref(),
                                                        StatusCode::BAD_GATEWAY.as_u16(),
                                                        Some(&message),
                                                    );
                                                    endpoint_lease.failure();
                                                    endpoint_failed = true;
                                                    if last_response.is_none() {
                                                        last_response = Some(
                                                            body::RetainedReply::conversion_failed(
                                                                upstream.clone(),
                                                                request_insights.clone(),
                                                                transform_usage.clone(),
                                                                message.clone(),
                                                            ),
                                                        );
                                                    }
                                                    last_error = Some(message);
                                                    continue 'endpoints;
                                                }
                                            }
                                        }
                                        (Some(_), None) if status.is_success() => {
                                            let message = format!(
                                                "Upstream returned a non-JSON success body for transformed Claude request: {} ({}/{})",
                                                upstream.profile_name, tool_id, upstream.profile_id
                                            );
                                            log_attempt(
                                                None,
                                                StatusCode::BAD_GATEWAY.as_u16(),
                                                Some(&message),
                                            );
                                            endpoint_lease.failure();
                                            endpoint_failed = true;
                                            last_error = Some(message);
                                            continue 'endpoints;
                                        }
                                        (Some(_), None) => {
                                            Some(openai_error_to_anthropic(status.as_u16(), None))
                                        }
                                        (None, parsed) => parsed,
                                    };

                                    if status.is_success() {
                                        let endpoint_accepted = endpoint_lease.success();
                                        let profile_accepted = profile_lease.success();
                                        if endpoint_accepted && profile_accepted {
                                            route_succeeded(
                                                &app_handle,
                                                &tool_id,
                                                &upstream,
                                                base_url,
                                                profile_index > 0 && !routed,
                                            );
                                        }
                                    }
                                    let usage = transform_usage.or_else(|| {
                                        transformed_body
                                            .as_ref()
                                            .and_then(parse_usage_metrics_from_response)
                                    });
                                    let error_message = if status.is_success() {
                                        None
                                    } else {
                                        transformed_body
                                            .as_ref()
                                            .and_then(extract_error_message_from_response)
                                    };
                                    log_attempt(
                                        usage.as_ref(),
                                        status.as_u16(),
                                        error_message.as_deref(),
                                    );
                                    if let Some(mut transformed_body) = transformed_body {
                                        if is_desktop
                                            && status.is_success()
                                            && is_claude_messages_path(&original_relative_path)
                                        {
                                            desktop::restore_response_model(
                                                &mut transformed_body,
                                                &body_bytes,
                                            );
                                        }
                                        return build_json_response_from_value(
                                            status,
                                            &headers,
                                            &transformed_body,
                                        );
                                    }
                                    return build_forward_response_from_parts(
                                        status,
                                        &headers,
                                        Body::from(bytes),
                                    );
                                }
                                Err(error) => {
                                    let message = format!(
                                        "Failed to read upstream response body for {} ({}/{}): {error}",
                                        upstream.profile_name, tool_id, upstream.profile_id
                                    );
                                    log_attempt(
                                        None,
                                        StatusCode::BAD_GATEWAY.as_u16(),
                                        Some(&message),
                                    );
                                    endpoint_lease.failure();
                                    endpoint_failed = true;
                                    last_error = Some(message);
                                    continue 'endpoints;
                                }
                            }
                        }

                        if is_stream_response {
                            let path = original_relative_path.trim_matches('/');
                            let health = if is_claude_messages_path(path)
                                || path.ends_with("chat/completions")
                                || path.ends_with("responses")
                                || path.contains(":streamGenerateContent")
                            {
                                streaming_health::StreamHealth::requiring_completion()
                            } else {
                                streaming_health::StreamHealth::default()
                            };
                            let route_app = app_handle.clone();
                            let route_tool = tool_id.clone();
                            let route_target = upstream.clone();
                            let route_base = base_url.clone();
                            let on_success = move || {
                                route_succeeded(
                                    &route_app,
                                    &route_tool,
                                    &route_target,
                                    &route_base,
                                    profile_index > 0 && !routed,
                                )
                            };
                            let desktop_model = if is_desktop
                                && status.is_success()
                                && is_claude_messages_path(&original_relative_path)
                            {
                                parse_json_bytes(&body_bytes).and_then(|value| {
                                    value
                                        .get("model")
                                        .and_then(Value::as_str)
                                        .map(str::to_string)
                                })
                            } else {
                                None
                            };
                            let body = streaming::streaming_body(
                                response,
                                &original_relative_path,
                                claude_transform,
                                app_handle.clone(),
                                request_id.clone(),
                                tool_id.clone(),
                                upstream.clone(),
                                request_insights.clone(),
                                &optimizer_config,
                                is_desktop,
                                desktop_model,
                                health.clone(),
                                started_at,
                                budget.first,
                            )
                            .await;
                            let body = match body {
                                Ok(body) => body,
                                Err(error) => {
                                    log_attempt(None, 502, Some(&error));
                                    endpoint_lease.failure();
                                    endpoint_failed = true;
                                    last_error = Some(error);
                                    continue 'endpoints;
                                }
                            };
                            return build_forward_response_from_parts(
                                status,
                                &headers,
                                track_body(
                                    body,
                                    profile_lease,
                                    endpoint_lease,
                                    status.is_success(),
                                    health,
                                    on_success,
                                ),
                            );
                        }

                        let error_message = if status.is_success() {
                            None
                        } else {
                            Some(format!("Upstream returned HTTP {}", status.as_u16()))
                        };
                        match read_response_body_limited(
                            response,
                            MAX_PROXY_RESPONSE_BODY_BYTES,
                            budget,
                        )
                        .await
                        {
                            Ok((status, headers, bytes)) => {
                                log_attempt(None, status.as_u16(), error_message.as_deref());
                                if status.is_success() {
                                    let endpoint_accepted = endpoint_lease.success();
                                    let profile_accepted = profile_lease.success();
                                    if endpoint_accepted && profile_accepted {
                                        route_succeeded(
                                            &app_handle,
                                            &tool_id,
                                            &upstream,
                                            base_url,
                                            profile_index > 0 && !routed,
                                        );
                                    }
                                }
                                return build_forward_response_from_parts(
                                    status,
                                    &headers,
                                    Body::from(bytes),
                                );
                            }
                            Err(error) => {
                                let message = format!(
                                    "Failed to read upstream response body for {} ({}/{}): {error}",
                                    upstream.profile_name, tool_id, upstream.profile_id
                                );
                                log_attempt(None, StatusCode::BAD_GATEWAY.as_u16(), Some(&message));
                                endpoint_lease.failure();
                                endpoint_failed = true;
                                last_error = Some(message);
                                continue 'endpoints;
                            }
                        }
                    }
                    Err(error) => {
                        let message = format!(
                            "Upstream request failed for {} ({}/{} @ {}): {error}",
                            upstream.profile_name, tool_id, upstream.profile_id, base_url
                        );
                        last_error = Some(message.clone());
                        log_attempt(None, 502, Some(&message));
                        endpoint_lease.failure();
                        endpoint_failed = true;
                        if index + 1 < attempt_count {
                            continue 'endpoints;
                        }
                        profile_lease.failure();
                        if profile_index + 1 < profile_candidate_count {
                            crate::utils::append_runtime_log(
                                "warn",
                                "provider_proxy",
                                &format!(
                                    "Proxy request failed [{tool_id}] {} ({} @ {}). Trying next provider.",
                                    upstream.profile_name, upstream.profile_id, base_url
                                ),
                            );
                            continue 'profiles;
                        }

                        crate::utils::append_runtime_log("warn", "provider_proxy", &message);
                        return last_response
                            .map(|reply| {
                                reply.finish(&app_handle, &request_id, &tool_id, started_at)
                            })
                            .unwrap_or_else(|| {
                                build_proxy_error(StatusCode::BAD_GATEWAY, message)
                            });
                    }
                }
            }
        }
        if endpoint_failed {
            profile_lease.failure();
        }
    }

    if let Some(response) = last_response {
        return response.finish(&app_handle, &request_id, &tool_id, started_at);
    }
    let unavailable = last_error.is_none();
    let mut response = build_proxy_error(
        if last_error.is_some() {
            StatusCode::BAD_GATEWAY
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        last_error.unwrap_or_else(|| format!("No upstream provider available for {tool_id}")),
    );
    if unavailable {
        if let Ok(value) = axum::http::HeaderValue::from_str(
            &retry_after_seconds(&runtime, &tool_id, &profile_ids).to_string(),
        ) {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, value);
        }
    }
    response
}

#[cfg(test)]
#[path = "forward/tests.rs"]
mod tests;
