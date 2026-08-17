// 把 profile snapshot JSON 改写成"通过本地 proxy 转发"的版本：
// 把目标工具的 base URL 改成本地 proxy 端口，并塞入 PROXY_TOKEN 用于鉴权。
use serde_json::Value;

use super::{local_provider_proxy_tool_base_url, LOCAL_PROVIDER_PROXY_TOKEN};

pub(super) fn rewrite_claude_snapshot(snapshot: &str, port: u16) -> Result<String, String> {
    let mut parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Claude snapshot".to_string())?;
    let managed_oauth = obj
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("providerType"))
        .and_then(Value::as_str)
        .is_some_and(|provider_type| {
            matches!(
                provider_type,
                "github_copilot" | "codex_oauth" | "xai_oauth"
            )
        });
    let explicit_api_key = obj
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("authField"))
        .and_then(Value::as_str)
        == Some("ANTHROPIC_API_KEY");
    let env = obj
        .entry("env")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Claude env must be an object".to_string())?;

    env.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        Value::String(local_provider_proxy_tool_base_url(port, "claude")),
    );

    if managed_oauth && !explicit_api_key {
        env.remove("ANTHROPIC_API_KEY");
        env.remove("ANTHROPIC_AUTH_TOKEN");
    }
    let auth_key = if env.contains_key("ANTHROPIC_API_KEY") {
        "ANTHROPIC_API_KEY"
    } else {
        "ANTHROPIC_AUTH_TOKEN"
    };
    env.insert(
        auth_key.to_string(),
        Value::String(LOCAL_PROVIDER_PROXY_TOKEN.to_string()),
    );

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

