#![allow(clippy::too_many_arguments)]

use super::super::types::*;
use super::*;

pub fn read_codex_structured_config_from_content(
    content: &str,
    api_key: String,
) -> CodexTomlStructuredConfig {
    let model_provider =
        parse_toml_assignment(content, "model_provider").unwrap_or_else(|| "custom".to_string());
    let provider_section = format!("model_providers.{model_provider}");

    let mcp_servers = content
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|doc| {
            doc.get("mcp_servers")
                .and_then(|item| item.as_table())
                .map(|table| {
                    table
                        .iter()
                        .map(|(key, _)| key.to_string())
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or_default();

    let malformed_mcp_servers = content
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|doc| doc.get("mcp_servers").map(|item| !item.is_table()))
        .unwrap_or(false);

    CodexTomlStructuredConfig {
        model_provider: model_provider.clone(),
        provider_label: parse_toml_section_assignment(content, &provider_section, "name")
            .unwrap_or_else(|| model_provider.clone()),
        base_url: parse_toml_section_assignment(content, &provider_section, "base_url")
            .unwrap_or_default(),
        wire_api: parse_toml_section_assignment(content, &provider_section, "wire_api")
            .unwrap_or_else(|| "responses".to_string()),
        model: parse_toml_assignment(content, "model").unwrap_or_default(),
        reasoning_effort: parse_toml_assignment(content, "model_reasoning_effort")
            .unwrap_or_else(|| "medium".to_string()),
        personality: parse_toml_assignment(content, "personality")
            .unwrap_or_else(|| "pragmatic".to_string()),
        disable_response_storage: parse_toml_assignment(content, "disable_response_storage")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        model_context_window: parse_toml_assignment(content, "model_context_window")
            .unwrap_or_default(),
        model_auto_compact_token_limit: parse_toml_assignment(
            content,
            "model_auto_compact_token_limit",
        )
        .unwrap_or_default(),
        api_key,
        mcp_servers,
        malformed_mcp_servers,
    }
}

pub fn write_codex_structured_config_to_text(
    raw_toml: &str,
    config: &CodexTomlStructuredConfig,
) -> String {
    let mut doc = raw_toml
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());

    let provider_name =
        normalized_non_empty(&config.model_provider).unwrap_or_else(|| "custom".to_string());
    let provider_label =
        normalized_non_empty(&config.provider_label).unwrap_or_else(|| provider_name.clone());
    let wire_api =
        normalized_non_empty(&config.wire_api).unwrap_or_else(|| "responses".to_string());
    let reasoning_effort =
        normalized_non_empty(&config.reasoning_effort).unwrap_or_else(|| "medium".to_string());
    let personality =
        normalized_non_empty(&config.personality).unwrap_or_else(|| "pragmatic".to_string());

    doc["model_provider"] = toml_edit::value(provider_name.clone());
    doc["model"] = toml_edit::value(config.model.trim());
    doc["model_reasoning_effort"] = toml_edit::value(reasoning_effort);
    doc["personality"] = toml_edit::value(personality);
    doc["disable_response_storage"] = toml_edit::value(config.disable_response_storage);

    if let Some(context_window) = normalize_integer_like(&config.model_context_window) {
        doc["model_context_window"] = toml_edit::value(context_window);
    } else {
        doc.as_table_mut().remove("model_context_window");
    }

    if let Some(compact_limit) = normalize_integer_like(&config.model_auto_compact_token_limit) {
        doc["model_auto_compact_token_limit"] = toml_edit::value(compact_limit);
    } else {
        doc.as_table_mut().remove("model_auto_compact_token_limit");
    }

    doc["model_providers"][provider_name.as_str()]["name"] = toml_edit::value(provider_label);
    doc["model_providers"][provider_name.as_str()]["base_url"] =
        toml_edit::value(config.base_url.trim());
    doc["model_providers"][provider_name.as_str()]["wire_api"] = toml_edit::value(wire_api);
    doc["model_providers"][provider_name.as_str()]["requires_openai_auth"] = toml_edit::value(true);

    let malformed_mcp_servers = doc
        .get("mcp_servers")
        .map(|item| !item.is_table())
        .unwrap_or(false);
    if malformed_mcp_servers {
        doc.as_table_mut().remove("mcp_servers");
    }
    if doc.get("mcp_servers").is_none() {
        doc["mcp_servers"] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    doc.to_string()
}

fn apply_common_config_to_claude_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Claude snapshot".to_string())?;

    if snippet.hide_attribution {
        obj.insert(
            "attribution".to_string(),
            serde_json::json!({ "commit": "", "pr": "" }),
        );
    }
    if snippet.effort_level_high {
        obj.insert(
            "effortLevel".to_string(),
            serde_json::Value::String("high".to_string()),
        );
    }

    let env = obj
        .entry("env")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Claude env must be an object".to_string())?;
    if snippet.enable_teammates {
        env.insert(
            "CLAUDE_CODE_ENABLE_TEAMMATES".to_string(),
            serde_json::json!("true"),
        );
        env.insert(
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".to_string(),
            serde_json::json!("1"),
        );
    }
    if snippet.enable_tool_search {
        env.insert("ENABLE_TOOL_SEARCH".to_string(), serde_json::json!("true"));
    }
    for (key, value) in &snippet.custom_values {
        env.insert(key.clone(), serde_json::json!(value));
    }

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

fn apply_common_config_to_codex_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Codex snapshot".to_string())?;
    let current_config = obj
        .get("config")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let current_config = current_config.to_string();
    let current_api_key = obj
        .get("auth")
        .and_then(|value| value.get("OPENAI_API_KEY"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let mut structured =
        read_codex_structured_config_from_content(&current_config, current_api_key);
    if snippet.effort_level_high {
        structured.reasoning_effort = "high".to_string();
    }
    for (key, value) in &snippet.custom_values {
        if key == "model_auto_compact_token_limit" {
            structured.model_auto_compact_token_limit = value.clone();
        }
    }
    let mut next_toml = write_codex_structured_config_to_text(&current_config, &structured);
    for (key, value) in &snippet.custom_values {
        if key == "model_auto_compact_token_limit" {
            continue;
        }
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            continue;
        }
        let mut doc = next_toml
            .parse::<toml_edit::DocumentMut>()
            .unwrap_or_else(|_| toml_edit::DocumentMut::new());
        if let Some(integer) = normalize_integer_like(value) {
            doc[normalized_key] = toml_edit::value(integer);
        } else if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
            doc[normalized_key] = toml_edit::value(value.eq_ignore_ascii_case("true"));
        } else {
            doc[normalized_key] = toml_edit::value(value.as_str());
        }
        next_toml = doc.to_string();
    }
    obj.insert("config".to_string(), serde_json::Value::String(next_toml));
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

fn apply_common_config_to_gemini_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Gemini snapshot".to_string())?;
    let env = obj
        .entry("env")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Gemini env must be an object".to_string())?;
    for (key, value) in &snippet.custom_values {
        env.insert(key.clone(), serde_json::json!(value));
    }
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

#[allow(dead_code)]
fn apply_common_config_snippet_to_snapshot(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
) -> Result<String, String> {
    let snippet = read_common_config_snippet_from_conn(conn, tool_id)?;
    if !common_config_snippet_has_payload(&snippet) {
        return Ok(snapshot.to_string());
    }

    match tool_id {
        "claude" => apply_common_config_to_claude_snapshot(snapshot, &snippet),
        "codex" => apply_common_config_to_codex_snapshot(snapshot, &snippet),
        "gemini" => apply_common_config_to_gemini_snapshot(snapshot, &snippet),
        _ => Ok(snapshot.to_string()),
    }
}

pub fn join_api_endpoint(base_url: &str, suffix: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed_base = base_url.trim().trim_end_matches('/');
    let trimmed_suffix = suffix.trim_start_matches('/');
    if trimmed_base.ends_with(trimmed_suffix) {
        trimmed_base.to_string()
    } else {
        format!("{trimmed_base}/{trimmed_suffix}")
    }
}

pub fn build_claude_messages_endpoint(base_url: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/messages") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/messages")
    } else {
        format!("{trimmed}/v1/messages")
    }
}

pub fn build_gemini_stream_endpoint(base_url: &str, model: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.contains(":streamGenerateContent") {
        trimmed.to_string()
    } else if trimmed.ends_with(&format!("/models/{model}")) {
        format!("{trimmed}:streamGenerateContent?alt=sse")
    } else {
        format!("{trimmed}/models/{model}:streamGenerateContent?alt=sse")
    }
}
