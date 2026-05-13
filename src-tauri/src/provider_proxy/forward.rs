// 把 proxy 上下文路由到对应的 upstream URL，按顺序尝试候选并落配额日志。
use axum::body::{to_bytes, Body};
use axum::http::{Request, Response, StatusCode};
use bytes::Bytes;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

use crate::copilot_auth;
use crate::db::DbState;
use crate::provider_proxy_transform::{
    anthropic_to_openai, anthropic_to_responses, create_anthropic_sse_stream,
    create_anthropic_sse_stream_from_gemini, create_anthropic_sse_stream_from_responses,
    openai_error_to_anthropic, rectify_anthropic_request_bytes,
};

use super::cost::{
    extract_error_message_from_response, log_proxy_request, transform_claude_response_body,
};
use super::optimizer::{apply_proxy_optimizers, read_optimizer_config, read_rectifier_config};
use super::profiles::{
    canonicalize_base_url, is_claude_messages_path, ordered_profile_candidates,
    ordered_upstream_base_urls, record_endpoint_failure, record_endpoint_success,
    record_profile_failure, record_profile_success, remember_preferred_upstream_base_url,
    rewrite_claude_request_target, should_strip_claude_transform_header,
};
use super::usage::{create_usage_tracking_stream, parse_usage_metrics_from_response};
use super::{
    build_forward_response, build_forward_response_from_parts, build_json_response_from_value,
    build_proxy_error, build_upstream_request_url, extract_request_insights,
    extract_upstream_target, is_hop_by_hop_header, is_retryable_upstream_status,
    next_proxy_request_id, parse_json_bytes, read_local_provider_proxy_settings_from_conn,
    reqwest_client, transform_claude_request_body, ClaudeApiFormat, LocalProviderProxySettings,
    ProxyUsageMetrics, LOCAL_PROVIDER_PROXY_SETTINGS_KEY, LOCAL_PROVIDER_PROXY_TOKEN,
    MAX_PROXY_BODY_BYTES,
};

