use super::{
    apply_model_fetch_headers, build_claude_models_url, build_gemini_models_url,
    build_openai_models_url,
    catalog::{merge_catalog, parse_catalog},
    ModelInfo, PROVIDER_MODELS_TIMEOUT_SECS,
};
use reqwest::{Client, Url};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use tokio::time::{timeout_at, Instant};

const MAX_CATALOG_BYTES: usize = 8 * 1024 * 1024;
const MAX_CATALOG_MODELS: usize = 10_000;
const MAX_CATALOG_PAGES: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Protocol {
    OpenAi,
    Anthropic,
    Gemini,
}

impl Protocol {
    fn resolve(tool: &str, format: Option<&str>) -> Result<Self, String> {
        if !matches!(
            tool,
            "claude" | "codex" | "gemini" | "grokbuild" | "openclaw" | "opencode" | "hermes"
        ) {
            return Err(format!("Fetching models is not supported for {tool}"));
        }
        match format.map(str::trim).filter(|value| !value.is_empty()) {
            Some("anthropic" | "anthropic-messages" | "@ai-sdk/anthropic") => Ok(Self::Anthropic),
            Some("gemini" | "google-generative-ai" | "@ai-sdk/google") => Ok(Self::Gemini),
            Some(
                "openai"
                | "openai_chat"
                | "openai_responses"
                | "openai-completions"
                | "openai-responses"
                | "@ai-sdk/openai"
                | "@ai-sdk/openai-compatible",
            ) => Ok(Self::OpenAi),
            Some(_) => Err("Model discovery is not supported for the selected API protocol".into()),
            None => Ok(match tool {
                "claude" => Self::Anthropic,
                "gemini" => Self::Gemini,
                _ => Self::OpenAi,
            }),
        }
    }

    fn format(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }

    fn url(self, base: &str, full: bool) -> Result<Url, String> {
        let raw = match self {
            Self::OpenAi => build_openai_models_url(base, full),
            Self::Anthropic => build_claude_models_url(base, full),
            Self::Gemini => build_gemini_models_url(base, full),
        }?;
        let url = Url::parse(&raw).map_err(|_| "Invalid models URL".to_string())?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(
                "Models URL must use HTTP(S) with a host and no embedded credentials".into(),
            );
        }
        Ok(url)
    }
}

fn next_cursor(
    payload: &Value,
    protocol: Protocol,
) -> Result<Option<(&'static str, String)>, String> {
    match protocol {
        Protocol::Gemini => match payload
            .get("nextPageToken")
            .filter(|value| !value.is_null())
        {
            None => Ok(None),
            Some(value) => value
                .as_str()
                .map(str::trim)
                .map(|s| (!s.is_empty()).then(|| ("pageToken", s.to_string())))
                .ok_or_else(|| "Model response contains an invalid page token".into()),
        },
        Protocol::Anthropic if payload.get("has_more").and_then(Value::as_bool) == Some(true) => {
            let id = payload
                .get("last_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or("Model response is missing its next-page cursor")?;
            Ok(Some(("after_id", id.to_string())))
        }
        _ => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn fetch_catalog(
    client: &Client,
    tool: &str,
    base: &str,
    key: &str,
    full: bool,
    format: Option<&str>,
    user_agent: Option<&str>,
    headers: Option<&BTreeMap<String, String>>,
) -> Result<Vec<ModelInfo>, String> {
    let deadline = Instant::now() + std::time::Duration::from_secs(PROVIDER_MODELS_TIMEOUT_SECS);
    fetch_catalog_until(
        client, tool, base, key, full, format, user_agent, headers, deadline,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn fetch_catalog_until(
    client: &Client,
    tool: &str,
    base: &str,
    key: &str,
    full: bool,
    format: Option<&str>,
    user_agent: Option<&str>,
    headers: Option<&BTreeMap<String, String>>,
    deadline: Instant,
) -> Result<Vec<ModelInfo>, String> {
    if key.trim().is_empty() {
        return Err("API key is required".into());
    }
    if key.trim().chars().any(char::is_control) {
        return Err("API key contains invalid control characters".into());
    }
    let protocol = Protocol::resolve(&tool.trim().to_ascii_lowercase(), format)?;
    let base_url = protocol.url(base, full)?;
    let mut url = base_url.clone();
    let mut cursors = HashSet::new();
    let mut models = Vec::new();
    let mut total_bytes = 0;
    for page in 0..MAX_CATALOG_PAGES {
        let request = apply_model_fetch_headers(
            client.get(url),
            key,
            Some(protocol.format()),
            user_agent,
            headers,
        );
        let mut response = timeout_at(deadline, request.send())
            .await
            .map_err(|_| "Model discovery timed out".to_string())?
            .map_err(|error| format!("Model request failed: {}", error.without_url()))?;
        if !response.status().is_success() {
            // Error bodies may echo request secrets. Never send those to the UI.
            return Err(format!(
                "Model endpoint returned HTTP {}",
                response.status()
            ));
        }
        let mut body = Vec::new();
        while let Some(chunk) = timeout_at(deadline, response.chunk())
            .await
            .map_err(|_| "Model discovery timed out".to_string())?
            .map_err(|error| format!("Failed to read model response: {}", error.without_url()))?
        {
            total_bytes += chunk.len();
            if total_bytes > MAX_CATALOG_BYTES {
                return Err("Model catalog exceeds the response size limit".into());
            }
            body.extend_from_slice(&chunk);
        }
        let payload: Value = serde_json::from_slice(&body)
            .map_err(|_| "Model endpoint returned invalid JSON".to_string())?;
        models.extend(parse_catalog(&payload, protocol == Protocol::Gemini)?);
        if models.len() > MAX_CATALOG_MODELS {
            return Err("Model catalog exceeds the model count limit".into());
        }
        let Some((name, cursor)) = next_cursor(&payload, protocol)? else {
            return Ok(merge_catalog(models));
        };
        if !cursors.insert(cursor.clone()) {
            return Err("Model endpoint repeated a page cursor".into());
        }
        if page + 1 == MAX_CATALOG_PAGES {
            return Err("Model catalog exceeds the page limit".into());
        }
        url = base_url.clone();
        url.query_pairs_mut().append_pair(name, &cursor);
    }
    unreachable!("the last page returns a limit error")
}

#[cfg(test)]
mod tests;
