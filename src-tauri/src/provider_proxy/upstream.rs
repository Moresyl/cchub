// 把 active profile 翻译成 upstream URL/headers，处理 Copilot OAuth 与候选端点。
use axum::body::Body;
use axum::http::{Response, StatusCode};
use axum::response::IntoResponse;
use bytes::Bytes;
use serde_json::Value;
use tauri::AppHandle;
use uuid::Uuid;

use crate::provider_proxy_transform::{anthropic_to_openai, anthropic_to_responses};

use super::profiles::{
    default_base_url_for_claude, default_base_url_for_codex, default_base_url_for_gemini,
    extract_cost_multiplier, extract_metadata_endpoint_candidates, extract_provider_type,
    extract_use_full_url, filter_endpoint_candidates,
};
use super::{ClaudeApiFormat, UpstreamTarget};
#[path = "request_insights.rs"]
mod request_insights;
pub(super) use request_insights::extract_request_insights;
pub(super) fn is_retryable_upstream_status(status: StatusCode) -> bool {
    matches!(
        status.as_u16(),
        408 | 409 | 425 | 429 | 500 | 502 | 503 | 504
    )
}

pub(super) async fn extract_upstream_target<R: tauri::Runtime>(
    app_handle: &AppHandle<R>,
    tool_id: &str,
    profile_id: String,
    profile_name: String,
    snapshot: &str,
) -> Result<UpstreamTarget, String> {
    let parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let metadata_candidates = extract_metadata_endpoint_candidates(&parsed);
    let provider_type = extract_provider_type(&parsed);
    let is_github_copilot = provider_type.as_deref() == Some("github_copilot");
    let is_codex_oauth = provider_type.as_deref() == Some("codex_oauth");
    let is_xai_oauth = provider_type.as_deref() == Some("xai_oauth");
    let cost_multiplier = extract_cost_multiplier(&parsed);
    let use_full_url = extract_use_full_url(&parsed);
    let transport_headers = extract_transport_headers(&parsed);
    let (request_header_overrides, request_body_override) = extract_local_proxy_overrides(&parsed);
    let mut managed_principal = None;

    let target = match tool_id {
        "claude" | "claude-desktop" => {
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
            let headers = if let Some(credentials) = super::managed_auth::resolve(
                app_handle,
                provider_type.as_deref(),
                &parsed,
                &profile_name,
            )
            .await?
            {
                let (headers, principal) = credentials.into_parts();
                managed_principal = Some(principal);
                headers
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
                request_header_overrides: Vec::new(),
                request_body_override: None,
                claude_api_format: Some(claude_api_format),
                is_github_copilot,
                is_codex_oauth,
                managed_principal: None,
                affinity: None,
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
                request_header_overrides: Vec::new(),
                request_body_override: None,
                claude_api_format: None,
                is_github_copilot: false,
                is_codex_oauth: false,
                managed_principal: None,
                affinity: None,
                cost_multiplier,
            })
        }
        "grokbuild" => {
            let config = parsed
                .get("config")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let config_value = config.parse::<toml::Value>().ok();
            let selected_model = config_value
                .as_ref()
                .and_then(|value| value.get("models"))
                .and_then(|value| value.get("default"))
                .and_then(|value| value.as_str())
                .or_else(|| parsed.get("model").and_then(Value::as_str))
                .unwrap_or("grok-4.5");
            let selected = config_value
                .as_ref()
                .and_then(|value| value.get("model"))
                .and_then(|value| value.get(selected_model));
            let legacy_provider = config_value.as_ref().and_then(|value| {
                let provider_name = value.get("model_provider")?.as_str()?;
                value.get("model_providers")?.get(provider_name)
            });
            let base_url = selected
                .and_then(|value| value.get("base_url"))
                .and_then(toml::Value::as_str)
                .or_else(|| {
                    legacy_provider
                        .and_then(|value| value.get("base_url"))
                        .and_then(toml::Value::as_str)
                })
                .or_else(|| parsed.get("baseUrl").and_then(Value::as_str))
                .or_else(|| parsed.get("base_url").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("https://api.x.ai/v1")
                .to_string();
            let headers = if is_xai_oauth {
                let credentials = super::managed_auth::resolve(
                    app_handle,
                    provider_type.as_deref(),
                    &parsed,
                    &profile_name,
                )
                .await?
                .ok_or("Managed xAI credentials are unavailable")?;
                let (headers, principal) = credentials.into_parts();
                managed_principal = Some(principal);
                headers
            } else {
                let token = selected
                    .and_then(|value| value.get("api_key"))
                    .and_then(toml::Value::as_str)
                    .or_else(|| {
                        legacy_provider
                            .and_then(|value| value.get("api_key"))
                            .and_then(toml::Value::as_str)
                    })
                    .or_else(|| parsed.get("apiKey").and_then(Value::as_str))
                    .or_else(|| {
                        parsed
                            .get("auth")
                            .and_then(|value| value.get("OPENAI_API_KEY"))
                            .and_then(Value::as_str)
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .or_else(|| {
                        selected
                            .and_then(|value| value.get("env_key"))
                            .and_then(toml::Value::as_str)
                            .or_else(|| {
                                legacy_provider
                                    .and_then(|value| value.get("env_key"))
                                    .and_then(|value| value.as_str())
                            })
                            .and_then(|key| std::env::var(key).ok())
                            .map(|value| value.trim().to_string())
                            .filter(|value| !value.is_empty())
                    })
                    .ok_or_else(|| {
                        format!("Provider {profile_name} does not define a Grok Build API key")
                    })?;
                vec![("authorization".to_string(), format!("Bearer {token}"))]
            };
            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                request_header_overrides: Vec::new(),
                request_body_override: None,
                claude_api_format: None,
                is_github_copilot: false,
                is_codex_oauth: false,
                managed_principal: None,
                affinity: None,
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
                request_header_overrides: Vec::new(),
                request_body_override: None,
                claude_api_format: None,
                is_github_copilot: false,
                is_codex_oauth: false,
                managed_principal: None,
                affinity: None,
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
                request_header_overrides: Vec::new(),
                request_body_override: None,
                claude_api_format: None,
                is_github_copilot: false,
                is_codex_oauth: false,
                managed_principal: None,
                affinity: None,
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
                request_header_overrides: Vec::new(),
                request_body_override: None,
                claude_api_format: None,
                is_github_copilot: false,
                is_codex_oauth: false,
                managed_principal: None,
                affinity: None,
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
                request_header_overrides: Vec::new(),
                request_body_override: None,
                claude_api_format: None,
                is_github_copilot: false,
                is_codex_oauth: false,
                managed_principal: None,
                affinity: None,
                cost_multiplier,
            })
        }
        _ => Err(format!("Unsupported proxy tool: {tool_id}")),
    };
    target.map(|mut target| {
        target.managed_principal = managed_principal;
        target.headers.extend(transport_headers);
        target.request_header_overrides = request_header_overrides;
        target.request_body_override = request_body_override;
        target
    })
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
pub(crate) fn extract_transport_headers(parsed: &Value) -> Vec<(String, String)> {
    let metadata = parsed.get("metadata").and_then(Value::as_object);
    let custom_user_agent = metadata
        .and_then(|value| {
            value
                .get("customUserAgent")
                .or_else(|| value.get("custom_user_agent"))
        })
        .or_else(|| {
            parsed
                .get("customUserAgent")
                .or_else(|| parsed.get("custom_user_agent"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .filter(|value| {
            !value
                .bytes()
                .any(|byte| byte < 0x20 && byte != b'\t' || byte == 0x7f)
        })
        .map(|value| ("user-agent".to_string(), value.to_string()));
    let mut headers = Vec::new();
    let request_headers = metadata
        .and_then(|value| {
            value
                .get("requestHeaders")
                .or_else(|| value.get("request_headers"))
        })
        .or_else(|| {
            parsed
                .get("requestHeaders")
                .or_else(|| parsed.get("request_headers"))
        })
        .and_then(Value::as_object);
    if let Some(request_headers) = request_headers {
        for (name, value) in request_headers {
            let name = name.trim();
            if name.is_empty()
                || name.len() > 128
                || value.as_str().is_none()
                || headers.len() >= 64
            {
                continue;
            }
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
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
                    | "chatgpt-account-id"
            ) {
                continue;
            }
            let Ok(name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) else {
                continue;
            };
            let Some(value) = value.as_str() else {
                continue;
            };
            if value.len() > 4096 || reqwest::header::HeaderValue::from_str(value).is_err() {
                continue;
            }
            if lower == "user-agent" && custom_user_agent.is_some() {
                continue;
            }
            headers.push((name.as_str().to_string(), value.to_string()));
        }
    }
    if let Some(custom_user_agent) = custom_user_agent {
        headers.push(custom_user_agent);
    }
    headers
}

pub(crate) fn extract_local_proxy_overrides(
    parsed: &Value,
) -> (Vec<(String, String)>, Option<Value>) {
    let overrides = parsed
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| {
            metadata
                .get("localProxyRequestOverrides")
                .or_else(|| metadata.get("local_proxy_request_overrides"))
        })
        .or_else(|| {
            parsed
                .get("localProxyRequestOverrides")
                .or_else(|| parsed.get("local_proxy_request_overrides"))
        })
        .and_then(Value::as_object);
    let Some(overrides) = overrides else {
        return (Vec::new(), None);
    };

    let mut headers = Vec::new();
    if let Some(values) = overrides.get("headers").and_then(Value::as_object) {
        for (raw_name, raw_value) in values {
            let name = raw_name.trim();
            let Some(value) = raw_value.as_str() else {
                continue;
            };
            if name.is_empty()
                || name.len() > 128
                || value.len() > 4096
                || reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_err()
                || reqwest::header::HeaderValue::from_str(value).is_err()
                || headers.len() >= 64
            {
                continue;
            }
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "host"
                    | "content-length"
                    | "transfer-encoding"
                    | "connection"
                    | "proxy-authorization"
                    | "authorization"
                    | "x-api-key"
                    | "x-goog-api-key"
                    | "chatgpt-account-id"
                    | "content-type"
            ) || headers
                .iter()
                .any(|(existing, _): &(String, String)| existing.eq_ignore_ascii_case(name))
            {
                continue;
            }
            headers.push((name.to_string(), value.to_string()));
        }
    }

    let body = overrides
        .get("body")
        .filter(|value| value.is_object())
        .cloned()
        .and_then(|mut value| {
            value.as_object_mut()?.remove("stream");
            let encoded = serde_json::to_vec(&value).ok()?;
            (encoded.len() <= 64 * 1024
                && value.as_object().is_some_and(|object| !object.is_empty()))
            .then_some(value)
        });
    (headers, body)
}

pub(super) fn build_upstream_request_url(
    base_url: &str,
    relative_path: &str,
    query: Option<&str>,
    use_full_url: bool,
) -> Result<String, String> {
    if use_full_url {
        let trimmed = base_url.trim();
        if relative_path.trim_matches('/') == "alpha/search" {
            return super::alpha_search::rewrite_full_url(trimmed, query);
        }
        if let Some(query) = query.filter(|value| !value.is_empty()) {
            if trimmed.contains('?') {
                return Ok(trimmed.to_string());
            }
            return Ok(format!("{trimmed}?{query}"));
        }
        return Ok(trimmed.to_string());
    }
    let base = base_url.trim().split('#').next().unwrap_or_default();
    let (base, base_query) = base
        .split_once('?')
        .map_or((base, None), |(path, query)| (path, Some(query)));
    let base = base.trim_end_matches('/');
    let relative = relative_path.trim_start_matches('/');
    let (relative, relative_query) = relative
        .split_once('?')
        .map_or((relative, None), |(path, query)| (path, Some(query)));
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

    // The base query belongs after the joined path, never inside it. Preserve
    // configured parameters ahead of caller parameters with the same name.
    for query in [base_query, relative_query, query]
        .into_iter()
        .flatten()
        .filter(|value| !value.is_empty())
    {
        if let Some((_, existing)) = url.split_once('?') {
            let keys: std::collections::HashSet<_> =
                url::form_urlencoded::parse(existing.as_bytes())
                    .map(|(key, _)| key.into_owned())
                    .collect();
            let mut extra = url::form_urlencoded::Serializer::new(String::new());
            for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
                if !keys.contains(key.as_ref()) {
                    extra.append_pair(&key, &value);
                }
            }
            let extra = extra.finish();
            if !extra.is_empty() {
                url.push('&');
                url.push_str(&extra);
            }
        } else {
            url.push('?');
            url.push_str(query);
        }
    }

    Ok(url)
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

pub(super) fn reqwest_client(proxy_url: Option<&str>) -> Result<reqwest::Client, String> {
    crate::shared::http_client::build_http_client_no_timeout(
        proxy_url,
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
    is_codex_oauth: bool,
) -> Result<Bytes, String> {
    let parsed = parse_json_bytes(body_bytes)
        .ok_or_else(|| "Claude transformed proxy request must be valid JSON".to_string())?;
    let transformed = match api_format {
        ClaudeApiFormat::Anthropic => parsed,
        ClaudeApiFormat::OpenAiChat => anthropic_to_openai(parsed)?,
        ClaudeApiFormat::OpenAiResponses => anthropic_to_responses(parsed, is_codex_oauth)?,
        ClaudeApiFormat::GeminiNative => {
            let (gemini_body, _model_id) = crate::gemini_transform::anthropic_to_gemini(parsed)?;
            gemini_body
        }
    };
    serde_json::to_vec(&transformed)
        .map(Bytes::from)
        .map_err(|error| format!("Failed to serialize transformed Claude request: {error}"))
}
