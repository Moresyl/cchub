// 把 active profile 翻译成 upstream URL/headers，处理 Copilot OAuth 与候选端点。
use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode};
use axum::response::IntoResponse;
use bytes::Bytes;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashSet;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::copilot_auth::{self, CopilotAuthState};
use crate::db::DbState;
use crate::provider_proxy_transform::{anthropic_to_openai, anthropic_to_responses};

use super::profiles::{
    active_profile_id_for_tool, canonicalize_base_url, default_base_url_for_claude,
    default_base_url_for_codex, default_base_url_for_gemini, extract_copilot_account_id,
    extract_cost_multiplier, extract_metadata_endpoint_candidates, extract_metadata_object,
    extract_provider_type, extract_use_full_url, filter_endpoint_candidates, profile_circuit_key,
    read_profile_candidates_for_tool, strip_beta_query,
};
use super::{
    ClaudeApiFormat, LocalProviderProxyRuntime, ProfileCandidate, ProxyRequestInsights,
    UpstreamTarget, LOCAL_PROVIDER_PROXY_TOKEN,
};

pub(super) fn is_retryable_upstream_status(status: StatusCode) -> bool {
    matches!(
        status.as_u16(),
        408 | 409 | 425 | 429 | 500 | 502 | 503 | 504
    )
}

pub(super) async fn extract_upstream_target(
    app_handle: &AppHandle,
    tool_id: &str,
    profile_id: String,
    profile_name: String,
    snapshot: &str,
) -> Result<UpstreamTarget, String> {
    let parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let metadata_candidates = extract_metadata_endpoint_candidates(&parsed);
    let provider_type = extract_provider_type(&parsed);
    let is_github_copilot = provider_type.as_deref() == Some("github_copilot");
    let cost_multiplier = extract_cost_multiplier(&parsed);
    let use_full_url = extract_use_full_url(&parsed);

    match tool_id {
        "claude" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let api_format = env
                .get("ANTHROPIC_API_FORMAT")
                .and_then(|value| value.as_str())
                .unwrap_or("anthropic");
            let base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| default_base_url_for_claude(api_format))
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define a Claude upstream base URL")
                })?;
            let headers = if is_github_copilot {
                let account_id = extract_copilot_account_id(&parsed);
                let manager = app_handle.state::<CopilotAuthState>().0.clone();
                let token = manager
                    .get_valid_token_for_account(account_id.as_deref())
                    .await
                    .map_err(|error| {
                        format!(
                            "GitHub Copilot auth is not ready for provider {profile_name}: {error}"
                        )
                    })?;
                copilot_auth::copilot_request_headers(&token)
            } else {
                let token = env
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .or_else(|| env.get("ANTHROPIC_API_KEY"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        format!("Provider {profile_name} does not define a Claude API token")
                    })?;

                if api_format == "anthropic" {
                    vec![
                        ("x-api-key".to_string(), token.to_string()),
                        ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ]
                } else {
                    vec![("authorization".to_string(), format!("Bearer {token}"))]
                }
            };
            let claude_api_format = ClaudeApiFormat::from_str(api_format);

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                claude_api_format: Some(claude_api_format),
                is_github_copilot,
                cost_multiplier,
            })
        }
        "codex" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let base_url =
                extract_toml_string(config, "base_url").unwrap_or_else(default_base_url_for_codex);
            let token = parsed
                .get("auth")
                .and_then(|value| value.get("OPENAI_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define an OPENAI_API_KEY")
                })?;

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        "gemini" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(default_base_url_for_gemini);
            let token = env
                .get("GEMINI_API_KEY")
                .or_else(|| env.get("GOOGLE_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define a Gemini API key")
                })?;

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        "openclaw" => {
            let protocol = parsed
                .get("api")
                .and_then(|value| value.as_str())
                .unwrap_or("openai-completions");
            let base_url = parsed
                .get("baseUrl")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define an OpenClaw baseUrl")
                })?
                .to_string();
            let token = parsed
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define an OpenClaw API key")
                })?;
            let headers = match protocol {
                "anthropic-messages" => vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ],
                "google-generative-ai" => vec![("x-goog-api-key".to_string(), token.to_string())],
                _ => vec![("authorization".to_string(), format!("Bearer {token}"))],
            };

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        "hermes" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let model = config
                .get("model")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let provider = model
                .get("provider")
                .and_then(|value| value.as_str())
                .unwrap_or("custom");
            let base_url = model
                .get("base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define a Hermes base_url")
                })?
                .to_string();
            let env_key = parsed
                .get("metadata")
                .and_then(|value| value.get("hermesApiKeyEnv"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    crate::hermes::providers::default_env_key_for_provider(provider)
                        .map(str::to_string)
                })
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define a Hermes API key env")
                })?;
            let token = env
                .get(&env_key)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define Hermes API key {env_key}")
                })?;
            let headers = if provider == "gemini" {
                vec![("x-goog-api-key".to_string(), token.to_string())]
            } else if provider == "anthropic" {
                vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ]
            } else {
                vec![("authorization".to_string(), format!("Bearer {token}"))]
            };

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        "opencode" => {
            let npm = parsed
                .get("npm")
                .and_then(|value| value.as_str())
                .unwrap_or("@ai-sdk/openai-compatible");
            let options = parsed
                .get("options")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let base_url = options
                .get("baseURL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if npm.contains("anthropic") {
                        "https://api.anthropic.com".to_string()
                    } else if npm.contains("google") {
                        default_base_url_for_gemini()
                    } else {
                        default_base_url_for_codex()
                    }
                });
            let token = options
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define an OpenCode API key")
                })?;
            let headers = if npm.contains("anthropic") {
                vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ]
            } else if npm.contains("google") {
                vec![("x-goog-api-key".to_string(), token.to_string())]
            } else {
                vec![("authorization".to_string(), format!("Bearer {token}"))]
            };

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        _ => Err(format!("Unsupported proxy tool: {tool_id}")),
    }
}

