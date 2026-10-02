use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use tauri::State;

use crate::db::DbState;

mod catalog;
mod detailed;

pub use catalog::ModelInfo;
pub(crate) use catalog::{merge_catalog, model as parse_model_info};

const PROVIDER_MODELS_TIMEOUT_SECS: u64 = 15;

/// Model shape used by the provider editor. It intentionally keeps optional
/// ownership metadata so OpenAI-compatible gateways can return richer rows
/// without forcing the UI to understand every vendor-specific field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedModel {
    pub id: String,
    pub owned_by: Option<String>,
}

fn build_provider_models_client(conn: &rusqlite::Connection) -> Result<reqwest::Client, String> {
    let proxy_url: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'proxy_url'",
            [],
            |row| row.get(0),
        )
        .ok();

    crate::shared::http_client::build_http_client(
        proxy_url.as_deref(),
        Some("CCHub Provider Models Fetcher"),
        std::time::Duration::from_secs(PROVIDER_MODELS_TIMEOUT_SECS),
    )
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
    let trimmed = trim_query_and_fragment(base_url)
        .trim()
        .trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Base URL is empty".to_string());
    }

    let replacements = match tool_id {
        "claude" => vec!["/v1/messages", "/messages"],
        "codex" | "grokbuild" | "openclaw" | "opencode" | "hermes" => {
            vec![
                "/v1/chat/completions",
                "/chat/completions",
                "/v1/responses",
                "/responses",
            ]
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

fn generic_models_url(
    base_url: &str,
    is_full_url: bool,
    explicit: Option<&str>,
) -> Result<Url, String> {
    let candidate = explicit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            let trimmed = trim_query_and_fragment(base_url).trim_end_matches('/');
            if trimmed.ends_with("/models") {
                trimmed.to_string()
            } else if trimmed.ends_with("/v1") || trimmed.ends_with("/v1beta") {
                format!("{trimmed}/models")
            } else if is_full_url {
                let root = [
                    "/chat/completions",
                    "/v1/chat/completions",
                    "/responses",
                    "/v1/responses",
                    "/messages",
                    "/v1/messages",
                ]
                .iter()
                .find_map(|suffix| trimmed.strip_suffix(suffix))
                .unwrap_or(trimmed);
                if root.ends_with("/v1") || root.ends_with("/v1beta") {
                    format!("{root}/models")
                } else {
                    format!("{root}/v1/models")
                }
            } else {
                format!("{trimmed}/v1/models")
            }
        });
    let url = Url::parse(&candidate).map_err(|error| format!("Invalid models URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Models URL must use HTTP(S) and include a host".to_string());
    }
    Ok(url)
}

