use std::collections::HashMap;

use url::Url;

use crate::deeplink::DeepLinkImportRequest;
use crate::error::AppError;

pub fn parse_deeplink_url(url_str: &str) -> Result<DeepLinkImportRequest, AppError> {
    let url = Url::parse(url_str)
        .map_err(|error| AppError::Custom(format!("Invalid deep link URL: {error}")))?;

    let scheme = url.scheme();
    if scheme != "cchub" && scheme != "ccswitch" {
        return Err(AppError::Custom(format!(
            "Unsupported deep link scheme: {scheme}"
        )));
    }

    let version = url
        .host_str()
        .ok_or_else(|| AppError::Custom("Missing deep link protocol version".to_string()))?
        .to_string();
    if version != "v1" {
        return Err(AppError::Custom(format!(
            "Unsupported deep link protocol version: {version}"
        )));
    }

    if url.path() != "/import" {
        return Err(AppError::Custom(format!(
            "Unsupported deep link path: {}",
            url.path()
        )));
    }

    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let resource = required_param(&params, "resource")?;

    match resource.as_str() {
        "provider" => parse_provider(params, version, resource),
        "prompt" => parse_prompt(params, version, resource),
        "mcp" => parse_mcp(params, version, resource),
        "skill" => parse_skill(params, version, resource),
        other => Err(AppError::Custom(format!(
            "Unsupported deep link resource: {other}"
        ))),
    }
}

fn parse_provider(
    params: HashMap<String, String>,
    version: String,
    resource: String,
) -> Result<DeepLinkImportRequest, AppError> {
    let app = required_param(&params, "app")?;
    validate_provider_app(&app)?;

    Ok(DeepLinkImportRequest {
        version,
        resource,
        app: Some(app),
        name: Some(required_param(&params, "name")?),
        enabled: parse_bool(params.get("enabled")),
        homepage: optional_param(&params, "homepage"),
        endpoint: optional_param(&params, "endpoint"),
        api_key: optional_param(&params, "apiKey"),
        icon: optional_param(&params, "icon"),
        model: optional_param(&params, "model"),
        notes: optional_param(&params, "notes"),
        haiku_model: optional_param(&params, "haikuModel"),
        sonnet_model: optional_param(&params, "sonnetModel"),
        opus_model: optional_param(&params, "opusModel"),
        api_format: optional_param(&params, "apiFormat"),
        auth_field: optional_param(&params, "authField"),
        codex_wire_api: optional_param(&params, "codexWireApi"),
        codex_reasoning_effort: optional_param(&params, "codexReasoningEffort"),
        api_protocol: optional_param(&params, "apiProtocol"),
        npm: optional_param(&params, "npm"),
        content: None,
        description: None,
        apps: None,
        repo: None,
        directory: None,
        branch: None,
        config: optional_param(&params, "config"),
        config_format: optional_param(&params, "configFormat"),
        config_url: optional_param(&params, "configUrl"),
        usage_enabled: parse_bool(params.get("usageEnabled")),
        usage_script: optional_param(&params, "usageScript"),
        usage_api_key: optional_param(&params, "usageApiKey"),
        usage_base_url: optional_param(&params, "usageBaseUrl"),
        usage_access_token: optional_param(&params, "usageAccessToken"),
        usage_user_id: optional_param(&params, "usageUserId"),
        usage_auto_interval: params
            .get("usageAutoInterval")
            .and_then(|value| value.parse::<u64>().ok()),
    })
}

fn parse_prompt(
    params: HashMap<String, String>,
    version: String,
    resource: String,
) -> Result<DeepLinkImportRequest, AppError> {
    let app = required_param(&params, "app")?;
    validate_provider_app(&app)?;

    Ok(DeepLinkImportRequest {
        version,
        resource,
        app: Some(app),
        name: Some(required_param(&params, "name")?),
        enabled: parse_bool(params.get("enabled")),
        homepage: None,
        endpoint: None,
        api_key: None,
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        api_format: None,
        auth_field: None,
        codex_wire_api: None,
        codex_reasoning_effort: None,
        api_protocol: None,
        npm: None,
        content: Some(required_param(&params, "content")?),
        description: optional_param(&params, "description"),
        apps: None,
        repo: None,
        directory: None,
        branch: None,
        config: None,
        config_format: None,
        config_url: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    })
}

fn parse_mcp(
    params: HashMap<String, String>,
    version: String,
    resource: String,
) -> Result<DeepLinkImportRequest, AppError> {
    let apps = required_param(&params, "apps")?;
    for app in apps
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        validate_mcp_app(app)?;
    }

    Ok(DeepLinkImportRequest {
        version,
        resource,
        app: None,
        name: optional_param(&params, "name"),
        enabled: parse_bool(params.get("enabled")),
        homepage: None,
        endpoint: None,
        api_key: None,
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        api_format: None,
        auth_field: None,
        codex_wire_api: None,
        codex_reasoning_effort: None,
        api_protocol: None,
        npm: None,
        content: None,
        description: None,
        apps: Some(apps),
        repo: None,
        directory: None,
        branch: None,
        config: Some(required_param(&params, "config")?),
        config_format: optional_param(&params, "configFormat"),
        config_url: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    })
}

fn parse_skill(
    params: HashMap<String, String>,
    version: String,
    resource: String,
) -> Result<DeepLinkImportRequest, AppError> {
    let repo = required_param(&params, "repo")?;
    if repo.split('/').count() != 2 {
        return Err(AppError::Custom(format!(
            "Invalid skill repository format: {repo}"
        )));
    }

    Ok(DeepLinkImportRequest {
        version,
        resource,
        app: None,
        name: optional_param(&params, "name"),
        enabled: None,
        homepage: None,
        endpoint: None,
        api_key: None,
        icon: None,
        model: None,
        notes: None,
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        api_format: None,
        auth_field: None,
        codex_wire_api: None,
        codex_reasoning_effort: None,
        api_protocol: None,
        npm: None,
        content: None,
        description: None,
        apps: None,
        repo: Some(repo),
        directory: optional_param(&params, "directory"),
        branch: optional_param(&params, "branch"),
        config: None,
        config_format: None,
        config_url: None,
        usage_enabled: None,
        usage_script: None,
        usage_api_key: None,
        usage_base_url: None,
        usage_access_token: None,
        usage_user_id: None,
        usage_auto_interval: None,
    })
}

fn required_param(params: &HashMap<String, String>, key: &str) -> Result<String, AppError> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| AppError::Custom(format!("Missing required deep link parameter: {key}")))
}

fn optional_param(params: &HashMap<String, String>, key: &str) -> Option<String> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_bool(value: Option<&String>) -> Option<bool> {
    value.and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    })
}

fn validate_provider_app(app: &str) -> Result<(), AppError> {
    match app {
        "claude" | "codex" | "gemini" | "opencode" | "openclaw" => Ok(()),
        other => Err(AppError::Custom(format!(
            "Unsupported provider app in deep link: {other}"
        ))),
    }
}

fn validate_mcp_app(app: &str) -> Result<(), AppError> {
    match app {
        "claude" | "codex" | "gemini" | "opencode" | "openclaw" => Ok(()),
        other => Err(AppError::Custom(format!(
            "Unsupported MCP target app in deep link: {other}"
        ))),
    }
}
