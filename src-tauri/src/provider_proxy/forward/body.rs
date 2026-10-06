use crate::provider_proxy::upstream::{
    build_forward_response_from_parts, build_json_response_from_value, parse_json_bytes,
};
use crate::provider_proxy::ClaudeApiFormat;
use axum::{
    body::Body,
    http::{Response, StatusCode},
};
use bytes::Bytes;
use serde_json::Value;

#[path = "body/json_response.rs"]
mod json_response;
pub(super) use json_response::json_success_error;

pub(super) struct RetainedReply {
    pub response: Response<Body>,
    pub upstream: crate::provider_proxy::UpstreamTarget,
    pub insights: crate::provider_proxy::ProxyRequestInsights,
    pub error_message: String,
    pub usage: Option<crate::provider_proxy::ProxyUsageMetrics>,
}

pub(super) async fn finish_or_error<R: tauri::Runtime>(
    reply: Option<RetainedReply>,
    app: &tauri::AppHandle<R>,
    request_id: &str,
    tool_id: &str,
    started_at: std::time::Instant,
    error: String,
    lease: &crate::provider_proxy::cost::AccountingLease,
) -> Response<Body> {
    match reply {
        Some(reply) => {
            reply
                .finish(app, request_id, tool_id, started_at, lease)
                .await
        }
        None => crate::provider_proxy::build_proxy_error(StatusCode::BAD_GATEWAY, error),
    }
}

pub(super) fn exhausted_response(
    runtime: &std::sync::Arc<
        std::sync::Mutex<crate::provider_proxy::LocalProviderProxyRuntimeInner>,
    >,
    tool_id: &str,
    profile_ids: &[String],
    last_error: Option<String>,
) -> Response<Body> {
    let unavailable = last_error.is_none();
    let mut response = crate::provider_proxy::build_proxy_error(
        if unavailable {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::BAD_GATEWAY
        },
        last_error.unwrap_or_else(|| format!("No upstream provider available for {tool_id}")),
    );
    if unavailable {
        if let Ok(value) = axum::http::HeaderValue::from_str(
            &crate::provider_proxy::circuits::retry_after_seconds(runtime, tool_id, profile_ids)
                .to_string(),
        ) {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, value);
        }
    }
    response
}

impl RetainedReply {
    pub(super) fn conversion_failed(
        upstream: crate::provider_proxy::UpstreamTarget,
        insights: crate::provider_proxy::ProxyRequestInsights,
        usage: Option<crate::provider_proxy::ProxyUsageMetrics>,
        error_message: String,
    ) -> Self {
        Self {
            response: conversion_error_response(
                StatusCode::BAD_GATEWAY,
                "Upstream response could not be converted to Anthropic format",
            ),
            upstream,
            insights,
            usage,
            error_message,
        }
    }

    pub(super) async fn finish<R: tauri::Runtime>(
        self,
        app: &tauri::AppHandle<R>,
        request_id: &str,
        tool_id: &str,
        started_at: std::time::Instant,
        lease: &crate::provider_proxy::cost::AccountingLease,
    ) -> Response<Body> {
        crate::provider_proxy::cost::log_proxy_request(
            app,
            request_id,
            tool_id,
            &self.upstream,
            &self.insights,
            self.usage.as_ref(),
            None,
            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            self.response.status().as_u16(),
            Some(&self.error_message),
            lease,
        )
        .await;
        self.response
    }
}

pub(super) fn conversion_error_response(status: StatusCode, message: &str) -> Response<Body> {
    let kind = if status == StatusCode::BAD_REQUEST {
        "invalid_request_error"
    } else {
        "api_error"
    };
    build_json_response_from_value(
        status,
        &reqwest::header::HeaderMap::new(),
        &serde_json::json!({"type":"error","error":{"type":kind,"message":message}}),
    )
}

pub(super) fn failed_response(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    bytes: Bytes,
    transform: Option<ClaudeApiFormat>,
) -> Response<Body> {
    if transform.is_some() {
        let parsed = parse_json_bytes(&bytes);
        let error = crate::provider_proxy_transform::openai_error_to_anthropic(
            status.as_u16(),
            parsed.as_ref(),
        );
        let mut headers = headers.clone();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        build_json_response_from_value(status, &headers, &error)
    } else {
        build_forward_response_from_parts(status, headers, Body::from(bytes))
    }
}