fn parse_fetched_models(payload: &Value) -> Vec<FetchedModel> {
    let entries = payload
        .get("data")
        .or_else(|| payload.get("models"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut models = entries
        .into_iter()
        .filter_map(|entry| {
            if let Some(id) = entry.as_str().and_then(normalize_model_id) {
                return Some(FetchedModel { id, owned_by: None });
            }
            let id = entry
                .get("id")
                .and_then(Value::as_str)
                .and_then(normalize_model_id)?;
            Some(FetchedModel {
                id,
                owned_by: entry
                    .get("owned_by")
                    .or_else(|| entry.get("ownedBy"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.id.cmp(&right.id));
    models.dedup_by(|left, right| left.id == right.id);
    models
}

fn apply_model_fetch_headers(
    request: reqwest::RequestBuilder,
    api_key: &str,
    api_format: Option<&str>,
    custom_user_agent: Option<&str>,
    request_headers: Option<&BTreeMap<String, String>>,
) -> reqwest::RequestBuilder {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, USER_AGENT};
    let mut headers = HeaderMap::new();
    if let Some(custom) = request_headers {
        for (name, value) in custom {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.trim().as_bytes()),
                HeaderValue::from_str(value),
            ) {
                headers.insert(name, value);
            }
        }
    }
    let format = api_format.unwrap_or_default().to_ascii_lowercase();
    if !api_key.trim().is_empty() {
        if format.contains("anthropic") {
            // These values replace matching custom headers, rather than append
            // another credential that the endpoint could interpret differently.
            if let Ok(value) = HeaderValue::from_str(api_key.trim()) {
                headers.insert("x-api-key", value);
            }
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        } else if format.contains("google") || format.contains("gemini") {
            if let Ok(value) = HeaderValue::from_str(api_key.trim()) {
                headers.insert("x-goog-api-key", value);
            }
        } else if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", api_key.trim())) {
            headers.insert(AUTHORIZATION, value);
        }
    }
    if let Some(user_agent) = custom_user_agent
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Ok(value) = HeaderValue::from_str(user_agent) {
            headers.insert(USER_AGENT, value);
        }
    }
    request.headers(headers)
}

/// Fetches a model list without requiring a saved provider profile. This is
/// useful while a provider form is still being edited and supports explicit
/// endpoints, custom headers and common provider auth conventions.
#[tauri::command(rename_all = "camelCase")]
pub async fn fetch_models_for_config(
    base_url: String,
    api_key: String,
    is_full_url: Option<bool>,
    models_url: Option<String>,
    custom_user_agent: Option<String>,
    api_format: Option<String>,
    request_headers: Option<BTreeMap<String, String>>,
) -> Result<Vec<FetchedModel>, String> {
    let url = generic_models_url(
        &base_url,
        is_full_url.unwrap_or(false),
        models_url.as_deref(),
    )?;
    let client = crate::shared::http_client::build_http_client(
        None,
        Some("CCHub Model Fetcher"),
        std::time::Duration::from_secs(PROVIDER_MODELS_TIMEOUT_SECS),
    )?;
    let response = apply_model_fetch_headers(
        client.get(url),
        &api_key,
        api_format.as_deref(),
        custom_user_agent.as_deref(),
        request_headers.as_ref(),
    )
    .send()
    .await
    .map_err(|error| format!("Model request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Failed to read model response: {error}"))?;
    if !status.is_success() {
        let detail = body.chars().take(500).collect::<String>();
        return Err(format!("Model endpoint returned HTTP {status}: {detail}"));
    }
    let payload: Value = serde_json::from_str(&body)
        .map_err(|error| format!("Invalid model response JSON: {error}"))?;
    let models = parse_fetched_models(&payload);
    if models.is_empty() {
        return Err("Model endpoint returned no usable models".to_string());
    }
    Ok(models)
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
    let models = detailed::fetch_catalog(
        &client,
        &tool_id,
        &base_url,
        &api_key,
        use_full_url.unwrap_or(false),
        None,
        None,
        None,
    )
    .await?;
    Ok(models.into_iter().map(|model| model.id).collect())
}

const MODEL_CACHE_TTL_SECS: i64 = 600; // 10 minutes

fn model_cache_key(tool_id: &str, base_url: &str, api_key: &str, use_full_url: bool) -> String {
    use sha2::{Digest, Sha256};
    // Length-prefixed JSON keeps credentials and endpoint mode in distinct
    // identities. Only a digest is stored as the database key.
    let identity = serde_json::to_vec(&(
        tool_id.trim().to_ascii_lowercase(),
        base_url.trim(),
        api_key.trim(),
        use_full_url,
    ))
    .expect("string tuple serializes");
    format!("model_cache_v2_{:x}", Sha256::digest(identity))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ModelCache {
    models: Vec<String>,
    fetched_at: i64,
}

fn read_model_cache(conn: &rusqlite::Connection, key: &str) -> Option<Vec<String>> {
    let raw: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .ok()?;
    let cache: ModelCache = serde_json::from_str(&raw).ok()?;
    let now = chrono::Utc::now().timestamp();
    let age = now.checked_sub(cache.fetched_at)?;
    if !(0..=MODEL_CACHE_TTL_SECS).contains(&age) {
        return None;
    }
    Some(cache.models)
}

fn write_model_cache(conn: &rusqlite::Connection, key: &str, models: &[String]) {
    let cache = ModelCache {
        models: models.to_vec(),
        fetched_at: chrono::Utc::now().timestamp(),
    };
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, json],
        );
    }
}

