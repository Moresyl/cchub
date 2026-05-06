mod parser;

use std::sync::Mutex;
use std::time::Duration;

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;

pub use parser::parse_deeplink_url;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkImportRequest {
    pub version: String,
    pub resource: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub haiku_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sonnet_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opus_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex_wire_api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex_reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apps: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_auto_interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkErrorPayload {
    pub url: String,
    pub error: String,
}

#[derive(Default)]
pub struct DeepLinkState {
    imports: Mutex<Vec<DeepLinkImportRequest>>,
    errors: Mutex<Vec<DeepLinkErrorPayload>>,
}

impl DeepLinkState {
    pub fn enqueue_import(&self, request: DeepLinkImportRequest) -> Result<(), AppError> {
        let mut queue = self.imports.lock()?;
        queue.push(request);
        if queue.len() > 16 {
            let drain_count = queue.len().saturating_sub(16);
            queue.drain(0..drain_count);
        }
        Ok(())
    }

    pub fn enqueue_error(&self, payload: DeepLinkErrorPayload) -> Result<(), AppError> {
        let mut queue = self.errors.lock()?;
        queue.push(payload);
        if queue.len() > 16 {
            let drain_count = queue.len().saturating_sub(16);
            queue.drain(0..drain_count);
        }
        Ok(())
    }

    pub fn take_imports(&self) -> Result<Vec<DeepLinkImportRequest>, AppError> {
        let mut queue = self.imports.lock()?;
        Ok(std::mem::take(&mut *queue))
    }

    pub fn take_errors(&self) -> Result<Vec<DeepLinkErrorPayload>, AppError> {
        let mut queue = self.errors.lock()?;
        Ok(std::mem::take(&mut *queue))
    }
}

pub async fn merge_deeplink_request(
    mut request: DeepLinkImportRequest,
) -> Result<DeepLinkImportRequest, AppError> {
    if request.resource != "provider" {
        return Ok(request);
    }

    let config_text = if let Some(config_url) = request
        .config_url
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        fetch_remote_config(config_url).await?
    } else if let Some(config) = request
        .config
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        decode_text_payload(config)?
    } else {
        String::new()
    };

    if config_text.is_empty() {
        if request
            .homepage
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            request.homepage = infer_homepage_from_endpoint(request.endpoint.as_deref());
        }
        return Ok(request);
    }

    let app_id = request
        .app
        .clone()
        .ok_or_else(|| AppError::Custom("Missing app field for provider deep link".to_string()))?;

    match app_id.as_str() {
        "claude" => merge_claude_config(&mut request, &config_text)?,
        "codex" => merge_codex_config(&mut request, &config_text)?,
        "gemini" => merge_gemini_config(&mut request, &config_text)?,
        "openclaw" => merge_openclaw_config(&mut request, &config_text)?,
        "hermes" => merge_hermes_config(&mut request, &config_text)?,
        "opencode" => merge_opencode_config(&mut request, &config_text)?,
        other => {
            return Err(AppError::Custom(format!(
                "Unsupported provider app in deep link config merge: {other}"
            )));
        }
    }

    if request
        .homepage
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        request.homepage = infer_homepage_from_endpoint(request.endpoint.as_deref());
    }

    Ok(request)
}

pub fn decode_text_payload(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    if looks_like_plain_text(trimmed) {
        return Ok(trimmed.to_string());
    }

    for engine in [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD] {
        if let Ok(decoded) = engine.decode(trimmed) {
            if let Ok(text) = String::from_utf8(decoded) {
                return Ok(text);
            }
        }
    }

    Ok(trimmed.to_string())
}

fn looks_like_plain_text(value: &str) -> bool {
    let trimmed = value.trim_start();
    trimmed.starts_with('{')
        || trimmed.starts_with('[')
        || trimmed.starts_with('#')
        || trimmed.contains('\n')
        || trimmed.contains("\r\n")
        || trimmed.contains(" = ")
        || trimmed.contains("=\"")
        || trimmed.contains("[model_providers")
}

async fn fetch_remote_config(config_url: &str) -> Result<String, AppError> {
    let response = crate::shared::http_client::build_http_client(
        None,
        Some("CCHub/1.3"),
        Duration::from_secs(15),
    )
    .map_err(|error| AppError::Custom(format!("Failed to build HTTP client: {error}")))?
    .get(config_url)
    .send()
    .await
    .map_err(|error| AppError::Custom(format!("Failed to fetch config URL: {error}")))?;

    let response = response
        .error_for_status()
        .map_err(|error| AppError::Custom(format!("Config URL returned error: {error}")))?;

    response
        .text()
        .await
        .map_err(|error| AppError::Custom(format!("Failed to read config URL response: {error}")))
}