fn extract_toml_string(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(key) {
            return None;
        }
        let (_, raw) = trimmed.split_once('=')?;
        let value = raw.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

pub(super) fn build_upstream_request_url(
    base_url: &str,
    relative_path: &str,
    query: Option<&str>,
    use_full_url: bool,
) -> String {
    if use_full_url {
        let trimmed = base_url.trim();
        if let Some(query) = query.filter(|value| !value.is_empty()) {
            if trimmed.contains('?') {
                return trimmed.to_string();
            }
            return format!("{trimmed}?{query}");
        }
        return trimmed.to_string();
    }
    let base = base_url.trim().trim_end_matches('/');
    let relative = relative_path.trim_start_matches('/');
    let adjusted = if relative.is_empty() || base.ends_with(&format!("/{relative}")) {
        String::new()
    } else if let Some(stripped) = relative.strip_prefix("v1/") {
        if base.ends_with("/v1") {
            stripped.to_string()
        } else {
            relative.to_string()
        }
    } else if relative == "v1" && base.ends_with("/v1") {
        String::new()
    } else if let Some(stripped) = relative.strip_prefix("v1beta/") {
        if base.ends_with("/v1beta") {
            stripped.to_string()
        } else {
            relative.to_string()
        }
    } else if relative == "v1beta" && base.ends_with("/v1beta") {
        String::new()
    } else {
        relative.to_string()
    };

    let mut url = if adjusted.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{adjusted}")
    };

    if let Some(query) = query.filter(|value| !value.is_empty()) {
        url.push('?');
        url.push_str(query);
    }

    url
}

pub(super) fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name,
        "host"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "authorization"
            | "x-api-key"
            | "x-goog-api-key"
    )
}

pub(super) fn build_proxy_error(status: StatusCode, message: String) -> Response<Body> {
    (status, message).into_response()
}

