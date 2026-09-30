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

pub(super) struct RetainedReply {
    pub response: Response<Body>,
    pub upstream: crate::provider_proxy::UpstreamTarget,
    pub insights: crate::provider_proxy::ProxyRequestInsights,
    pub error_message: String,
}

impl RetainedReply {
    pub(super) fn finish<R: tauri::Runtime>(
        self,
        app: &tauri::AppHandle<R>,
        request_id: &str,
        tool_id: &str,
        started_at: std::time::Instant,
    ) -> Response<Body> {
        crate::provider_proxy::cost::log_proxy_request(
            app,
            request_id,
            tool_id,
            &self.upstream,
            &self.insights,
            None,
            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            self.response.status().as_u16(),
            Some(&self.error_message),
        );
        self.response
    }
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