fn merge_claude_config(
    request: &mut DeepLinkImportRequest,
    config_text: &str,
) -> Result<(), AppError> {
    let config = parse_json_value(config_text)?;
    let env = config
        .get("env")
        .and_then(Value::as_object)
        .or_else(|| config.as_object())
        .ok_or_else(|| AppError::Custom("Claude config must be a JSON object".to_string()))?;

    if request
        .api_key
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(api_key) = env
            .get("ANTHROPIC_AUTH_TOKEN")
            .or_else(|| env.get("ANTHROPIC_API_KEY"))
            .and_then(Value::as_str)
        {
            request.api_key = Some(api_key.to_string());
        }
    }

    if request.auth_field.is_none() {
        if env.get("ANTHROPIC_API_KEY").is_some() {
            request.auth_field = Some("ANTHROPIC_API_KEY".to_string());
        } else if env.get("ANTHROPIC_AUTH_TOKEN").is_some() {
            request.auth_field = Some("ANTHROPIC_AUTH_TOKEN".to_string());
        }
    }

    if request
        .endpoint
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(endpoint) = env.get("ANTHROPIC_BASE_URL").and_then(Value::as_str) {
            request.endpoint = Some(endpoint.to_string());
        }
    }

    if request.model.is_none() {
        request.model = env
            .get("ANTHROPIC_MODEL")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if request.haiku_model.is_none() {
        request.haiku_model = env
            .get("ANTHROPIC_DEFAULT_HAIKU_MODEL")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if request.sonnet_model.is_none() {
        request.sonnet_model = env
            .get("ANTHROPIC_DEFAULT_SONNET_MODEL")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if request.opus_model.is_none() {
        request.opus_model = env
            .get("ANTHROPIC_DEFAULT_OPUS_MODEL")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if request.api_format.is_none() {
        request.api_format = env
            .get("ANTHROPIC_API_FORMAT")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }

    Ok(())
}

fn merge_codex_config(
    request: &mut DeepLinkImportRequest,
    config_text: &str,
) -> Result<(), AppError> {
    let config_json = parse_json_value(config_text).ok();

    if request
        .api_key
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(api_key) = config_json
            .as_ref()
            .and_then(|config| config.get("auth"))
            .and_then(|auth| auth.get("OPENAI_API_KEY"))
            .and_then(Value::as_str)
        {
            request.api_key = Some(api_key.to_string());
        }
    }

    let config_toml_text = config_json
        .as_ref()
        .and_then(|config| config.get("config"))
        .and_then(Value::as_str)
        .unwrap_or(config_text);

    let toml_value = toml::from_str::<toml::Value>(config_toml_text)
        .map_err(|error| AppError::Custom(format!("Invalid Codex config TOML: {error}")))?;

    if request
        .endpoint
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(base_url) = extract_codex_base_url(&toml_value) {
            request.endpoint = Some(base_url);
        }
    }
    if request.model.is_none() {
        request.model = toml_value
            .get("model")
            .and_then(toml::Value::as_str)
            .map(ToString::to_string);
    }
    if request.codex_reasoning_effort.is_none() {
        request.codex_reasoning_effort = toml_value
            .get("model_reasoning_effort")
            .and_then(toml::Value::as_str)
            .map(ToString::to_string);
    }
    if request.codex_wire_api.is_none() {
        request.codex_wire_api = extract_codex_wire_api(&toml_value);
    }

    Ok(())
}

fn merge_gemini_config(
    request: &mut DeepLinkImportRequest,
    config_text: &str,
) -> Result<(), AppError> {
    let config = parse_json_value(config_text)?;
    let env = config
        .get("env")
        .and_then(Value::as_object)
        .or_else(|| config.as_object())
        .ok_or_else(|| AppError::Custom("Gemini config must be a JSON object".to_string()))?;

    if request
        .api_key
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(api_key) = env.get("GEMINI_API_KEY").and_then(Value::as_str) {
            request.api_key = Some(api_key.to_string());
        }
    }

    if request
        .endpoint
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(endpoint) = env
            .get("GOOGLE_GEMINI_BASE_URL")
            .or_else(|| env.get("GEMINI_BASE_URL"))
            .and_then(Value::as_str)
        {
            request.endpoint = Some(endpoint.to_string());
        }
    }

    if request.model.is_none() {
        request.model = env
            .get("GEMINI_MODEL")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }

    Ok(())
}

