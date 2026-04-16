use reqwest::Url;
use serde::Deserialize;
use tauri::State;

use crate::db::DbState;

const PROVIDER_MODELS_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Option<Vec<OpenAiModelEntry>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelEntry {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Option<Vec<GeminiModelEntry>>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelEntry {
    name: Option<String>,
}

fn build_provider_models_client(conn: &rusqlite::Connection) -> Result<reqwest::Client, String> {
    let proxy_url: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'proxy_url'",
            [],
            |row| row.get(0),
        )
        .ok();

    let mut builder = reqwest::Client::builder()
        .user_agent("CCHub Provider Models Fetcher")
        .timeout(std::time::Duration::from_secs(
            PROVIDER_MODELS_TIMEOUT_SECS,
        ));

    if let Some(proxy_url) = proxy_url {
        let trimmed = proxy_url.trim();
        if !trimmed.is_empty() {
            let proxy =
                reqwest::Proxy::all(trimmed).map_err(|e| format!("Invalid proxy: {e}"))?;
            builder = builder.proxy(proxy);
        }
    }

    builder.build().map_err(|e| e.to_string())
}

fn trim_query_and_fragment(base_url: &str) -> &str {
    base_url
        .split('#')
        .next()
        .unwrap_or(base_url)
        .split('?')
        .next()
        .unwrap_or(base_url)
}

fn derive_root_from_full_url(tool_id: &str, base_url: &str) -> Result<String, String> {
    let trimmed = trim_query_and_fragment(base_url).trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Base URL is empty".to_string());
    }

    let replacements = match tool_id {
        "claude" => vec!["/v1/messages", "/messages"],
        "codex" | "openclaw" | "opencode" => {
            vec!["/v1/chat/completions", "/chat/completions", "/v1/responses", "/responses"]
        }
        "gemini" => {
            if let Some(prefix) = trimmed.split(":streamGenerateContent").next() {
                if let Some(index) = prefix.find("/models/") {
                    return Ok(prefix[..index].to_string());
                }
            }
            vec![]
        }
        _ => vec![],
    };

    for suffix in replacements {
        if let Some(value) = trimmed.strip_suffix(suffix) {
            return Ok(value.trim_end_matches('/').to_string());
        }
    }

    if tool_id == "gemini" {
        if let Some(index) = trimmed.find("/models/") {
            return Ok(trimmed[..index].trim_end_matches('/').to_string());
        }
    }

    if let Some(index) = trimmed.rfind("/v1/") {
        return Ok(trimmed[..index].to_string());
    }

    if let Some(index) = trimmed.rfind("/v1beta/") {
        return Ok(trimmed[..index].to_string());
    }

    if let Some(index) = trimmed.rfind('/') {
        let candidate = trimmed[..index].trim_end_matches('/');
        if candidate.contains("://") {
            return Ok(candidate.to_string());
        }
    }

    Err("Cannot derive models endpoint from full URL".to_string())
}

fn build_openai_models_url(base_url: &str, use_full_url: bool) -> Result<String, String> {
    let root = if use_full_url {
        derive_root_from_full_url("codex", base_url)?
    } else {
        base_url.trim().trim_end_matches('/').to_string()
    };

    let normalized = if root.is_empty() {
        "https://api.openai.com".to_string()
    } else {
        root
    };

    if normalized.ends_with("/v1") {
        Ok(format!("{normalized}/models"))
    } else {
        Ok(format!("{normalized}/v1/models"))
    }
}

fn build_claude_models_url(base_url: &str, use_full_url: bool) -> Result<String, String> {
    let root = if use_full_url {
        derive_root_from_full_url("claude", base_url)?
    } else {
        base_url.trim().trim_end_matches('/').to_string()
    };

    let normalized = if root.is_empty() {
        "https://api.anthropic.com".to_string()
    } else {
        root
    };

    if normalized.ends_with("/v1") {
        Ok(format!("{normalized}/models"))
    } else {
        Ok(format!("{normalized}/v1/models"))
    }
}

fn build_gemini_models_url(base_url: &str, use_full_url: bool) -> Result<String, String> {
    let root = if use_full_url {
        derive_root_from_full_url("gemini", base_url)?
    } else {
        base_url.trim().trim_end_matches('/').to_string()
    };

    let normalized = if root.is_empty() {
        "https://generativelanguage.googleapis.com".to_string()
    } else {
        root
    };

    if normalized.ends_with("/v1beta") {
        Ok(format!("{normalized}/models"))
    } else {
        Ok(format!("{normalized}/v1beta/models"))
    }
}