pub(super) fn rewrite_codex_snapshot(snapshot: &str, port: u16) -> Result<String, String> {
    let mut parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Codex snapshot".to_string())?;
    let auth = obj
        .entry("auth")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Codex auth must be an object".to_string())?;
    auth.insert(
        "OPENAI_API_KEY".to_string(),
        Value::String(LOCAL_PROVIDER_PROXY_TOKEN.to_string()),
    );

    let config_text = obj
        .get("config")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let rewritten = rewrite_codex_config_base_url(
        config_text,
        &local_provider_proxy_tool_base_url(port, "codex"),
    );
    obj.insert("config".to_string(), Value::String(rewritten));

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

pub(super) fn rewrite_codex_config_base_url(content: &str, base_url: &str) -> String {
    let parsed = content.parse::<toml_edit::DocumentMut>();
    if let Ok(mut doc) = parsed {
        let provider_name = doc
            .get("model_provider")
            .and_then(|value| value.as_str())
            .unwrap_or("custom")
            .to_string();
        doc["model_providers"][provider_name.as_str()]["base_url"] = toml_edit::value(base_url);
        return doc.to_string();
    }

    let mut replaced = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        if line.trim_start().starts_with("base_url = ") {
            lines.push(format!("base_url = \"{base_url}\""));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !replaced {
        lines.push(format!("base_url = \"{base_url}\""));
    }
    lines.join("\n")
}

pub(super) fn rewrite_gemini_snapshot(snapshot: &str, port: u16) -> Result<String, String> {
    let mut parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Gemini snapshot".to_string())?;
    let env = obj
        .entry("env")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Gemini env must be an object".to_string())?;

    env.insert(
        "GOOGLE_GEMINI_BASE_URL".to_string(),
        Value::String(local_provider_proxy_tool_base_url(port, "gemini")),
    );
    env.insert(
        "GEMINI_API_KEY".to_string(),
        Value::String(LOCAL_PROVIDER_PROXY_TOKEN.to_string()),
    );

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

pub(super) fn rewrite_grok_snapshot(snapshot: &str, port: u16) -> Result<String, String> {
    let mut parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Grok Build snapshot".to_string())?;
    let proxy_url = local_provider_proxy_tool_base_url(port, "grokbuild");
    let config_text = obj
        .get("config")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut document = config_text
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_default();
    let default_model = document
        .get("models")
        .and_then(|value| value.get("default"))
        .and_then(|value| value.as_str())
        .or_else(|| obj.get("model").and_then(Value::as_str))
        .unwrap_or("grok-4.5")
        .to_string();
    let legacy_provider = document
        .get("model_provider")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    if let Some(provider_name) = legacy_provider {
        let provider = &mut document["model_providers"][provider_name];
        if provider.is_none() {
            *provider = toml_edit::Item::Table(toml_edit::Table::new());
        }
        provider["base_url"] = toml_edit::value(proxy_url);
        provider["api_key"] = toml_edit::value(LOCAL_PROVIDER_PROXY_TOKEN);
    } else {
        document["models"]["default"] = toml_edit::value(default_model.clone());
        let model = &mut document["model"][default_model];
        if model.is_none() {
            *model = toml_edit::Item::Table(toml_edit::Table::new());
        }
        model["model"] = toml_edit::value(
            obj.get("model")
                .and_then(Value::as_str)
                .unwrap_or("grok-4.5"),
        );
        model["base_url"] = toml_edit::value(proxy_url);
        model["api_backend"] = toml_edit::value(
            model
                .get("api_backend")
                .and_then(|value| value.as_str())
                .unwrap_or("responses"),
        );
        if model.get("context_window").is_none() {
            model["context_window"] = toml_edit::value(500_000_i64);
        }
        model["api_key"] = toml_edit::value(LOCAL_PROVIDER_PROXY_TOKEN);
    }
    obj.insert("config".to_string(), Value::String(document.to_string()));

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

pub(super) fn rewrite_openclaw_snapshot(snapshot: &str, port: u16) -> Result<String, String> {
    let mut parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid OpenClaw snapshot".to_string())?;
    obj.insert(
        "baseUrl".to_string(),
        Value::String(local_provider_proxy_tool_base_url(port, "openclaw")),
    );
    obj.insert(
        "apiKey".to_string(),
        Value::String(LOCAL_PROVIDER_PROXY_TOKEN.to_string()),
    );
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

pub(super) fn rewrite_hermes_snapshot(snapshot: &str, port: u16) -> Result<String, String> {
    let mut parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Hermes snapshot".to_string())?;
    let config = obj
        .entry("config")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Hermes config must be an object".to_string())?;
    let model = config
        .entry("model")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Hermes model must be an object".to_string())?;
    model.insert(
        "base_url".to_string(),
        Value::String(local_provider_proxy_tool_base_url(port, "hermes")),
    );

    let env_key = obj
        .get("metadata")
        .and_then(|value| value.get("hermesApiKeyEnv"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "OPENROUTER_API_KEY".to_string());
    let env = obj
        .entry("env")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Hermes env must be an object".to_string())?;
    env.insert(
        env_key,
        Value::String(LOCAL_PROVIDER_PROXY_TOKEN.to_string()),
    );

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

pub(super) fn rewrite_opencode_snapshot(snapshot: &str, port: u16) -> Result<String, String> {
    let mut parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid OpenCode snapshot".to_string())?;
    let options = obj
        .entry("options")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "OpenCode options must be an object".to_string())?;

    options.insert(
        "baseURL".to_string(),
        Value::String(local_provider_proxy_tool_base_url(port, "opencode")),
    );
    options.insert(
        "apiKey".to_string(),
        Value::String(LOCAL_PROVIDER_PROXY_TOKEN.to_string()),
    );

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{rewrite_claude_snapshot, rewrite_grok_snapshot};

    #[test]
    fn managed_oauth_takeover_uses_auth_token_and_clears_stale_api_key() {
        let snapshot = r#"{"env":{"ANTHROPIC_API_KEY":"old-key"},"metadata":{"providerType":"github_copilot"}}"#;
        let rewritten = rewrite_claude_snapshot(snapshot, 3456).unwrap();
        let value = serde_json::from_str::<serde_json::Value>(&rewritten).unwrap();
        let env = value
            .get("env")
            .and_then(|value| value.as_object())
            .unwrap();
        assert!(!env.contains_key("ANTHROPIC_API_KEY"));
        assert_eq!(
            env.get("ANTHROPIC_AUTH_TOKEN")
                .and_then(|value| value.as_str()),
            Some("cchub-local-proxy")
        );
    }

    #[test]
    fn managed_oauth_explicit_api_key_mode_is_preserved() {
        let snapshot = r#"{"env":{"ANTHROPIC_API_KEY":"old-key"},"metadata":{"providerType":"codex_oauth","authField":"ANTHROPIC_API_KEY"}}"#;
        let rewritten = rewrite_claude_snapshot(snapshot, 3456).unwrap();
        let value = serde_json::from_str::<serde_json::Value>(&rewritten).unwrap();
        let env = value
            .get("env")
            .and_then(|value| value.as_object())
            .unwrap();
        assert_eq!(
            env.get("ANTHROPIC_API_KEY")
                .and_then(|value| value.as_str()),
            Some("cchub-local-proxy")
        );
        assert!(!env.contains_key("ANTHROPIC_AUTH_TOKEN"));
    }

    #[test]
    fn rewrites_grok_native_config_to_local_proxy() {
        let snapshot = r#"{"config":"[models]\ndefault = \"grok-4.5\"\n\n[model.\"grok-4.5\"]\nmodel = \"grok-4.5\"\nbase_url = \"https://api.x.ai/v1\"\napi_backend = \"responses\""}"#;
        let rewritten = rewrite_grok_snapshot(snapshot, 4567).unwrap();
        let config = serde_json::from_str::<serde_json::Value>(&rewritten)
            .unwrap()
            .get("config")
            .and_then(|value| value.as_str())
            .unwrap()
            .to_string();
        assert!(config.contains("http://127.0.0.1:4567/proxy/grokbuild"));
        assert!(config.contains("api_key = \"cchub-local-proxy\""));
    }

    #[test]
    fn rewrites_legacy_model_provider_config_without_dropping_shape() {
        let snapshot = r#"{"model":"grok-4.5","config":"model_provider = \"custom\"\nmodel = \"grok-4.5\"\n\n[model_providers.custom]\nbase_url = \"https://example.com/v1\"\nwire_api = \"responses\""}"#;
        let rewritten = rewrite_grok_snapshot(snapshot, 4568).unwrap();
        assert!(rewritten.contains("[model_providers.custom]"));
        assert!(rewritten.contains("http://127.0.0.1:4568/proxy/grokbuild"));
        assert!(!rewritten.contains("[models]"));
    }
}