fn merge_openclaw_config(
    request: &mut DeepLinkImportRequest,
    config_text: &str,
) -> Result<(), AppError> {
    let config = parse_json_value(config_text)?;

    if request
        .api_key
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(api_key) = config
            .get("apiKey")
            .or_else(|| config.get("api_key"))
            .and_then(Value::as_str)
        {
            request.api_key = Some(api_key.to_string());
        }
    }

    if request
        .endpoint
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(endpoint) = config
            .get("baseUrl")
            .or_else(|| config.get("base_url"))
            .and_then(Value::as_str)
        {
            request.endpoint = Some(endpoint.to_string());
        }
    }

    if request.model.is_none() {
        request.model = config
            .get("models")
            .and_then(Value::as_array)
            .and_then(|models| models.first())
            .and_then(|model| model.get("id"))
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }

    if request.api_protocol.is_none() {
        request.api_protocol = config
            .get("api")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }

    Ok(())
}

fn merge_opencode_config(
    request: &mut DeepLinkImportRequest,
    config_text: &str,
) -> Result<(), AppError> {
    let config = parse_json_value(config_text)?;

    if request.npm.is_none() {
        request.npm = config
            .get("npm")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }

    if request
        .api_key
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(api_key) = config
            .get("options")
            .and_then(|value| value.get("apiKey"))
            .and_then(Value::as_str)
        {
            request.api_key = Some(api_key.to_string());
        }
    }

    if request
        .endpoint
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(endpoint) = config
            .get("options")
            .and_then(|value| value.get("baseURL"))
            .or_else(|| config.get("options").and_then(|value| value.get("baseUrl")))
            .and_then(Value::as_str)
        {
            request.endpoint = Some(endpoint.to_string());
        }
    }

    if request.model.is_none() {
        request.model = config
            .get("models")
            .and_then(Value::as_object)
            .and_then(|models| models.keys().next().cloned());
    }

    Ok(())
}

fn merge_hermes_config(
    request: &mut DeepLinkImportRequest,
    config_text: &str,
) -> Result<(), AppError> {
    let config = parse_json_value(config_text)?;
    let config_obj = config
        .get("config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let model = config_obj
        .get("model")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let env = config
        .get("env")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    if request
        .endpoint
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(endpoint) = model.get("base_url").and_then(Value::as_str) {
            request.endpoint = Some(endpoint.to_string());
        }
    }

    if request.model.is_none() {
        request.model = model
            .get("default")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }

    if request.notes.is_none() {
        request.notes = model
            .get("provider")
            .and_then(Value::as_str)
            .map(|provider| format!("provider={provider}"));
    }

    if request
        .api_key
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        if let Some(env_key) = config
            .get("metadata")
            .and_then(|value| value.get("hermesApiKeyEnv"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            if let Some(api_key) = env.get(env_key).and_then(Value::as_str) {
                request.api_key = Some(api_key.to_string());
            }
        }
    }

    Ok(())
}

fn parse_json_value(config_text: &str) -> Result<Value, AppError> {
    serde_json::from_str(config_text)
        .map_err(|error| AppError::Custom(format!("Invalid JSON config: {error}")))
}

fn extract_codex_base_url(toml_value: &toml::Value) -> Option<String> {
    let provider_key = toml_value
        .get("model_provider")
        .and_then(toml::Value::as_str)
        .map(ToString::to_string);

    if let Some(key) = provider_key {
        if let Some(base_url) = toml_value
            .get("model_providers")
            .and_then(toml::Value::as_table)
            .and_then(|providers| providers.get(&key))
            .and_then(|provider| provider.get("base_url"))
            .and_then(toml::Value::as_str)
        {
            return Some(base_url.to_string());
        }
    }

    toml_value
        .get("model_providers")
        .and_then(toml::Value::as_table)
        .and_then(|providers| {
            providers
                .values()
                .find_map(|provider| provider.get("base_url").and_then(toml::Value::as_str))
        })
        .map(ToString::to_string)
}

fn extract_codex_wire_api(toml_value: &toml::Value) -> Option<String> {
    let provider_key = toml_value
        .get("model_provider")
        .and_then(toml::Value::as_str)
        .map(ToString::to_string);

    if let Some(key) = provider_key {
        if let Some(wire_api) = toml_value
            .get("model_providers")
            .and_then(toml::Value::as_table)
            .and_then(|providers| providers.get(&key))
            .and_then(|provider| provider.get("wire_api"))
            .and_then(toml::Value::as_str)
        {
            return Some(wire_api.to_string());
        }
    }

    toml_value
        .get("model_providers")
        .and_then(toml::Value::as_table)
        .and_then(|providers| {
            providers
                .values()
                .find_map(|provider| provider.get("wire_api").and_then(toml::Value::as_str))
        })
        .map(ToString::to_string)
}

pub fn infer_homepage_from_endpoint(endpoint: Option<&str>) -> Option<String> {
    let endpoint = endpoint
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let parsed = url::Url::parse(endpoint).ok()?;
    let host = parsed.host_str()?;
    Some(format!("{}://{}", parsed.scheme(), host))
}
