use axum::http::StatusCode;
use bytes::Bytes;

use crate::provider_proxy_transform::rectify_anthropic_request_bytes;
use crate::proxy_optimizer::config::RectifierConfig;

use super::super::{profiles::is_claude_messages_path, ClaudeApiFormat, UpstreamTarget};

pub(super) fn retry(
    status: StatusCode,
    path: &str,
    upstream: &UpstreamTarget,
    attempts: &mut usize,
    body: &mut Bytes,
    error: Option<&str>,
    config: &RectifierConfig,
) -> bool {
    if status != StatusCode::BAD_REQUEST
        || *attempts >= 2
        || upstream.claude_api_format != Some(ClaudeApiFormat::Anthropic)
        || !is_claude_messages_path(path)
    {
        return false;
    }
    match rectify_anthropic_request_bytes(body, error, config) {
        Ok(Some(rectified)) => {
            *attempts += 1;
            *body = Bytes::from(rectified);
            // Do not repeat the upstream's error, which can echo prompt contents.
            crate::utils::append_runtime_log(
                "info",
                "provider_proxy",
                "Applied Claude request rectifier after upstream 400",
            );
            true
        }
        Ok(None) => false,
        Err(_) => {
            crate::utils::append_runtime_log(
                "warn",
                "provider_proxy",
                "Could not apply Claude request rectifier",
            );
            false
        }
    }
}