pub(super) fn raw_response(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    bytes: Bytes,
    path: &str,
) -> Response<Body> {
    let is_json = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.split(';').next())
        .is_some_and(|mime| {
            let mime = mime.trim().to_ascii_lowercase();
            mime == "application/json" || mime.ends_with("+json")
        });
    let original = bytes.clone();
    let bytes = if status.is_success()
        && is_json
        && matches!(
            super::streaming_errors::Protocol::for_path(path),
            Some(super::streaming_errors::Protocol::Chat)
        ) {
        super::streaming_chat::repair_whole(bytes)
    } else {
        bytes
    };
    let mut headers = headers.clone();
    if bytes != original {
        invalidate_body_validators(&mut headers);
    }
    build_forward_response_from_parts(status, &headers, Body::from(bytes))
}

fn invalidate_body_validators(headers: &mut reqwest::header::HeaderMap) {
    for name in [
        "etag",
        "content-md5",
        "digest",
        "content-digest",
        "repr-digest",
    ] {
        headers.remove(name);
    }
}

pub(super) fn stream_response(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: Body,
) -> Response<Body> {
    let mut headers = headers.clone();
    // SSE decoding/repair/translation can change representation bytes. Do not
    // advertise integrity values for the original upstream representation.
    invalidate_body_validators(&mut headers);
    build_forward_response_from_parts(status, &headers, body)
}

pub(super) fn finish_json_response(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    bytes: Bytes,
    converted: Option<Value>,
    path: &str,
    is_desktop: bool,
    original_request: &[u8],
) -> Response<Body> {
    if let Some(mut converted) = converted {
        if is_desktop
            && status.is_success()
            && crate::provider_proxy::profiles::is_claude_messages_path(path)
        {
            crate::provider_proxy::desktop::restore_response_model(
                &mut converted,
                original_request,
            );
        }
        build_json_response_from_value(status, headers, &converted)
    } else {
        raw_response(status, headers, bytes, path)
    }
}

pub(super) fn apply_local_proxy_body_override(
    body: Bytes,
    override_value: Option<&Value>,
) -> Bytes {
    let Some(override_value) = override_value else {
        return body;
    };
    let Ok(mut target) = serde_json::from_slice::<Value>(&body) else {
        return body;
    };
    if !target.is_object() || !override_value.is_object() {
        return body;
    }
    merge_json_objects(&mut target, override_value);
    serde_json::to_vec(&target).map(Bytes::from).unwrap_or(body)
}

fn merge_json_objects(target: &mut Value, overrides: &Value) {
    let Some(target_object) = target.as_object_mut() else {
        *target = overrides.clone();
        return;
    };
    let Some(override_object) = overrides.as_object() else {
        *target = overrides.clone();
        return;
    };
    for (key, value) in override_object {
        if let Some(current) = target_object.get_mut(key) {
            if current.is_object() && value.is_object() {
                merge_json_objects(current, value);
                continue;
            }
        }
        target_object.insert(key.clone(), value.clone());
    }
}

#[cfg(test)]
#[path = "body/chat_tests.rs"]
mod chat_tests;

#[cfg(test)]
mod tests {
    use super::apply_local_proxy_body_override;
    use bytes::Bytes;
    use serde_json::json;

    #[test]
    fn local_proxy_body_override_deep_merges_json_without_stream() {
        let body =
            Bytes::from(r#"{"model":"demo","generationConfig":{"temperature":0.7},"stream":true}"#);
        let result = apply_local_proxy_body_override(
            body,
            Some(&json!({ "generationConfig": { "temperature": 0.2 }, "max_tokens": 64 })),
        );
        let value: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(value["model"], "demo");
        assert_eq!(value["generationConfig"]["temperature"], 0.2);
        assert_eq!(value["max_tokens"], 64);
        assert_eq!(value["stream"], true);
    }
}
