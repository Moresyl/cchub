#![allow(clippy::too_many_arguments)]
use std::time::Duration;
use tauri::AppHandle;

use crate::hermes;
use crate::shared::http_client;

use super::stream_auth::{
    build_openai_chat_endpoint, build_openai_responses_endpoint, extract_provider_type,
    extract_use_full_url, resolve_codex_headers, resolve_copilot_headers, resolve_xai_headers,
};
use super::*;

pub struct StreamCheckRequestSpec {
    pub endpoint: String,
    pub headers: Vec<(String, String)>,
    pub body: serde_json::Value,
}

pub fn build_provider_probe_client(conn: &rusqlite::Connection) -> Result<reqwest::Client, String> {
    let proxy_url = get_text_app_setting(conn, "proxy_url")?.unwrap_or_default();
    http_client::build_http_client(
        Some(proxy_url.as_str()),
        Some("CCHub Provider Probe"),
        Duration::from_secs(10),
    )
}

pub async fn extract_probe_target(
    app_handle: &AppHandle,
    profile: &ConfigProfile,
) -> Result<(Option<String>, Vec<(String, String)>), String> {
    let parsed: serde_json::Value =
        serde_json::from_str(&profile.config_snapshot).map_err(|e| e.to_string())?;
    let provider_type = extract_provider_type(&parsed);
    let use_full_url = extract_use_full_url(&parsed);

    match profile.tool_id.as_str() {
        "claude" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                base_url
            } else if matches!(
                provider_type.as_deref(),
                Some("github_copilot") | Some("codex_oauth") | Some("xai_oauth")
            ) {
                base_url.map(|value| join_api_endpoint(&value, "models", false))
            } else {
                base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let headers = if provider_type.as_deref() == Some("github_copilot") {
                resolve_copilot_headers(app_handle, &parsed).await?
            } else if provider_type.as_deref() == Some("codex_oauth") {
                resolve_codex_headers(app_handle, &parsed).await?
            } else if provider_type.as_deref() == Some("xai_oauth") {
                resolve_xai_headers(app_handle, &parsed).await?
            } else {
                let mut headers = Vec::new();
                if let Some(token) = env
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .or_else(|| env.get("ANTHROPIC_API_KEY"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let api_format = env
                        .get("ANTHROPIC_API_FORMAT")
                        .and_then(|value| value.as_str())
                        .unwrap_or("anthropic");
                    if api_format == "anthropic" {
                        headers.push(("x-api-key".to_string(), token.to_string()));
                        headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
                    } else {
                        headers.push(("authorization".to_string(), format!("Bearer {token}")));
                    }
                }
                headers
            };
            Ok((base_url, headers))
        }
        "codex" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let explicit_base_url = parse_toml_assignment(config, "base_url");
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.openai.com/v1".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("auth")
                .and_then(|value| value.get("OPENAI_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
        }
        "gemini" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| {
                    Some("https://generativelanguage.googleapis.com/v1beta".to_string())
                })
            };
            let mut headers = Vec::new();
            if let Some(token) = env
                .get("GEMINI_API_KEY")
                .or_else(|| env.get("GOOGLE_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("x-goog-api-key".to_string(), token.to_string()));
            }
            Ok((base_url, headers))
        }
        "openclaw" => {
            let explicit_base_url = parsed
                .get("baseUrl")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
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
            let explicit_base_url = model
                .get("base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| {
                    hermes::providers::default_base_url_for_provider(provider).map(str::to_string)
                })
            };
            let env_key = parsed
                .get("metadata")
                .and_then(|value| value.get("hermesApiKeyEnv"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    hermes::providers::default_env_key_for_provider(provider).map(str::to_string)
                });
            let mut headers = Vec::new();
            if let Some(token) = env_key
                .as_deref()
                .and_then(|key| env.get(key))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if provider == "gemini" {
                    headers.push(("x-goog-api-key".to_string(), token.to_string()));
                } else if provider == "anthropic" {
                    headers.push(("x-api-key".to_string(), token.to_string()));
                    headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
                } else {
                    headers.push(("authorization".to_string(), format!("Bearer {token}")));
                }
            }
            Ok((base_url, headers))
        }
        "opencode" => {
            let explicit_base_url = parsed
                .get("options")
                .and_then(|value| value.get("baseURL"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("options")
                .and_then(|value| value.get("apiKey"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
        }
        _ => Ok((None, Vec::new())),
    }
}

pub fn classify_provider_latency_status(latency_ms: u64) -> String {
    if latency_ms < 200 {
        "fast".to_string()
    } else if latency_ms <= 500 {
        "medium".to_string()
    } else {
        "slow".to_string()
    }
}

pub async fn extract_stream_check_request(
    app_handle: &AppHandle,
    profile: &ConfigProfile,
) -> Result<StreamCheckRequestSpec, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(&profile.config_snapshot).map_err(|e| e.to_string())?;
    let provider_type = extract_provider_type(&parsed);
    let use_full_url = extract_use_full_url(&parsed);

    match profile.tool_id.as_str() {
        "claude" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Claude base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string())
            };
            let model = env
                .get("ANTHROPIC_MODEL")
                .or_else(|| env.get("ANTHROPIC_DEFAULT_SONNET_MODEL"))
                .or_else(|| env.get("ANTHROPIC_REASONING_MODEL"))
                .or_else(|| env.get("ANTHROPIC_DEFAULT_HAIKU_MODEL"))
                .or_else(|| env.get("ANTHROPIC_DEFAULT_OPUS_MODEL"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("claude-sonnet-4-5");
            let api_format = env
                .get("ANTHROPIC_API_FORMAT")
                .and_then(|value| value.as_str())
                .unwrap_or("anthropic");

            if provider_type.as_deref() == Some("github_copilot") || api_format == "openai_chat" {
                let headers = if provider_type.as_deref() == Some("github_copilot") {
                    resolve_copilot_headers(app_handle, &parsed).await?
                } else if provider_type.as_deref() == Some("xai_oauth") {
                    resolve_xai_headers(app_handle, &parsed).await?
                } else {
                    let token = env
                        .get("ANTHROPIC_AUTH_TOKEN")
                        .or_else(|| env.get("ANTHROPIC_API_KEY"))
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| "No Claude API token configured".to_string())?;
                    vec![("authorization".to_string(), format!("Bearer {token}"))]
                };
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_openai_chat_endpoint(
                        &base_url,
                        provider_type.as_deref(),
                        use_full_url,
                    ),
                    headers,
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                });
            }

            if api_format == "openai_responses" {
                let headers = if provider_type.as_deref() == Some("codex_oauth") {
                    resolve_codex_headers(app_handle, &parsed).await?
                } else {
                    let token = env
                        .get("ANTHROPIC_AUTH_TOKEN")
                        .or_else(|| env.get("ANTHROPIC_API_KEY"))
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| "No Claude API token configured".to_string())?;
                    vec![("authorization".to_string(), format!("Bearer {token}"))]
                };
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_openai_responses_endpoint(
                        &base_url,
                        provider_type.as_deref(),
                        use_full_url,
                    ),
                    headers,
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                });
            }

            let token = env
                .get("ANTHROPIC_AUTH_TOKEN")
                .or_else(|| env.get("ANTHROPIC_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Claude API token configured".to_string())?;

            Ok(StreamCheckRequestSpec {
                endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                headers: vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ],
                body: serde_json::json!({
                    "model": model,
                    "max_tokens": 16,
                    "stream": true,
                    "messages": [
                        { "role": "user", "content": "Reply with OK." }
                    ],
                }),
            })
        }
        "codex" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let token = parsed
                .get("auth")
                .and_then(|value| value.get("OPENAI_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Codex OPENAI_API_KEY configured".to_string())?;
            let explicit_base_url = parse_toml_assignment(config, "base_url");
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Codex base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            };
            let wire_api = parse_toml_assignment(config, "wire_api")
                .unwrap_or_else(|| "responses".to_string());
            let model =
                parse_toml_assignment(config, "model").unwrap_or_else(|| "gpt-5.6-sol".to_string());
            let (endpoint, body) = if wire_api == "chat" {
                (
                    join_api_endpoint(&base_url, "chat/completions", use_full_url),
                    serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                )
            } else {
                (
                    join_api_endpoint(&base_url, "responses", use_full_url),
                    serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                )
            };

            Ok(StreamCheckRequestSpec {
                endpoint,
                headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                body,
            })
        }
        "gemini" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let token = env
                .get("GEMINI_API_KEY")
                .or_else(|| env.get("GOOGLE_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Gemini API key configured".to_string())?;
            let explicit_base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Gemini base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| {
                    "https://generativelanguage.googleapis.com/v1beta".to_string()
                })
            };
            let model = env
                .get("GEMINI_MODEL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gemini-3.6-flash");

            Ok(StreamCheckRequestSpec {
                endpoint: build_gemini_stream_endpoint(&base_url, model, use_full_url),
                headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                body: serde_json::json!({
                    "contents": [
                        {
                            "role": "user",
                            "parts": [{ "text": "Reply with OK." }]
                        }
                    ],
                    "generationConfig": {
                        "maxOutputTokens": 16
                    }
                }),
            })
        }
        "openclaw" => {
            let api = parsed
                .get("api")
                .and_then(|value| value.as_str())
                .unwrap_or("openai-completions");
            let explicit_base_url = parsed
                .get("baseUrl")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No OpenClaw baseUrl configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string())
            };
            let api_key = parsed
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let model = parsed
                .get("models")
                .and_then(|value| value.as_array())
                .and_then(|models| models.first())
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gpt-5.6-sol");

            match api {
                "openai-responses" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: join_api_endpoint(&base_url, "responses", use_full_url),
                        headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                        body: serde_json::json!({
                            "model": model,
                            "stream": true,
                            "max_output_tokens": 16,
                            "input": "Reply with OK.",
                        }),
                    })
                }
                "anthropic-messages" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                        headers: vec![
                            ("x-api-key".to_string(), token),
                            ("anthropic-version".to_string(), "2023-06-01".to_string()),
                        ],
                        body: serde_json::json!({
                            "model": model,
                            "max_tokens": 16,
                            "stream": true,
                            "messages": [
                                { "role": "user", "content": "Reply with OK." }
                            ],
                        }),
                    })
                }
                "google-generative-ai" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: build_gemini_stream_endpoint(&base_url, model, use_full_url),
                        headers: vec![("x-goog-api-key".to_string(), token)],
                        body: serde_json::json!({
                            "contents": [
                                {
                                    "role": "user",
                                    "parts": [{ "text": "Reply with OK." }]
                                }
                            ],
                            "generationConfig": {
                                "maxOutputTokens": 16
                            }
                        }),
                    })
                }
                "bedrock-converse-stream" => Err("AWS Bedrock ConverseStream requires SigV4 signing and is not yet supported for stream checks".to_string()),
                _ => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                        headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                        body: serde_json::json!({
                            "model": model,
                            "stream": true,
                            "max_tokens": 16,
                            "messages": [
                                { "role": "user", "content": "Reply with OK." }
                            ],
                        }),
                    })
                }
            }
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
            let explicit_base_url = model
                .get("base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Hermes base_url configured".to_string())?
            } else {
                explicit_base_url
                    .or_else(|| {
                        hermes::providers::default_base_url_for_provider(provider)
                            .map(str::to_string)
                    })
                    .ok_or_else(|| "No Hermes base_url configured".to_string())?
            };
            let model_id = model
                .get("default")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gpt-5.6-sol");
            let env_key = parsed
                .get("metadata")
                .and_then(|value| value.get("hermesApiKeyEnv"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    hermes::providers::default_env_key_for_provider(provider).map(str::to_string)
                })
                .ok_or_else(|| "No Hermes API key env configured".to_string())?;
            let token = env
                .get(&env_key)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("No Hermes API key configured in {env_key}"))?;

            if provider == "gemini" {
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_gemini_stream_endpoint(&base_url, model_id, use_full_url),
                    headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                    body: serde_json::json!({
                        "contents": [{ "role": "user", "parts": [{ "text": "Reply with OK." }] }],
                        "generationConfig": { "maxOutputTokens": 16 },
                    }),
                });
            }

            if provider == "anthropic" {
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                    headers: vec![
                        ("x-api-key".to_string(), token.to_string()),
                        ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ],
                    body: serde_json::json!({
                        "model": model_id,
                        "max_tokens": 16,
                        "stream": true,
                        "messages": [{ "role": "user", "content": "Reply with OK." }],
                    }),
                });
            }

            Ok(StreamCheckRequestSpec {
                endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                body: serde_json::json!({
                    "model": model_id,
                    "stream": true,
                    "max_tokens": 16,
                    "messages": [{ "role": "user", "content": "Reply with OK." }],
                }),
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
            let explicit_base_url = options
                .get("baseURL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No OpenCode baseURL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            };
            let token = options
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No OpenCode API key configured".to_string())?;
            let model = parsed
                .get("models")
                .and_then(|value| value.as_object())
                .and_then(|value| value.keys().next().cloned())
                .unwrap_or_else(|| "gpt-5.6-sol".to_string());

            if npm.contains("anthropic") {
                Ok(StreamCheckRequestSpec {
                    endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                    headers: vec![
                        ("x-api-key".to_string(), token.to_string()),
                        ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ],
                    body: serde_json::json!({
                        "model": model,
                        "max_tokens": 16,
                        "stream": true,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                })
            } else if npm.contains("google") {
                Ok(StreamCheckRequestSpec {
                    endpoint: build_gemini_stream_endpoint(&base_url, &model, use_full_url),
                    headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                    body: serde_json::json!({
                        "contents": [
                            {
                                "role": "user",
                                "parts": [{ "text": "Reply with OK." }]
                            }
                        ],
                        "generationConfig": {
                            "maxOutputTokens": 16
                        }
                    }),
                })
            } else if npm == "@ai-sdk/openai" {
                Ok(StreamCheckRequestSpec {
                    endpoint: join_api_endpoint(&base_url, "responses", use_full_url),
                    headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                })
            } else {
                Ok(StreamCheckRequestSpec {
                    endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                    headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                })
            }
        }
        _ => Err("Stream check is not supported for this profile".to_string()),
    }
}