pub(super) fn build_forward_response_from_parts(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: Body,
) -> Response<Body> {
    let mut response = Response::builder().status(status);

    for (name, value) in headers {
        if is_hop_by_hop_header(name.as_str()) {
            continue;
        }
        response = response.header(name, value);
    }

    match response.body(body) {
        Ok(response) => response,
        Err(error) => build_proxy_error(
            StatusCode::BAD_GATEWAY,
            format!("Failed to build proxy response: {error}"),
        ),
    }
}

pub(super) fn build_forward_response(upstream_response: reqwest::Response) -> Response<Body> {
    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let body = Body::from_stream(upstream_response.bytes_stream());
    build_forward_response_from_parts(status, &headers, body)
}

pub(super) fn build_json_response_from_value(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    value: &Value,
) -> Response<Body> {
    let payload = match serde_json::to_vec(value) {
        Ok(payload) => payload,
        Err(error) => {
            return build_proxy_error(
                StatusCode::BAD_GATEWAY,
                format!("Failed to serialize proxy response body: {error}"),
            );
        }
    };

    let mut response = Response::builder().status(status);
    let mut has_content_type = false;
    for (name, header_value) in headers {
        if is_hop_by_hop_header(name.as_str()) {
            continue;
        }
        if name == reqwest::header::CONTENT_TYPE {
            has_content_type = true;
        }
        response = response.header(name, header_value);
    }

    if !has_content_type {
        response = response.header(reqwest::header::CONTENT_TYPE, "application/json");
    }

    match response.body(Body::from(payload)) {
        Ok(response) => response,
        Err(error) => build_proxy_error(
            StatusCode::BAD_GATEWAY,
            format!("Failed to build JSON proxy response: {error}"),
        ),
    }
}

pub(super) fn reqwest_client() -> Result<reqwest::Client, String> {
    crate::shared::http_client::build_http_client_no_timeout(
        None,
        Some("CCHub Local Provider Proxy"),
    )
}

pub(super) fn next_proxy_request_id() -> String {
    let uuid = Uuid::new_v4().simple().to_string();
    format!(
        "proxy-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        &uuid[..8]
    )
}

pub(super) fn parse_json_bytes(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

pub(super) fn transform_claude_request_body(
    api_format: ClaudeApiFormat,
    body_bytes: &[u8],
) -> Result<Bytes, String> {
    let parsed = parse_json_bytes(body_bytes)
        .ok_or_else(|| "Claude transformed proxy request must be valid JSON".to_string())?;
    let transformed = match api_format {
        ClaudeApiFormat::Anthropic => parsed,
        ClaudeApiFormat::OpenAiChat => anthropic_to_openai(parsed)?,
        ClaudeApiFormat::OpenAiResponses => anthropic_to_responses(parsed)?,
        ClaudeApiFormat::GeminiNative => {
            let (gemini_body, _model_id) = crate::gemini_transform::anthropic_to_gemini(parsed)?;
            gemini_body
        }
    };
    serde_json::to_vec(&transformed)
        .map(Bytes::from)
        .map_err(|error| format!("Failed to serialize transformed Claude request: {error}"))
}

fn extract_gemini_model_from_path(relative_path: &str) -> Option<String> {
    let (_, suffix) = relative_path.split_once("models/")?;
    let model = suffix
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches('/');
    if model.is_empty() {
        None
    } else {
        Some(model.to_string())
    }
}

pub(super) fn extract_request_insights(
    tool_id: &str,
    relative_path: &str,
    body_bytes: &[u8],
) -> ProxyRequestInsights {
    let parsed = parse_json_bytes(body_bytes);
    let request_model = parsed
        .as_ref()
        .and_then(|value| value.get("model"))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if tool_id == "gemini" {
                extract_gemini_model_from_path(relative_path)
            } else {
                None
            }
        });

    let is_streaming = parsed
        .as_ref()
        .and_then(|value| value.get("stream"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || relative_path.contains("streamGenerateContent");

    ProxyRequestInsights {
        request_model,
        is_streaming,
    }
}
