// 把 proxy 上下文路由到对应的 upstream URL，按顺序尝试候选并落配额日志。
use axum::body::{to_bytes, Body};
use axum::http::{Request, Response, StatusCode};
use serde_json::Value;
use std::time::Instant;
use tauri::{AppHandle, Manager};

use crate::db::DbState;
use crate::provider_proxy_transform::openai_error_to_anthropic;

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
#[path = "forward/model_request.rs"]
mod model_request;
#[path = "forward/quota.rs"]
mod quota;
#[path = "forward/rectifier.rs"]
mod rectifier;
#[path = "forward/responses_history.rs"]
mod responses_history;
#[path = "forward/streaming.rs"]
mod streaming;
#[path = "forward/streaming_chat.rs"]
mod streaming_chat;
#[path = "forward/streaming_errors.rs"]
mod streaming_errors;
#[path = "forward/streaming_health.rs"]
pub(super) mod streaming_health;
#[path = "forward/streaming_keepalive.rs"]
mod streaming_keepalive;
#[path = "forward/streaming_preflight.rs"]
mod streaming_preflight;
#[path = "forward/timeouts.rs"]
mod timeouts;
#[path = "forward/transport.rs"]
mod transport;
use super::optimizer::{read_optimizer_config, read_rectifier_config};
use super::profiles::{
    endpoint_circuit_key, is_claude_messages_path, ordered_upstream_base_urls, profile_circuit_key,
    read_profile_candidates_for_tool, route_succeeded, should_strip_claude_transform_header,
};
use super::usage::{parse_usage_metrics_with_basis, source_input_basis};
use super::{
    build_proxy_error, build_upstream_request_url, extract_upstream_target, is_hop_by_hop_header,
    is_retryable_upstream_status, next_proxy_request_id, parse_json_bytes, reqwest_client,
    LocalProviderProxyRuntime, MAX_PROXY_BODY_BYTES, MAX_PROXY_RESPONSE_BODY_BYTES,
};
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
    let quota_aware = routing
        .as_ref()
        .is_some_and(|(document, _)| document.policy.quota_aware);
    let profile_candidates: Vec<_> = profile_candidates
        .into_iter()
        .take(if quota_aware {
            usize::MAX
        } else {
            profile_budget
        })
        .collect();
    let profile_candidate_count = profile_candidates.len();
    let mut attempts = quota::Attempts::new(profile_budget);
    let profile_ids: Vec<String> = profile_candidates
        .iter()
        .map(|candidate| candidate.profile_id.clone())
        .collect();

    'profiles: for (profile_index, candidate) in profile_candidates.into_iter().enumerate() {
        if !attempts.reserve() {
            break;
        }
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
        let model_aliases = match super::model_aliases::ModelAliases::from_snapshot(&snapshot) {
            Ok(aliases) => aliases,
            Err(error) => return build_proxy_error(StatusCode::BAD_REQUEST, error),
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
                if attempts.can_continue(profile_index, profile_candidate_count) {
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

        let model_request::Prepared {
            path: effective_relative_path,
            query: effective_request_query,
            body: effective_body_bytes,
            extra_headers: optimizer_extra_headers,
            insights: request_insights,
        } = match model_request::prepare(model_request::Input {
            tool: &tool_id,
            desktop: is_desktop,
            path: &original_relative_path,
            query: &request_query,
            body: &body_bytes,
            headers: &original_headers,
            optimizer: &optimizer_config,
            upstream: &upstream,
            snapshot: &candidate.snapshot,
            aliases: &model_aliases,
        }) {
            Ok(prepared) => prepared,
            Err(response) => return response,
        };
        if quota_aware {
            if let Some(retry) = quota::blocked(
                &app_handle,
                &upstream,
                &method,
                &effective_relative_path,
                request_insights.sent_model(),
            ) {
                attempts.skip_quota(retry);
                continue;
            }
        }
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
            let mut rectifier_attempts = 0usize;
            let aliased_base_url = match super::model_aliases::alias_base_url(
                base_url,
                upstream.use_full_url,
                &effective_relative_path,
                &request_insights,
            ) {
                Ok(url) => url,
                Err(error) => return build_proxy_error(StatusCode::BAD_REQUEST, error),
            };
            let upstream_url = match build_upstream_request_url(
                &aliased_base_url,
                &effective_relative_path,
                effective_request_query.as_deref(),
                upstream.use_full_url,
            ) {
                Ok(url) => url,
                Err(error) => return build_proxy_error(StatusCode::BAD_REQUEST, error),
            };
            let mut history = super::chat_history::Recovery::new(
                runtime.clone(),
                &tool_id,
                &upstream,
                &snapshot,
                &effective_relative_path,
                &upstream_url,
                request_insights.sent_model(),
                method == axum::http::Method::POST,
            );
            let mut request_body_bytes = history.prepare(effective_body_bytes.clone());
            // Repairs share the endpoint's deadline, including reading its error
            // body. A validation loop cannot restart the timeout each time.
            let budget = AttemptBudget::new(&optimizer_config, request_insights.is_streaming);
            loop {
                let builder = transport::request(
                    &client,
                    &method,
                    &upstream_url,
                    &forwarded_headers,
                    &upstream,
                    &optimizer_extra_headers,
                    !request_insights.is_streaming && !has_accept_encoding_header,
                    &request_body_bytes,
                );
                match timeouts::send(builder, budget).await {
                    Ok(response) => {
                        let status = response.status();
                        let is_retryable_status = is_retryable_upstream_status(status);
                        if is_retryable_status {
                            endpoint_lease.failure();
                            endpoint_failed = true;
                            if index + 1 < attempt_count
                                || attempts.can_continue(profile_index, profile_candidate_count)
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

                        let inspect_history_error = history.can_retry(status)
                            && !content_type.contains("text/event-stream");
                        if (is_json_response || inspect_history_error)
                            && (!is_stream_response || !status.is_success())
                        {
                            match read_response_body_limited(
                                response,
                                MAX_PROXY_RESPONSE_BODY_BYTES,
                                budget,
                            )
                            .await
                            {
                                Ok((_response_status, headers, bytes)) => {
                                    let parsed = parse_json_bytes(&bytes);
                                    if let Some(message) = status
                                        .is_success()
                                        .then(|| {
                                            body::json_success_error(
                                                parsed.as_ref(),
                                                &bytes,
                                                claude_transform.is_some(),
                                            )
                                        })
                                        .flatten()
                                    {
                                        endpoint_lease.failure();
                                        endpoint_failed = true;
                                        last_error = Some(message.to_string());
                                        log_attempt(None, 502, last_error.as_deref());
                                        if claude_transform.is_some() && last_response.is_none() {
                                            last_response =
                                                Some(body::RetainedReply::conversion_failed(
                                                    upstream.clone(),
                                                    request_insights.clone(),
                                                    None,
                                                    message.into(),
                                                ));
                                        }
                                        continue 'endpoints;
                                    }
                                    let upstream_error_message = parsed
                                        .as_ref()
                                        .and_then(extract_error_message_from_response);

                                    if let Some(next) =
                                        history.retry(status, &bytes, &request_body_bytes)
                                    {
                                        request_body_bytes = next;
                                        continue;
                                    }
                                    if rectifier::retry(
                                        status,
                                        &original_relative_path,
                                        &upstream,
                                        &mut rectifier_attempts,
                                        &mut request_body_bytes,
                                        upstream_error_message.as_deref(),
                                        &rectifier_config,
                                    ) {
                                        continue;
                                    }

                                    let valid_chat_reply = parsed
                                        .as_ref()
                                        .is_some_and(super::chat_history::is_chat_reply);
                                    let transform_usage = parsed.as_ref().and_then(|body| {
                                        parse_usage_metrics_with_basis(
                                            body,
                                            source_input_basis(
                                                &original_relative_path,
                                                claude_transform,
                                            ),
                                        )
                                    });
                                    let transformed_body = match (claude_transform, parsed) {
                                        (Some(api_format), Some(parsed)) => {
                                            match transform_claude_response_body(
                                                api_format,
                                                status,
                                                parsed,
                                                request_insights.sent_model(),
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
                                        (None, parsed)
                                            if is_desktop
                                                && is_claude_messages_path(
                                                    &original_relative_path,
                                                ) =>
                                        {
                                            parsed
                                        }
                                        (None, _) => None,
                                    };

                                    if status.is_success() {
                                        if valid_chat_reply {
                                            history.success();
                                        }
                                        let endpoint_accepted = endpoint_lease.success();
                                        let profile_accepted = profile_lease.success();
                                        if endpoint_accepted && profile_accepted {
                                            route_succeeded(
                                                &app_handle,
                                                &tool_id,
                                                &upstream,
                                                base_url,
                                                profile_index > 0 && !routed && !quota_aware,
                                            );
                                        }
                                    }
                                    let usage = transform_usage.or_else(|| {
                                        transformed_body.as_ref().and_then(|body| {
                                            parse_usage_metrics_with_basis(
                                                body,
                                                source_input_basis(&original_relative_path, None),
                                            )
                                        })
                                    });
                                    let error_message = if status.is_success() {
                                        None
                                    } else {
                                        transformed_body
                                            .as_ref()
                                            .and_then(extract_error_message_from_response)
                                            .or(upstream_error_message)
                                    };
                                    log_attempt(
                                        usage.as_ref(),
                                        status.as_u16(),
                                        error_message.as_deref(),
                                    );
                                    return body::finish_json_response(
                                        status,
                                        &headers,
                                        bytes,
                                        transformed_body,
                                        &original_relative_path,
                                        is_desktop,
                                        &body_bytes,
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
                                history.success();
                                route_succeeded(
                                    &route_app,
                                    &route_tool,
                                    &route_target,
                                    &route_base,
                                    profile_index > 0 && !routed && !quota_aware,
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
                                    log_attempt(
                                        error.usage.as_ref(),
                                        error.status.as_u16(),
                                        Some(&error.message),
                                    );
                                    if let Err(message) = super::cost::record_stream_attempt(
                                        &app_handle,
                                        &request_id,
                                        &upstream,
                                        &request_insights,
                                        error.status.as_u16(),
                                        error.usage.as_ref(),
                                    ) {
                                        return build_proxy_error(
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            message,
                                        );
                                    }
                                    if !error.retryable() {
                                        return error.response(&original_relative_path);
                                    }
                                    endpoint_lease.failure();
                                    endpoint_failed = true;
                                    if error.usage.is_some() {
                                        last_response = Some(body::RetainedReply {
                                            response: error.response(&original_relative_path),
                                            upstream: upstream.clone(),
                                            insights: request_insights.clone(),
                                            error_message: error.message.clone(),
                                            usage: error.usage.clone(),
                                        });
                                    }
                                    last_error = Some(error.message);
                                    continue 'endpoints;
                                }
                            };
                            return body::stream_response(
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
                                            profile_index > 0 && !routed && !quota_aware,
                                        );
                                    }
                                }
                                return body::raw_response(
                                    status,
                                    &headers,
                                    bytes,
                                    &original_relative_path,
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
                        if attempts.can_continue(profile_index, profile_candidate_count) {
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
    if let Some(response) = attempts.exhausted_response(profile_candidate_count) {
        return response;
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