fn normalize_model_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(
        trimmed
            .strip_prefix("models/")
            .unwrap_or(trimmed)
            .to_string(),
    )
}

fn sort_and_dedup_models(models: Vec<String>) -> Vec<String> {
    let mut next = models
        .into_iter()
        .filter_map(|value| normalize_model_id(&value))
        .collect::<Vec<_>>();
    next.sort();
    next.dedup();
    next
}

#[tauri::command]
pub async fn fetch_provider_models(
    tool_id: String,
    base_url: String,
    api_key: String,
    use_full_url: Option<bool>,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    let client = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        build_provider_models_client(&conn)?
    };

    let normalized_tool = tool_id.trim().to_ascii_lowercase();
    let use_full_url = use_full_url.unwrap_or(false);

    match normalized_tool.as_str() {
        "claude" => {
            if api_key.trim().is_empty() {
                return Err("API key is required to fetch Claude models".to_string());
            }

            let models_url = build_claude_models_url(&base_url, use_full_url)?;
            let response = client
                .get(&models_url)
                .header("x-api-key", api_key.trim())
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .map_err(|e| format!("Request failed: {e}"))?;
            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(format!("HTTP {status}: {body}"));
            }

            let parsed: OpenAiModelsResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {e}"))?;
            Ok(sort_and_dedup_models(
                parsed
                    .data
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|item| item.id)
                    .collect(),
            ))
        }
        "codex" | "openclaw" | "opencode" => {
            if api_key.trim().is_empty() {
                return Err("API key is required to fetch models".to_string());
            }

            let models_url = build_openai_models_url(&base_url, use_full_url)?;
            let response = client
                .get(&models_url)
                .header("authorization", format!("Bearer {}", api_key.trim()))
                .send()
                .await
                .map_err(|e| format!("Request failed: {e}"))?;
            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(format!("HTTP {status}: {body}"));
            }

            let parsed: OpenAiModelsResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {e}"))?;
            Ok(sort_and_dedup_models(
                parsed
                    .data
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|item| item.id)
                    .collect(),
            ))
        }
        "gemini" => {
            if api_key.trim().is_empty() {
                return Err("API key is required to fetch Gemini models".to_string());
            }

            let mut url =
                Url::parse(&build_gemini_models_url(&base_url, use_full_url)?).map_err(|e| {
                    format!("Invalid Gemini models URL: {e}")
                })?;
            url.query_pairs_mut().append_pair("key", api_key.trim());

            let response = client
                .get(url)
                .send()
                .await
                .map_err(|e| format!("Request failed: {e}"))?;
            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(format!("HTTP {status}: {body}"));
            }

            let parsed: GeminiModelsResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {e}"))?;
            Ok(sort_and_dedup_models(
                parsed
                    .models
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|item| item.name)
                    .collect(),
            ))
        }
        _ => Err(format!("Fetching models is not supported for {tool_id}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_claude_models_url, build_gemini_models_url, build_openai_models_url,
        normalize_model_id,
    };

    #[test]
    fn openai_models_url_uses_v1_suffix() {
        assert_eq!(
            build_openai_models_url("https://api.openai.com", false).unwrap(),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            build_openai_models_url("https://api.openai.com/v1", false).unwrap(),
            "https://api.openai.com/v1/models"
        );
    }

    #[test]
    fn full_openai_endpoint_derives_models_url() {
        assert_eq!(
            build_openai_models_url("https://proxy.example.com/v1/chat/completions", true)
                .unwrap(),
            "https://proxy.example.com/v1/models"
        );
        assert_eq!(
            build_openai_models_url("https://proxy.example.com/v1/responses", true).unwrap(),
            "https://proxy.example.com/v1/models"
        );
    }

    #[test]
    fn full_claude_endpoint_derives_models_url() {
        assert_eq!(
            build_claude_models_url("https://api.example.com/v1/messages", true).unwrap(),
            "https://api.example.com/v1/models"
        );
    }

    #[test]
    fn full_gemini_endpoint_derives_models_url() {
        assert_eq!(
            build_gemini_models_url(
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse",
                true,
            )
            .unwrap(),
            "https://generativelanguage.googleapis.com/v1beta/models"
        );
    }

    #[test]
    fn normalize_model_id_removes_gemini_prefix() {
        assert_eq!(
            normalize_model_id("models/gemini-2.5-pro").as_deref(),
            Some("gemini-2.5-pro")
        );
    }
}