#[tauri::command]
pub async fn fetch_provider_models_cached(
    tool_id: String,
    base_url: String,
    api_key: String,
    use_full_url: Option<bool>,
    force_refresh: Option<bool>,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    let cache_key = model_cache_key(&tool_id, &base_url, &api_key, use_full_url.unwrap_or(false));
    if api_key.trim().is_empty() {
        return Err("API key is required".into());
    }
    let force = force_refresh.unwrap_or(false);

    if !force {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = read_model_cache(&conn, &cache_key) {
            return Ok(cached);
        }
    }

    let models =
        fetch_provider_models(tool_id, base_url, api_key, use_full_url, db.clone()).await?;

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    write_model_cache(&conn, &cache_key, &models);

    Ok(models)
}

#[tauri::command]
pub fn get_cached_provider_models(
    tool_id: String,
    base_url: String,
    api_key: Option<String>,
    use_full_url: Option<bool>,
    db: State<'_, DbState>,
) -> Result<Option<Vec<String>>, String> {
    let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) else {
        return Ok(None);
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let key = model_cache_key(&tool_id, &base_url, &api_key, use_full_url.unwrap_or(false));
    Ok(read_model_cache(&conn, &key))
}

#[tauri::command]
pub async fn fetch_provider_models_detailed(
    tool_id: String,
    base_url: String,
    api_key: String,
    use_full_url: Option<bool>,
    custom_user_agent: Option<String>,
    request_headers: Option<BTreeMap<String, String>>,
    api_format: Option<String>,
    db: State<'_, DbState>,
) -> Result<Vec<ModelInfo>, String> {
    let client = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        build_provider_models_client(&conn)?
    };
    detailed::fetch_catalog(
        &client,
        &tool_id,
        &base_url,
        &api_key,
        use_full_url.unwrap_or(false),
        api_format.as_deref(),
        custom_user_agent.as_deref(),
        request_headers.as_ref(),
    )
    .await
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
            build_openai_models_url("https://proxy.example.com/v1/chat/completions", true).unwrap(),
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

    #[test]
    fn catalogs_are_scoped_to_credentials_and_endpoint_mode() {
        let key = super::model_cache_key("claude", "https://example.test", "first", false);
        assert_ne!(
            key,
            super::model_cache_key("claude", "https://example.test", "second", false)
        );
        assert_ne!(
            key,
            super::model_cache_key("claude", "https://example.test", "first", true)
        );
        assert!(!key.contains("first"));
    }

    #[test]
    fn future_and_extreme_cache_timestamps_are_not_valid() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT);")
            .unwrap();
        for timestamp in [chrono::Utc::now().timestamp() + 3600, i64::MIN] {
            let raw = serde_json::json!({"models":["a"],"fetched_at":timestamp}).to_string();
            conn.execute(
                "INSERT OR REPLACE INTO app_settings VALUES ('fixture', ?1)",
                [raw],
            )
            .unwrap();
            assert!(super::read_model_cache(&conn, "fixture").is_none());
        }
        super::write_model_cache(&conn, "fixture", &["valid".into()]);
        assert_eq!(
            super::read_model_cache(&conn, "fixture"),
            Some(vec!["valid".into()])
        );
    }

    #[test]
    fn authoritative_auth_and_user_agent_replace_custom_headers() {
        let custom = std::collections::BTreeMap::from([
            ("Authorization".into(), "Bearer wrong".into()),
            ("User-Agent".into(), "old".into()),
        ]);
        let request = super::apply_model_fetch_headers(
            reqwest::Client::new().get("http://example.test"),
            "correct",
            Some("openai"),
            Some("current"),
            Some(&custom),
        )
        .build()
        .unwrap();
        assert_eq!(request.headers().get_all("authorization").iter().count(), 1);
        assert_eq!(request.headers()["authorization"], "Bearer correct");
        assert_eq!(request.headers()["user-agent"], "current");
    }
}