#[allow(clippy::never_loop)]
pub(super) async fn forward_proxy_request(
    app_handle: AppHandle,
    tool_id: String,
    relative_path: String,
    request: Request<Body>,
) -> Response<Body> {
    let settings = {
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
        read_local_provider_proxy_settings_from_conn(&conn)
    };

    if !settings.enabled_apps.iter().any(|item| item == &tool_id) {
        return build_proxy_error(
            StatusCode::NOT_FOUND,
            format!("Local provider proxy is not enabled for {tool_id}"),
        );
    }

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

        match ordered_profile_candidates(&app_handle, &conn, &tool_id) {
            Ok(value) => value,
            Err(error) => return build_proxy_error(StatusCode::BAD_GATEWAY, error),
        }
    };
    let profile_candidate_count = profile_candidates.len();

    let method = request.method().clone();
    let client = match reqwest_client() {
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

    let request_id = next_proxy_request_id();
    let started_at = Instant::now();
    let mut last_error: Option<String> = None;
    let rectifier_config = read_rectifier_config(&app_handle);
    let optimizer_config = read_optimizer_config(&app_handle);

    let mut total_profile_retries: u32 = 0;

    'profiles: for (profile_index, candidate) in profile_candidates.into_iter().enumerate() {
        if profile_index > 0 {
            if !optimizer_config.failover_enabled {
                break;
            }
            total_profile_retries += 1;
            if total_profile_retries > optimizer_config.max_profile_retries {
                break;
            }
        }

        let upstream = match extract_upstream_target(
            &app_handle,
            &tool_id,
            candidate.profile_id.clone(),
            candidate.profile_name.clone(),
            &candidate.snapshot,
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
                return build_proxy_error(StatusCode::BAD_GATEWAY, error);
            }
        };

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
                        Some(body_bytes.as_ref()),
                    );
                    let transformed_body =
                        match transform_claude_request_body(api_format, body_bytes.as_ref()) {
                            Ok(body) => body,
                            Err(error) => return build_proxy_error(StatusCode::BAD_REQUEST, error),
                        };
                    (rewritten_path, rewritten_query, transformed_body)
                }
                _ => (
                    original_relative_path.clone(),
                    request_query.clone(),
                    body_bytes.clone(),
                ),
            };

        let optimizer_result = apply_proxy_optimizers(
            &tool_id,
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
        let ordered_base_urls = ordered_upstream_base_urls(&app_handle, &upstream);
        let attempt_count = ordered_base_urls.len();

        for (index, base_url) in ordered_base_urls.iter().enumerate() {
            let mut request_body_bytes = effective_body_bytes.clone();
            let mut rectifier_attempts = 0usize;

            loop {
                let upstream_url = build_upstream_request_url(
                    base_url,
                    &effective_relative_path,
                    effective_request_query.as_deref(),
                    upstream.use_full_url,
                );
                let mut builder = client.request(method.clone(), upstream_url.clone());
                for (name, value) in &forwarded_headers {
                    builder = builder.header(name, value);
                }
                for (name, value) in &upstream.headers {
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

                match builder.send().await {
                    Ok(response) => {
                        let status = response.status();
                        let is_retryable_status = is_retryable_upstream_status(status);
                        if is_retryable_status {
                            record_endpoint_failure(
                                &app_handle,
                                &tool_id,
                                &upstream,
                                base_url,
                                &optimizer_config,
                            );
                            record_profile_failure(
                                &app_handle,
                                &tool_id,
                                &upstream.profile_id,
                                &upstream.profile_name,
                                &optimizer_config,
                            );
                            if index + 1 < attempt_count {
                                crate::utils::append_runtime_log(
                                    "warn",
                                    "provider_proxy",
                                    &format!(
                                        "Proxy failover retry [{tool_id}] {} (profile {}) {} returned {}. Trying next endpoint.",
                                        upstream.profile_name, upstream.profile_id, base_url, status
                                    ),
                                );
                                continue;
                            }
                            if profile_index + 1 < profile_candidate_count {
                                crate::utils::append_runtime_log(
                                    "warn",
                                    "provider_proxy",
                                    &format!(
                                        "Proxy failover switching provider [{tool_id}] {} ({}) after {} from {}.",
                                        upstream.profile_name, upstream.profile_id, status, base_url
                                    ),
                                );
                                last_error = Some(format!(
                                    "Upstream returned retryable status {} for {} ({})",
                                    status, upstream.profile_name, upstream.profile_id
                                ));
                                continue 'profiles;
                            }
                        }

                        let latency_ms =
                            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                        if status.is_success() {
                            record_endpoint_success(
                                &app_handle,
                                &upstream,
                                base_url,
                                &optimizer_config,
                            );
                            record_profile_success(
                                &app_handle,
                                &tool_id,
                                &upstream.profile_id,
                                &optimizer_config,
                            );
                            remember_preferred_upstream_base_url(
                                &app_handle,
                                &upstream.profile_id,
                                &upstream.base_url,
                                base_url,
                            );
                            if canonicalize_base_url(base_url)
                                != canonicalize_base_url(&upstream.base_url)
                            {
                                crate::utils::append_runtime_log(
                                    "info",
                                    "provider_proxy",
                                    &format!(
                                        "Proxy failover promoted alternate endpoint [{tool_id}] {} -> {}",
                                        upstream.base_url, base_url
                                    ),
                                );
                            }
                            if profile_index > 0 {
                                let _ = app_handle.emit(
                                    "provider-failover",
                                    serde_json::json!({
                                        "tool_id": &tool_id,
                                        "profile_name": &upstream.profile_name,
                                        "profile_id": &upstream.profile_id,
                                    }),
                                );
                            }
                        }

                        let headers = response.headers().clone();
                        let content_type = headers
                            .get(reqwest::header::CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                            .map(|value| value.to_ascii_lowercase())
                            .unwrap_or_default();
                        let is_json_response = content_type.contains("application/json")
                            || content_type.contains("+json");
                        let is_stream_response = request_insights.is_streaming
                            || content_type.contains("text/event-stream");
                        let claude_transform = upstream.claude_api_format.filter(|format| {
                            format.needs_transform()
                                && is_claude_messages_path(&original_relative_path)
                        });

                        if is_json_response && (!is_stream_response || !status.is_success()) {
                            match response.bytes().await {
                                Ok(bytes) => {
                                    let parsed = parse_json_bytes(&bytes);
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
                                                    log_proxy_request(
                                                        &app_handle,
                                                        &request_id,
                                                        &tool_id,
                                                        &upstream,
                                                        &request_insights,
                                                        None,
                                                        latency_ms,
                                                        StatusCode::BAD_GATEWAY.as_u16(),
                                                        Some(&message),
                                                    );
                                                    return build_proxy_error(
                                                        StatusCode::BAD_GATEWAY,
                                                        message,
                                                    );
                                                }
                                            }
                                        }
                                        (Some(_), None) if status.is_success() => {
                                            let message = format!(
                                                "Upstream returned a non-JSON success body for transformed Claude request: {} ({}/{})",
                                                upstream.profile_name, tool_id, upstream.profile_id
                                            );
                                            log_proxy_request(
                                                &app_handle,
                                                &request_id,
                                                &tool_id,
                                                &upstream,
                                                &request_insights,
                                                None,
                                                latency_ms,
                                                StatusCode::BAD_GATEWAY.as_u16(),
                                                Some(&message),
                                            );
                                            return build_proxy_error(
                                                StatusCode::BAD_GATEWAY,
                                                message,
                                            );
                                        }
                                        (Some(_), None) => {
                                            Some(openai_error_to_anthropic(status.as_u16(), None))
                                        }
                                        (None, parsed) => parsed,
                                    };

                                    let usage = transformed_body
                                        .as_ref()
                                        .and_then(parse_usage_metrics_from_response);
                                    let error_message = if status.is_success() {
                                        None
                                    } else {
                                        transformed_body
                                            .as_ref()
                                            .and_then(extract_error_message_from_response)
                                    };
                                    log_proxy_request(
                                        &app_handle,
                                        &request_id,
                                        &tool_id,
                                        &upstream,
                                        &request_insights,
                                        usage.as_ref(),
                                        latency_ms,
                                        status.as_u16(),
                                        error_message.as_deref(),
                                    );
                                    if let Some(transformed_body) = transformed_body {
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
                                    log_proxy_request(
                                        &app_handle,
                                        &request_id,
                                        &tool_id,
                                        &upstream,
                                        &request_insights,
                                        None,
                                        latency_ms,
                                        StatusCode::BAD_GATEWAY.as_u16(),
                                        Some(&message),
                                    );
                                    return build_proxy_error(StatusCode::BAD_GATEWAY, message);
                                }
                            }
                        }

                        if is_stream_response {
                            log_proxy_request(
                                &app_handle,
                                &request_id,
                                &tool_id,
                                &upstream,
                                &request_insights,
                                None,
                                latency_ms,
                                status.as_u16(),
                                None,
                            );
                            if let Some(api_format) = claude_transform {
                                let body = match api_format {
                                    ClaudeApiFormat::OpenAiChat => {
                                        Body::from_stream(create_usage_tracking_stream(
                                            create_anthropic_sse_stream(response.bytes_stream()),
                                            app_handle.clone(),
                                            request_id.clone(),
                                            tool_id.clone(),
                                            upstream.clone(),
                                            request_insights.clone(),
                                            optimizer_config.streaming_first_byte_timeout,
                                            optimizer_config.streaming_idle_timeout,
                                        ))
                                    }
                                    ClaudeApiFormat::OpenAiResponses => {
                                        Body::from_stream(create_usage_tracking_stream(
                                            create_anthropic_sse_stream_from_responses(
                                                response.bytes_stream(),
                                            ),
                                            app_handle.clone(),
                                            request_id.clone(),
                                            tool_id.clone(),
                                            upstream.clone(),
                                            request_insights.clone(),
                                            optimizer_config.streaming_first_byte_timeout,
                                            optimizer_config.streaming_idle_timeout,
                                        ))
                                    }
                                    ClaudeApiFormat::GeminiNative => {
                                        let gemini_model = request_insights
                                            .request_model
                                            .clone()
                                            .unwrap_or_else(|| "gemini-2.5-flash".to_string());
                                        Body::from_stream(create_usage_tracking_stream(
                                            create_anthropic_sse_stream_from_gemini(
                                                response.bytes_stream(),
                                                gemini_model,
                                            ),
                                            app_handle.clone(),
                                            request_id.clone(),
                                            tool_id.clone(),
                                            upstream.clone(),
                                            request_insights.clone(),
                                            optimizer_config.streaming_first_byte_timeout,
                                            optimizer_config.streaming_idle_timeout,
                                        ))
                                    }
                                    ClaudeApiFormat::Anthropic => {
                                        Body::from_stream(create_usage_tracking_stream(
                                            response.bytes_stream(),
                                            app_handle.clone(),
                                            request_id.clone(),
                                            tool_id.clone(),
                                            upstream.clone(),
                                            request_insights.clone(),
                                            optimizer_config.streaming_first_byte_timeout,
                                            optimizer_config.streaming_idle_timeout,
                                        ))
                                    }
                                };
                                return build_forward_response_from_parts(status, &headers, body);
                            }
                            if content_type.contains("text/event-stream") {
                                let body = Body::from_stream(create_usage_tracking_stream(
                                    response.bytes_stream(),
                                    app_handle.clone(),
                                    request_id.clone(),
                                    tool_id.clone(),
                                    upstream.clone(),
                                    request_insights.clone(),
                                    optimizer_config.streaming_first_byte_timeout,
                                    optimizer_config.streaming_idle_timeout,
                                ));
                                return build_forward_response_from_parts(status, &headers, body);
                            }
                            return build_forward_response(response);
                        }

                        let error_message = if status.is_success() {
                            None
                        } else {
                            Some(format!("Upstream returned HTTP {}", status.as_u16()))
                        };
                        log_proxy_request(
                            &app_handle,
                            &request_id,
                            &tool_id,
                            &upstream,
                            &request_insights,
                            None,
                            latency_ms,
                            status.as_u16(),
                            error_message.as_deref(),
                        );
                        return build_forward_response(response);
                    }
                    Err(error) => {
                        let message = format!(
                            "Upstream request failed for {} ({}/{} @ {}): {error}",
                            upstream.profile_name, tool_id, upstream.profile_id, base_url
                        );
                        last_error = Some(message.clone());
                        record_endpoint_failure(
                            &app_handle,
                            &tool_id,
                            &upstream,
                            base_url,
                            &optimizer_config,
                        );
                        if index + 1 < attempt_count {
                            crate::utils::append_runtime_log(
                                "warn",
                                "provider_proxy",
                                &format!(
                                    "Proxy request failed [{tool_id}] {} @ {}: {error}. Trying next endpoint.",
                                    upstream.profile_name, base_url
                                ),
                            );
                            continue;
                        }

                        record_profile_failure(
                            &app_handle,
                            &tool_id,
                            &upstream.profile_id,
                            &upstream.profile_name,
                            &optimizer_config,
                        );
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
                        let latency_ms =
                            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                        log_proxy_request(
                            &app_handle,
                            &request_id,
                            &tool_id,
                            &upstream,
                            &request_insights,
                            None,
                            latency_ms,
                            StatusCode::BAD_GATEWAY.as_u16(),
                            Some(&message),
                        );
                        return build_proxy_error(StatusCode::BAD_GATEWAY, message);
                    }
                }
            }
        }
    }

    build_proxy_error(
        StatusCode::BAD_GATEWAY,
        last_error.unwrap_or_else(|| format!("No upstream provider available for {tool_id}")),
    )
}
