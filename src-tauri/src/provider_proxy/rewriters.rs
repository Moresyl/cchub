// 把 profile snapshot JSON 改写成"通过本地 proxy 转发"的版本：
// 把目标工具的 base URL 改成本地 proxy 端口，并塞入 PROXY_TOKEN 用于鉴权。
use serde_json::Value;

use super::{local_provider_proxy_tool_base_url, LOCAL_PROVIDER_PROXY_TOKEN};

pub(super) fn rewrite_claude_snapshot(snapshot: &str, port: u16) -> Result<String, String> {
    let mut parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Claude snapshot".to_string())?;
    let env = obj
        .entry("env")
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Claude env must be an object".to_string())?;

    env.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        Value::String(local_provider_proxy_tool_base_url(port, "claude")),
    );

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
