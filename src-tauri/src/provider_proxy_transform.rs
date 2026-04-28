use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub fn is_openai_o_series(model: &str) -> bool {
    model.len() > 1
        && model.starts_with('o')
        && model.as_bytes().get(1).is_some_and(|b| b.is_ascii_digit())
}

pub fn supports_reasoning_effort(model: &str) -> bool {
    is_openai_o_series(model)
        || model
            .to_lowercase()
            .strip_prefix("gpt-")
            .and_then(|rest| rest.chars().next())
            .is_some_and(|c| c.is_ascii_digit() && c >= '5')
}

pub fn resolve_reasoning_effort(body: &Value) -> Option<&'static str> {
    if let Some(effort) = body
        .pointer("/output_config/effort")
        .and_then(|v| v.as_str())
    {
        return match effort {
            "low" => Some("low"),
            "medium" => Some("medium"),
            "high" => Some("high"),
            "max" => Some("xhigh"),
            _ => None,
        };
    }

    let thinking = body.get("thinking")?;
    match thinking.get("type").and_then(|t| t.as_str()) {
        Some("adaptive") => Some("high"),
        Some("enabled") => {
            let budget = thinking.get("budget_tokens").and_then(|b| b.as_u64());
            match budget {
                Some(b) if b < 4_000 => Some("low"),
                Some(b) if b < 16_000 => Some("medium"),
                Some(_) => Some("high"),
                None => Some("high"),
            }
        }
        _ => None,
    }
}

const MAX_THINKING_BUDGET: u64 = 32_000;
const MAX_RECTIFIED_MAX_TOKENS: u64 = 64_000;

pub fn rectify_anthropic_request_bytes(
    body_bytes: &[u8],
    error_message: Option<&str>,
    config: &crate::proxy_optimizer::config::RectifierConfig,
) -> Result<Option<Vec<u8>>, String> {
    if !config.enabled {
        return Ok(None);
    }
    let Some(error_message) = error_message
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let mut body: Value = match serde_json::from_slice(body_bytes) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let mut applied = false;
    if config.thinking_signature && should_rectify_thinking_signature(error_message) {
        applied |= rectify_thinking_signature(&mut body);
    }
    if config.thinking_budget && should_rectify_thinking_budget(error_message) {
        applied |= rectify_thinking_budget(&mut body);
    }
    if !applied {
        return Ok(None);
    }

    serde_json::to_vec(&body)
        .map(Some)
        .map_err(|error| error.to_string())
}

pub fn strip_codex_oauth_fields(body: &mut Value) {
    if let Some(obj) = body.as_object_mut() {
        obj.remove("max_output_tokens");
        obj.remove("temperature");
        obj.remove("top_p");
        obj.insert("stream".to_string(), Value::Bool(true));
        obj.insert("store".to_string(), Value::Bool(false));
        obj.entry("instructions".to_string()).or_insert(Value::String(String::new()));
        obj.entry("tools".to_string()).or_insert(Value::Array(Vec::new()));
        obj.entry("parallel_tool_calls".to_string()).or_insert(Value::Bool(false));
    }
}

fn should_rectify_thinking_signature(error_message: &str) -> bool {
    let lower = error_message.to_ascii_lowercase();
    (lower.contains("invalid")
        && lower.contains("signature")
        && lower.contains("thinking")
        && lower.contains("block"))
        || lower.contains("must start with a thinking block")
        || (lower.contains("expected")
            && (lower.contains("thinking") || lower.contains("redacted_thinking"))
            && lower.contains("found")
            && lower.contains("tool_use"))
        || (lower.contains("signature") && lower.contains("field required"))
        || (lower.contains("signature") && lower.contains("extra inputs are not permitted"))
        || ((lower.contains("thinking") || lower.contains("redacted_thinking"))
            && lower.contains("cannot be modified"))
        || lower.contains("illegal request")
        || lower.contains("invalid request")
        || lower.contains("非法请求")
}

fn rectify_thinking_signature(body: &mut Value) -> bool {
    let messages = match body
        .get_mut("messages")
        .and_then(|value| value.as_array_mut())
    {
        Some(messages) => messages,
        None => return false,
    };

    let mut applied = false;
    for message in messages.iter_mut() {
        let content = match message
            .get_mut("content")
            .and_then(|value| value.as_array_mut())
        {
            Some(content) => content,
            None => continue,
        };

        let mut next_content = Vec::with_capacity(content.len());
        let mut content_modified = false;
        for block in content.iter() {
            let block_type = block.get("type").and_then(|value| value.as_str());
            if matches!(block_type, Some("thinking") | Some("redacted_thinking")) {
                content_modified = true;
                continue;
            }

            if block.get("signature").is_some() {
                let mut block_clone = block.clone();
                if let Some(obj) = block_clone.as_object_mut() {
                    obj.remove("signature");
                    next_content.push(Value::Object(obj.clone()));
                    content_modified = true;
                    continue;
                }
            }

            next_content.push(block.clone());
        }

        if content_modified {
            *content = next_content;
            applied = true;
        }
    }

    let messages_snapshot = body
        .get("messages")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if should_remove_top_level_thinking(body, &messages_snapshot) {
        if let Some(obj) = body.as_object_mut() {
            if obj.remove("thinking").is_some() {
                applied = true;
            }
        }
    }

    applied
}

fn should_remove_top_level_thinking(body: &Value, messages: &[Value]) -> bool {
    if body
        .get("thinking")
        .and_then(|value| value.get("type"))
        .and_then(|value| value.as_str())
        != Some("enabled")
    {
        return false;
    }

    let Some(last_assistant_content) = messages
        .iter()
        .rev()
        .find(|value| value.get("role").and_then(|role| role.as_str()) == Some("assistant"))
        .and_then(|value| value.get("content"))
        .and_then(|value| value.as_array())
        .filter(|value| !value.is_empty())
    else {
        return false;
    };

    let first_block_type = last_assistant_content
        .first()
        .and_then(|value| value.get("type"))
        .and_then(|value| value.as_str());
    if matches!(
        first_block_type,
        Some("thinking") | Some("redacted_thinking")
    ) {
        return false;
    }

    last_assistant_content.iter().any(|value| {
        value.get("type").and_then(|block_type| block_type.as_str()) == Some("tool_use")
    })
}

fn should_rectify_thinking_budget(error_message: &str) -> bool {
    let lower = error_message.to_ascii_lowercase();
    let has_budget_tokens_reference =
        lower.contains("budget_tokens") || lower.contains("budget tokens");
    let has_thinking_reference = lower.contains("thinking");
    let has_1024_constraint = lower.contains("greater than or equal to 1024")
        || lower.contains(">= 1024")
        || (lower.contains("1024") && lower.contains("input should be"));

    has_budget_tokens_reference && has_thinking_reference && has_1024_constraint
}

fn rectify_thinking_budget(body: &mut Value) -> bool {
    let before = snapshot_budget(body);
    if before.0.as_deref() == Some("adaptive") {
        return false;
    }

    if !body.get("thinking").is_some_and(Value::is_object) {
        body["thinking"] = Value::Object(serde_json::Map::new());
    }
    let Some(thinking) = body
        .get_mut("thinking")
        .and_then(|value| value.as_object_mut())
    else {
        return false;
    };

    thinking.insert("type".to_string(), Value::String("enabled".to_string()));
    thinking.insert(
        "budget_tokens".to_string(),
        Value::Number(MAX_THINKING_BUDGET.into()),
    );

    let max_tokens = body.get("max_tokens").and_then(|value| value.as_u64());
    if max_tokens.is_none() || max_tokens < Some(MAX_THINKING_BUDGET + 1) {
        body["max_tokens"] = Value::Number(MAX_RECTIFIED_MAX_TOKENS.into());
    }

    snapshot_budget(body) != before
}

fn snapshot_budget(body: &Value) -> (Option<String>, Option<u64>, Option<u64>) {
    let max_tokens = body.get("max_tokens").and_then(|value| value.as_u64());
    let thinking = body.get("thinking").and_then(|value| value.as_object());
    let thinking_type = thinking
        .and_then(|value| value.get("type"))
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let thinking_budget_tokens = thinking
        .and_then(|value| value.get("budget_tokens"))
        .and_then(|value| value.as_u64());
    (thinking_type, max_tokens, thinking_budget_tokens)
}

pub fn clean_schema(mut schema: Value) -> Value {
    if let Some(obj) = schema.as_object_mut() {
        if obj.get("format").and_then(|v| v.as_str()) == Some("uri") {
            obj.remove("format");
        }

        if let Some(properties) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
            for (_, value) in properties.iter_mut() {
                *value = clean_schema(value.clone());
            }
        }

        if let Some(items) = obj.get_mut("items") {
            *items = clean_schema(items.clone());
        }
    }
    schema
}

pub fn anthropic_to_openai(body: Value) -> Result<Value, String> {
    let mut result = json!({});

    if let Some(model) = body.get("model").and_then(|m| m.as_str()) {
        result["model"] = json!(model);
    }

    let mut messages = Vec::new();
    if let Some(system) = body.get("system") {
        if let Some(text) = system.as_str() {
            messages.push(json!({"role": "system", "content": text}));
        } else if let Some(arr) = system.as_array() {
            for msg in arr {
                if let Some(text) = msg.get("text").and_then(|t| t.as_str()) {
                    let mut sys_msg = json!({"role": "system", "content": text});
                    if let Some(cc) = msg.get("cache_control") {
                        sys_msg["cache_control"] = cc.clone();
                    }
                    messages.push(sys_msg);
                }
            }
        }
    }

    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let converted = convert_message_to_openai(role, msg.get("content"))?;
            messages.extend(converted);
        }
    }
    result["messages"] = json!(messages);

    let model = body.get("model").and_then(|m| m.as_str()).unwrap_or("");
    if let Some(v) = body.get("max_tokens") {
        if is_openai_o_series(model) {
            result["max_completion_tokens"] = v.clone();
        } else {
            result["max_tokens"] = v.clone();
        }
    }
    if let Some(v) = body.get("temperature") {
        result["temperature"] = v.clone();
    }
    if let Some(v) = body.get("top_p") {
        result["top_p"] = v.clone();
    }
    if let Some(v) = body.get("stop_sequences") {
        result["stop"] = v.clone();
    }
    if let Some(v) = body.get("stream") {
        result["stream"] = v.clone();
    }

    if supports_reasoning_effort(model) {
        if let Some(effort) = resolve_reasoning_effort(&body) {
            result["reasoning_effort"] = json!(effort);
        }
    }

    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let openai_tools: Vec<Value> = tools
            .iter()
            .filter(|t| t.get("type").and_then(|v| v.as_str()) != Some("BatchTool"))
            .map(|t| {
                let mut tool = json!({
                    "type": "function",
                    "function": {
                        "name": t.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                        "description": t.get("description"),
                        "parameters": clean_schema(t.get("input_schema").cloned().unwrap_or(json!({})))
                    }
                });
                if let Some(cc) = t.get("cache_control") {
                    tool["cache_control"] = cc.clone();
                }
                tool
            })
            .collect();
        if !openai_tools.is_empty() {
            result["tools"] = json!(openai_tools);
        }
    }

    if let Some(v) = body.get("tool_choice") {
        result["tool_choice"] = v.clone();
    }

    Ok(result)
}

fn convert_message_to_openai(role: &str, content: Option<&Value>) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    let Some(content) = content else {
        result.push(json!({"role": role, "content": null}));
        return Ok(result);
    };

    if let Some(text) = content.as_str() {
        result.push(json!({"role": role, "content": text}));
        return Ok(result);
    }

    if let Some(blocks) = content.as_array() {
        let mut content_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in blocks {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        let mut part = json!({"type": "text", "text": text});
                        if let Some(cc) = block.get("cache_control") {
                            part["cache_control"] = cc.clone();
                        }
                        content_parts.push(part);
                    }
                }
                "image" => {
                    if let Some(source) = block.get("source") {
                        let media_type = source
                            .get("media_type")
                            .and_then(|m| m.as_str())
                            .unwrap_or("image/png");
                        let data = source.get("data").and_then(|d| d.as_str()).unwrap_or("");
                        content_parts.push(json!({
                            "type": "image_url",
                            "image_url": {"url": format!("data:{};base64,{}", media_type, data)}
                        }));
                    }
                }
                "tool_use" => {
                    let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let input = block.get("input").cloned().unwrap_or(json!({}));
                    tool_calls.push(json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": serde_json::to_string(&input).unwrap_or_default()
                        }
                    }));
                }
                "tool_result" => {
                    let tool_use_id = block
                        .get("tool_use_id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("");
                    let content_str = match block.get("content") {
                        Some(Value::String(s)) => s.clone(),
                        Some(v) => serde_json::to_string(v).unwrap_or_default(),
                        None => String::new(),
                    };
                    result.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": content_str
                    }));
                }
                "thinking" => {}
                _ => {}
            }
        }

        if !content_parts.is_empty() || !tool_calls.is_empty() {
            let mut msg = json!({"role": role});
            if content_parts.is_empty() {
                msg["content"] = Value::Null;
            } else if content_parts.len() == 1 {
                let has_cache_control = content_parts[0].get("cache_control").is_some();
                if !has_cache_control {
                    if let Some(text) = content_parts[0].get("text") {
                        msg["content"] = text.clone();
                    } else {
                        msg["content"] = json!(content_parts);
                    }
                } else {
                    msg["content"] = json!(content_parts);
                }
            } else {
                msg["content"] = json!(content_parts);
            }
            if !tool_calls.is_empty() {
                msg["tool_calls"] = json!(tool_calls);
            }
            result.push(msg);
        }

        return Ok(result);
    }

    result.push(json!({"role": role, "content": content}));
    Ok(result)
}

pub fn openai_to_anthropic(body: Value) -> Result<Value, String> {
    let choices = body
        .get("choices")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "No choices in response".to_string())?;
    let choice = choices
        .first()
        .ok_or_else(|| "Empty choices array".to_string())?;
    let message = choice
        .get("message")
        .ok_or_else(|| "No message in choice".to_string())?;

    let mut content = Vec::new();
    let mut has_tool_use = false;

    if let Some(msg_content) = message.get("content") {
        if let Some(text) = msg_content.as_str() {
            if !text.is_empty() {
                content.push(json!({"type": "text", "text": text}));
            }
        } else if let Some(parts) = msg_content.as_array() {
            for part in parts {
                match part.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                    "text" | "output_text" => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            if !text.is_empty() {
                                content.push(json!({"type": "text", "text": text}));
                            }
                        }
                    }
                    "refusal" => {
                        if let Some(refusal) = part.get("refusal").and_then(|r| r.as_str()) {
                            if !refusal.is_empty() {
                                content.push(json!({"type": "text", "text": refusal}));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(refusal) = message.get("refusal").and_then(|r| r.as_str()) {
        if !refusal.is_empty() {
            content.push(json!({"type": "text", "text": refusal}));
        }
    }

    if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
        if !tool_calls.is_empty() {
            has_tool_use = true;
        }
        for tc in tool_calls {
            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let empty_obj = json!({});
            let func = tc.get("function").unwrap_or(&empty_obj);
            let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args_str = func
                .get("arguments")
                .and_then(|a| a.as_str())
                .unwrap_or("{}");
            let input: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
            content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input
            }));
        }
    }

    if !has_tool_use {
        if let Some(function_call) = message.get("function_call") {
            let id = function_call
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or("");
            let name = function_call
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let has_arguments = function_call.get("arguments").is_some();
            let input = match function_call.get("arguments") {
                Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(json!({})),
                Some(v @ Value::Object(_)) | Some(v @ Value::Array(_)) => v.clone(),
                _ => json!({}),
            };
            if !name.is_empty() || has_arguments {
                content.push(json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input
                }));
                has_tool_use = true;
            }
        }
    }

    let stop_reason = choice
        .get("finish_reason")
        .and_then(|r| r.as_str())
        .map(|r| match r {
            "stop" => "end_turn",
            "length" => "max_tokens",
            "tool_calls" | "function_call" => "tool_use",
            "content_filter" => "end_turn",
            _ => "end_turn",
        })
        .or(if has_tool_use { Some("tool_use") } else { None });

    let usage = body.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let mut usage_json = json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens
    });
    if let Some(cached) = usage
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(|v| v.as_u64())
    {
        usage_json["cache_read_input_tokens"] = json!(cached);
    }
    if let Some(v) = usage.get("cache_read_input_tokens") {
        usage_json["cache_read_input_tokens"] = v.clone();
    }
    if let Some(v) = usage.get("cache_creation_input_tokens") {
        usage_json["cache_creation_input_tokens"] = v.clone();
    }

    Ok(json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or(""),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": usage_json
    }))
}

pub fn anthropic_to_responses(body: Value) -> Result<Value, String> {
    let mut result = json!({});
    if let Some(model) = body.get("model").and_then(|m| m.as_str()) {
        result["model"] = json!(model);
    }

    if let Some(system) = body.get("system") {
        let instructions = if let Some(text) = system.as_str() {
            text.to_string()
        } else if let Some(arr) = system.as_array() {
            arr.iter()
                .filter_map(|msg| msg.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            String::new()
        };
        if !instructions.is_empty() {
            result["instructions"] = json!(instructions);
        }
    }

    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        result["input"] = json!(convert_messages_to_input(msgs)?);
    }

    if let Some(v) = body.get("max_tokens") {
        result["max_output_tokens"] = v.clone();
    }
    if let Some(v) = body.get("temperature") {
        result["temperature"] = v.clone();
    }
    if let Some(v) = body.get("top_p") {
        result["top_p"] = v.clone();
    }
    if let Some(v) = body.get("stream") {
        result["stream"] = v.clone();
    }

    if let Some(model_name) = body.get("model").and_then(|m| m.as_str()) {
        if supports_reasoning_effort(model_name) {
            if let Some(effort) = resolve_reasoning_effort(&body) {
                result["reasoning"] = json!({ "effort": effort });
            }
        }
    }

    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let response_tools: Vec<Value> = tools
            .iter()
            .filter(|t| t.get("type").and_then(|v| v.as_str()) != Some("BatchTool"))
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                    "description": t.get("description"),
                    "parameters": clean_schema(t.get("input_schema").cloned().unwrap_or(json!({})))
                })
            })
            .collect();
        if !response_tools.is_empty() {
            result["tools"] = json!(response_tools);
        }
    }

    if let Some(v) = body.get("tool_choice") {
        result["tool_choice"] = map_tool_choice_to_responses(v);
    }

    Ok(result)
}

fn map_tool_choice_to_responses(tool_choice: &Value) -> Value {
    match tool_choice {
        Value::String(_) => tool_choice.clone(),
        Value::Object(obj) => match obj.get("type").and_then(|t| t.as_str()) {
            Some("any") => json!("required"),
            Some("auto") => json!("auto"),
            Some("none") => json!("none"),
            Some("tool") => {
                let name = obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                json!({ "type": "function", "name": name })
            }
            _ => tool_choice.clone(),
        },
        _ => tool_choice.clone(),
    }
}

pub fn build_anthropic_usage_from_responses(usage: Option<&Value>) -> Value {
    let u = match usage {
        Some(v) if !v.is_null() => v,
        _ => {
            return json!({
                "input_tokens": 0,
                "output_tokens": 0
            })
        }
    };

    let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let output = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let mut result = json!({
        "input_tokens": input,
        "output_tokens": output
    });

    if let Some(cached) = u
        .pointer("/input_tokens_details/cached_tokens")
        .and_then(|v| v.as_u64())
    {
        result["cache_read_input_tokens"] = json!(cached);
    }
    if let Some(cached) = u
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(|v| v.as_u64())
    {
        if result.get("cache_read_input_tokens").is_none() {
            result["cache_read_input_tokens"] = json!(cached);
        }
    }
    if let Some(v) = u.get("cache_read_input_tokens") {
        result["cache_read_input_tokens"] = v.clone();
    }
    if let Some(v) = u.get("cache_creation_input_tokens") {
        result["cache_creation_input_tokens"] = v.clone();
    }

    result
}

fn map_responses_stop_reason(
    status: Option<&str>,
    has_tool_use: bool,
    incomplete_reason: Option<&str>,
) -> Option<&'static str> {
    status.map(|s| match s {
        "completed" => {
            if has_tool_use {
                "tool_use"
            } else {
                "end_turn"
            }
        }
        "incomplete" => {
            if matches!(
                incomplete_reason,
                Some("max_output_tokens") | Some("max_tokens")
            ) || incomplete_reason.is_none()
            {
                "max_tokens"
            } else {
                "end_turn"
            }
        }
        _ => "end_turn",
    })
}

fn convert_messages_to_input(messages: &[Value]) -> Result<Vec<Value>, String> {
    let mut input = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        match msg.get("content") {
            Some(Value::String(text)) => {
                let content_type = if role == "assistant" {
                    "output_text"
                } else {
                    "input_text"
                };
                input.push(json!({
                    "role": role,
                    "content": [{ "type": content_type, "text": text }]
                }));
            }
            Some(Value::Array(blocks)) => {
                let mut message_content = Vec::new();
                for block in blocks {
                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match block_type {
                        "text" => {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                let content_type = if role == "assistant" {
                                    "output_text"
                                } else {
                                    "input_text"
                                };
                                message_content.push(json!({ "type": content_type, "text": text }));
                            }
                        }
                        "image" => {
                            if let Some(source) = block.get("source") {
                                let media_type = source
                                    .get("media_type")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("image/png");
                                let data =
                                    source.get("data").and_then(|d| d.as_str()).unwrap_or("");
                                message_content.push(json!({
                                    "type": "input_image",
                                    "image_url": format!("data:{media_type};base64,{data}")
                                }));
                            }
                        }
                        "tool_use" => {
                            if !message_content.is_empty() {
                                input.push(
                                    json!({ "role": role, "content": message_content.clone() }),
                                );
                                message_content.clear();
                            }
                            let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let arguments = block.get("input").cloned().unwrap_or(json!({}));
                            input.push(json!({
                                "type": "function_call",
                                "call_id": id,
                                "name": name,
                                "arguments": serde_json::to_string(&arguments).unwrap_or_default()
                            }));
                        }
                        "tool_result" => {
                            if !message_content.is_empty() {
                                input.push(
                                    json!({ "role": role, "content": message_content.clone() }),
                                );
                                message_content.clear();
                            }
                            let call_id = block
                                .get("tool_use_id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("");
                            let output = match block.get("content") {
                                Some(Value::String(s)) => s.clone(),
                                Some(v) => serde_json::to_string(v).unwrap_or_default(),
                                None => String::new(),
                            };
                            input.push(json!({
                                "type": "function_call_output",
                                "call_id": call_id,
                                "output": output
                            }));
                        }
                        "thinking" => {}
                        _ => {}
                    }
                }

                if !message_content.is_empty() {
                    input.push(json!({ "role": role, "content": message_content }));
                }
            }
            _ => {
                input.push(json!({ "role": role }));
            }
        }
    }

    Ok(input)
}

pub fn responses_to_anthropic(body: Value) -> Result<Value, String> {
    let output = body
        .get("output")
        .and_then(|o| o.as_array())
        .ok_or_else(|| "No output in response".to_string())?;

    let mut content = Vec::new();
    let mut has_tool_use = false;

    for item in output {
        match item.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "message" => {
                if let Some(msg_content) = item.get("content").and_then(|c| c.as_array()) {
                    for block in msg_content {
                        match block.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                            "output_text" => {
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    if !text.is_empty() {
                                        content.push(json!({"type": "text", "text": text}));
                                    }
                                }
                            }
                            "refusal" => {
                                if let Some(refusal) = block.get("refusal").and_then(|t| t.as_str())
                                {
                                    if !refusal.is_empty() {
                                        content.push(json!({"type": "text", "text": refusal}));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "function_call" => {
                let call_id = item.get("call_id").and_then(|i| i.as_str()).unwrap_or("");
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args_str = item
                    .get("arguments")
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");
                let input: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                content.push(json!({
                    "type": "tool_use",
                    "id": call_id,
                    "name": name,
                    "input": input
                }));
                has_tool_use = true;
            }
            "reasoning" => {
                if let Some(summary) = item.get("summary").and_then(|s| s.as_array()) {
                    let thinking_text: String = summary
                        .iter()
                        .filter_map(|s| {
                            if s.get("type").and_then(|t| t.as_str()) == Some("summary_text") {
                                s.get("text").and_then(|t| t.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    if !thinking_text.is_empty() {
                        content.push(json!({"type": "thinking", "thinking": thinking_text}));
                    }
                }
            }
            _ => {}
        }
    }

    let stop_reason = map_responses_stop_reason(
        body.get("status").and_then(|s| s.as_str()),
        has_tool_use,
        body.pointer("/incomplete_details/reason")
            .and_then(|r| r.as_str()),
    );

    Ok(json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or(""),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": build_anthropic_usage_from_responses(body.get("usage"))
    }))
}

pub fn openai_error_to_anthropic(status_code: u16, body: Option<&Value>) -> Value {
    let message = body
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
                .or_else(|| value.get("message").and_then(|message| message.as_str()))
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("Upstream returned HTTP {}", status_code));

    json!({
        "type": "error",
        "error": {
            "type": "api_error",
            "message": message
        }
    })
}

fn strip_sse_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    line.strip_prefix(field)
        .and_then(|rest| rest.strip_prefix(':'))
        .map(str::trim_start)
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    id: String,
    model: String,
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<DeltaFunction>,
}

#[derive(Debug, Deserialize)]
struct DeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: u32,
}

#[derive(Debug, Clone)]
struct ToolBlockState {
    anthropic_index: u32,
    id: String,
    name: String,
    started: bool,
    pending_args: String,
}

pub fn create_anthropic_sse_stream<E: std::error::Error + Send + 'static>(
    stream: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send {
    async_stream::stream! {
        let mut buffer = String::new();
        let mut message_id = None;
        let mut current_model = None;
        let mut next_content_index: u32 = 0;
        let mut has_sent_message_start = false;
        let mut current_non_tool_block_type: Option<&'static str> = None;
        let mut current_non_tool_block_index: Option<u32> = None;
        let mut tool_blocks_by_index: HashMap<usize, ToolBlockState> = HashMap::new();
        let mut open_tool_block_indices: HashSet<u32> = HashSet::new();

        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&text);
                    while let Some(pos) = buffer.find("\n\n") {
                        let line = buffer[..pos].to_string();
                        buffer = buffer[pos + 2..].to_string();
                        if line.trim().is_empty() {
                            continue;
                        }

                        for l in line.lines() {
                            if let Some(data) = strip_sse_field(l, "data") {
                                if data.trim() == "[DONE]" {
                                    let event = json!({"type": "message_stop"});
                                    yield Ok(Bytes::from(format!("event: message_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                    continue;
                                }

                                if let Ok(chunk) = serde_json::from_str::<OpenAIStreamChunk>(data) {
                                    if message_id.is_none() {
                                        message_id = Some(chunk.id.clone());
                                    }
                                    if current_model.is_none() {
                                        current_model = Some(chunk.model.clone());
                                    }

                                    if let Some(choice) = chunk.choices.first() {
                                        if !has_sent_message_start {
                                            let mut start_usage = json!({"input_tokens": 0, "output_tokens": 0});
                                            if let Some(u) = &chunk.usage {
                                                start_usage["input_tokens"] = json!(u.prompt_tokens);
                                                if let Some(cached) = extract_cache_read_tokens(u) {
                                                    start_usage["cache_read_input_tokens"] = json!(cached);
                                                }
                                                if let Some(created) = u.cache_creation_input_tokens {
                                                    start_usage["cache_creation_input_tokens"] = json!(created);
                                                }
                                            }
                                            let event = json!({
                                                "type": "message_start",
                                                "message": {
                                                    "id": message_id.clone().unwrap_or_default(),
                                                    "type": "message",
                                                    "role": "assistant",
                                                    "model": current_model.clone().unwrap_or_default(),
                                                    "usage": start_usage
                                                }
                                            });
                                            yield Ok(Bytes::from(format!("event: message_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                            has_sent_message_start = true;
                                        }

                                        if let Some(reasoning) = &choice.delta.reasoning {
                                            if current_non_tool_block_type != Some("thinking") {
                                                if let Some(index) = current_non_tool_block_index.take() {
                                                    let event = json!({"type": "content_block_stop", "index": index});
                                                    yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                }
                                                let index = next_content_index;
                                                next_content_index += 1;
                                                let event = json!({
                                                    "type": "content_block_start",
                                                    "index": index,
                                                    "content_block": { "type": "thinking", "thinking": "" }
                                                });
                                                yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                current_non_tool_block_type = Some("thinking");
                                                current_non_tool_block_index = Some(index);
                                            }

                                            if let Some(index) = current_non_tool_block_index {
                                                let event = json!({
                                                    "type": "content_block_delta",
                                                    "index": index,
                                                    "delta": { "type": "thinking_delta", "thinking": reasoning }
                                                });
                                                yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                            }
                                        }

                                        if let Some(content) = &choice.delta.content {
                                            if !content.is_empty() {
                                                if current_non_tool_block_type != Some("text") {
                                                    if let Some(index) = current_non_tool_block_index.take() {
                                                        let event = json!({"type": "content_block_stop", "index": index});
                                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                    }
                                                    let index = next_content_index;
                                                    next_content_index += 1;
                                                    let event = json!({
                                                        "type": "content_block_start",
                                                        "index": index,
                                                        "content_block": { "type": "text", "text": "" }
                                                    });
                                                    yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                    current_non_tool_block_type = Some("text");
                                                    current_non_tool_block_index = Some(index);
                                                }

                                                if let Some(index) = current_non_tool_block_index {
                                                    let event = json!({
                                                        "type": "content_block_delta",
                                                        "index": index,
                                                        "delta": { "type": "text_delta", "text": content }
                                                    });
                                                    yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                }
                                            }
                                        }

                                        if let Some(tool_calls) = &choice.delta.tool_calls {
                                            if let Some(index) = current_non_tool_block_index.take() {
                                                let event = json!({"type": "content_block_stop", "index": index});
                                                yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                            }
                                            current_non_tool_block_type = None;

                                            for tool_call in tool_calls {
                                                let (anthropic_index, id, name, should_start, pending_after_start, immediate_delta) = {
                                                    let state = tool_blocks_by_index.entry(tool_call.index).or_insert_with(|| {
                                                        let index = next_content_index;
                                                        next_content_index += 1;
                                                        ToolBlockState {
                                                            anthropic_index: index,
                                                            id: String::new(),
                                                            name: String::new(),
                                                            started: false,
                                                            pending_args: String::new(),
                                                        }
                                                    });

                                                    if let Some(id) = &tool_call.id {
                                                        state.id = id.clone();
                                                    }
                                                    if let Some(function) = &tool_call.function {
                                                        if let Some(name) = &function.name {
                                                            state.name = name.clone();
                                                        }
                                                    }

                                                    let should_start = !state.started && !state.id.is_empty() && !state.name.is_empty();
                                                    if should_start {
                                                        state.started = true;
                                                    }
                                                    let pending_after_start = if should_start && !state.pending_args.is_empty() {
                                                        Some(std::mem::take(&mut state.pending_args))
                                                    } else {
                                                        None
                                                    };
                                                    let args_delta = tool_call.function.as_ref().and_then(|f| f.arguments.clone());
                                                    let immediate_delta = if let Some(args) = args_delta {
                                                        if state.started {
                                                            Some(args)
                                                        } else {
                                                            state.pending_args.push_str(&args);
                                                            None
                                                        }
                                                    } else {
                                                        None
                                                    };
                                                    (
                                                        state.anthropic_index,
                                                        state.id.clone(),
                                                        state.name.clone(),
                                                        should_start,
                                                        pending_after_start,
                                                        immediate_delta,
                                                    )
                                                };

                                                if should_start {
                                                    let event = json!({
                                                        "type": "content_block_start",
                                                        "index": anthropic_index,
                                                        "content_block": { "type": "tool_use", "id": id, "name": name }
                                                    });
                                                    yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                    open_tool_block_indices.insert(anthropic_index);
                                                }

                                                if let Some(args) = pending_after_start {
                                                    let event = json!({
                                                        "type": "content_block_delta",
                                                        "index": anthropic_index,
                                                        "delta": { "type": "input_json_delta", "partial_json": args }
                                                    });
                                                    yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                }

                                                if let Some(args) = immediate_delta {
                                                    let event = json!({
                                                        "type": "content_block_delta",
                                                        "index": anthropic_index,
                                                        "delta": { "type": "input_json_delta", "partial_json": args }
                                                    });
                                                    yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                }
                                            }
                                        }

                                        if let Some(finish_reason) = &choice.finish_reason {
                                            if let Some(index) = current_non_tool_block_index.take() {
                                                let event = json!({"type": "content_block_stop", "index": index});
                                                yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                            }
                                            current_non_tool_block_type = None;

                                            let mut late_tool_starts: Vec<(u32, String, String, String)> = Vec::new();
                                            for (tool_idx, state) in tool_blocks_by_index.iter_mut() {
                                                if state.started {
                                                    continue;
                                                }
                                                let has_payload = !state.pending_args.is_empty() || !state.id.is_empty() || !state.name.is_empty();
                                                if !has_payload {
                                                    continue;
                                                }
                                                let fallback_id = if state.id.is_empty() {
                                                    format!("tool_call_{tool_idx}")
                                                } else {
                                                    state.id.clone()
                                                };
                                                let fallback_name = if state.name.is_empty() {
                                                    "unknown_tool".to_string()
                                                } else {
                                                    state.name.clone()
                                                };
                                                state.started = true;
                                                let pending = std::mem::take(&mut state.pending_args);
                                                late_tool_starts.push((state.anthropic_index, fallback_id, fallback_name, pending));
                                            }
                                            late_tool_starts.sort_unstable_by_key(|(index, _, _, _)| *index);
                                            for (index, id, name, pending) in late_tool_starts {
                                                let event = json!({
                                                    "type": "content_block_start",
                                                    "index": index,
                                                    "content_block": { "type": "tool_use", "id": id, "name": name }
                                                });
                                                yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                open_tool_block_indices.insert(index);
                                                if !pending.is_empty() {
                                                    let delta_event = json!({
                                                        "type": "content_block_delta",
                                                        "index": index,
                                                        "delta": { "type": "input_json_delta", "partial_json": pending }
                                                    });
                                                    yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&delta_event).unwrap_or_default())));
                                                }
                                            }

                                            if !open_tool_block_indices.is_empty() {
                                                let mut tool_indices: Vec<u32> = open_tool_block_indices.iter().copied().collect();
                                                tool_indices.sort_unstable();
                                                for index in tool_indices {
                                                    let event = json!({"type": "content_block_stop", "index": index});
                                                    yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                                }
                                                open_tool_block_indices.clear();
                                            }

                                            let usage_json = chunk.usage.as_ref().map(|u| {
                                                let mut uj = json!({"input_tokens": u.prompt_tokens, "output_tokens": u.completion_tokens});
                                                if let Some(cached) = extract_cache_read_tokens(u) {
                                                    uj["cache_read_input_tokens"] = json!(cached);
                                                }
                                                if let Some(created) = u.cache_creation_input_tokens {
                                                    uj["cache_creation_input_tokens"] = json!(created);
                                                }
                                                uj
                                            });
                                            let event = json!({
                                                "type": "message_delta",
                                                "delta": {
                                                    "stop_reason": map_stop_reason(Some(finish_reason)),
                                                    "stop_sequence": null
                                                },
                                                "usage": usage_json
                                            });
                                            yield Ok(Bytes::from(format!("event: message_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_event = json!({
                        "type": "error",
                        "error": {
                            "type": "stream_error",
                            "message": format!("Stream error: {e}")
                        }
                    });
                    yield Ok(Bytes::from(format!("event: error\ndata: {}\n\n", serde_json::to_string(&error_event).unwrap_or_default())));
                    break;
                }
            }
        }
    }
}

fn extract_cache_read_tokens(usage: &Usage) -> Option<u32> {
    if let Some(v) = usage.cache_read_input_tokens {
        return Some(v);
    }
    usage
        .prompt_tokens_details
        .as_ref()
        .map(|d| d.cached_tokens)
        .filter(|&v| v > 0)
}

fn map_stop_reason(finish_reason: Option<&str>) -> Option<String> {
    finish_reason.map(|r| {
        match r {
            "tool_calls" | "function_call" => "tool_use",
            "stop" => "end_turn",
            "length" => "max_tokens",
            "content_filter" => "end_turn",
            _ => "end_turn",
        }
        .to_string()
    })
}

#[inline]
fn response_object_from_event(data: &Value) -> &Value {
    data.get("response").unwrap_or(data)
}

#[inline]
fn content_part_key(data: &Value) -> Option<String> {
    if let (Some(item_id), Some(content_index)) = (
        data.get("item_id").and_then(|v| v.as_str()),
        data.get("content_index").and_then(|v| v.as_u64()),
    ) {
        return Some(format!("part:{item_id}:{content_index}"));
    }
    if let (Some(output_index), Some(content_index)) = (
        data.get("output_index").and_then(|v| v.as_u64()),
        data.get("content_index").and_then(|v| v.as_u64()),
    ) {
        return Some(format!("part:out:{output_index}:{content_index}"));
    }
    None
}

#[inline]
fn tool_item_key_from_added(data: &Value, item: &Value) -> Option<String> {
    if let Some(item_id) = item.get("id").and_then(|v| v.as_str()) {
        return Some(format!("tool:{item_id}"));
    }
    if let Some(item_id) = data.get("item_id").and_then(|v| v.as_str()) {
        return Some(format!("tool:{item_id}"));
    }
    if let Some(output_index) = data.get("output_index").and_then(|v| v.as_u64()) {
        return Some(format!("tool:out:{output_index}"));
    }
    None
}

#[inline]
fn tool_item_key_from_event(data: &Value) -> Option<String> {
    if let Some(item_id) = data.get("item_id").and_then(|v| v.as_str()) {
        return Some(format!("tool:{item_id}"));
    }
    if let Some(output_index) = data.get("output_index").and_then(|v| v.as_u64()) {
        return Some(format!("tool:out:{output_index}"));
    }
    None
}

#[inline]
fn resolve_content_index(
    data: &Value,
    next_content_index: &mut u32,
    index_by_key: &mut HashMap<String, u32>,
    fallback_open_index: &mut Option<u32>,
) -> u32 {
    if let Some(k) = content_part_key(data) {
        if let Some(existing) = index_by_key.get(&k).copied() {
            existing
        } else {
            let assigned = *next_content_index;
            *next_content_index += 1;
            index_by_key.insert(k, assigned);
            assigned
        }
    } else if let Some(existing) = *fallback_open_index {
        existing
    } else {
        let assigned = *next_content_index;
        *next_content_index += 1;
        *fallback_open_index = Some(assigned);
        assigned
    }
}

pub fn create_anthropic_sse_stream_from_responses<E: std::error::Error + Send + 'static>(
    stream: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send {
    async_stream::stream! {
        let mut buffer = String::new();
        let mut message_id: Option<String> = None;
        let mut current_model: Option<String> = None;
        let mut has_sent_message_start = false;
        let mut has_tool_use = false;
        let mut next_content_index: u32 = 0;
        let mut index_by_key: HashMap<String, u32> = HashMap::new();
        let mut open_indices: HashSet<u32> = HashSet::new();
        let mut fallback_open_index: Option<u32> = None;
        let mut current_text_index: Option<u32> = None;
        let mut tool_index_by_item_id: HashMap<String, u32> = HashMap::new();
        let mut last_tool_index: Option<u32> = None;

        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&text);

                    while let Some(pos) = buffer.find("\n\n") {
                        let block = buffer[..pos].to_string();
                        buffer = buffer[pos + 2..].to_string();
                        if block.trim().is_empty() {
                            continue;
                        }

                        let mut event_type: Option<String> = None;
                        let mut data_parts: Vec<String> = Vec::new();
                        for line in block.lines() {
                            if let Some(evt) = strip_sse_field(line, "event") {
                                event_type = Some(evt.trim().to_string());
                            } else if let Some(d) = strip_sse_field(line, "data") {
                                data_parts.push(d.to_string());
                            }
                        }
                        if data_parts.is_empty() {
                            continue;
                        }

                        let data_str = data_parts.join("\n");
                        let event_name = event_type.as_deref().unwrap_or("");
                        let data: Value = match serde_json::from_str(&data_str) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        match event_name {
                            "response.created" => {
                                let response_obj = response_object_from_event(&data);
                                if let Some(id) = response_obj.get("id").and_then(|i| i.as_str()) {
                                    message_id = Some(id.to_string());
                                }
                                if let Some(model) = response_obj.get("model").and_then(|m| m.as_str()) {
                                    current_model = Some(model.to_string());
                                }

                                has_sent_message_start = true;
                                let start_usage = build_anthropic_usage_from_responses(response_obj.get("usage"));
                                let event = json!({
                                    "type": "message_start",
                                    "message": {
                                        "id": message_id.clone().unwrap_or_default(),
                                        "type": "message",
                                        "role": "assistant",
                                        "model": current_model.clone().unwrap_or_default(),
                                        "usage": start_usage
                                    }
                                });
                                yield Ok(Bytes::from(format!("event: message_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                            }
                            "response.content_part.added" => {
                                if !has_sent_message_start {
                                    let start_event = json!({
                                        "type": "message_start",
                                        "message": {
                                            "id": message_id.clone().unwrap_or_default(),
                                            "type": "message",
                                            "role": "assistant",
                                            "model": current_model.clone().unwrap_or_default(),
                                            "usage": { "input_tokens": 0, "output_tokens": 0 }
                                        }
                                    });
                                    yield Ok(Bytes::from(format!("event: message_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));
                                    has_sent_message_start = true;
                                }

                                if let Some(part) = data.get("part") {
                                    let part_type = part.get("type").and_then(|t| t.as_str());
                                    if matches!(part_type, Some("output_text") | Some("refusal")) {
                                        let index = if let Some(index) = current_text_index {
                                            index
                                        } else {
                                            let index = resolve_content_index(&data, &mut next_content_index, &mut index_by_key, &mut fallback_open_index);
                                            current_text_index = Some(index);
                                            index
                                        };

                                        if !open_indices.contains(&index) {
                                            let event = json!({
                                                "type": "content_block_start",
                                                "index": index,
                                                "content_block": { "type": "text", "text": "" }
                                            });
                                            yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                            open_indices.insert(index);
                                        }
                                    }
                                }
                            }
                            "response.output_text.delta" | "response.refusal.delta" => {
                                if let Some(delta) = data.get("delta").and_then(|d| d.as_str()) {
                                    let index = if let Some(index) = current_text_index {
                                        index
                                    } else {
                                        let index = resolve_content_index(&data, &mut next_content_index, &mut index_by_key, &mut fallback_open_index);
                                        current_text_index = Some(index);
                                        index
                                    };

                                    if !open_indices.contains(&index) {
                                        let start_event = json!({
                                            "type": "content_block_start",
                                            "index": index,
                                            "content_block": { "type": "text", "text": "" }
                                        });
                                        yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));
                                        open_indices.insert(index);
                                    }
                                    let event = json!({
                                        "type": "content_block_delta",
                                        "index": index,
                                        "delta": { "type": "text_delta", "text": delta }
                                    });
                                    yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                }
                            }
                            "response.output_item.added" => {
                                if let Some(item) = data.get("item") {
                                    let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                    if item_type == "function_call" {
                                        has_tool_use = true;
                                        if let Some(index) = current_text_index.take() {
                                            if open_indices.remove(&index) {
                                                let stop_event = json!({"type": "content_block_stop", "index": index});
                                                yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                            }
                                            if fallback_open_index == Some(index) {
                                                fallback_open_index = None;
                                            }
                                        }
                                        if !has_sent_message_start {
                                            let start_event = json!({
                                                "type": "message_start",
                                                "message": {
                                                    "id": message_id.clone().unwrap_or_default(),
                                                    "type": "message",
                                                    "role": "assistant",
                                                    "model": current_model.clone().unwrap_or_default(),
                                                    "usage": { "input_tokens": 0, "output_tokens": 0 }
                                                }
                                            });
                                            yield Ok(Bytes::from(format!("event: message_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));
                                            has_sent_message_start = true;
                                        }

                                        let call_id = item.get("call_id").and_then(|i| i.as_str()).unwrap_or("");
                                        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                        let index = if let Some(k) = tool_item_key_from_added(&data, item) {
                                            if let Some(existing) = index_by_key.get(&k).copied() {
                                                existing
                                            } else {
                                                let assigned = next_content_index;
                                                next_content_index += 1;
                                                index_by_key.insert(k, assigned);
                                                assigned
                                            }
                                        } else {
                                            let assigned = next_content_index;
                                            next_content_index += 1;
                                            assigned
                                        };
                                        if let Some(item_id) = item.get("id").and_then(|v| v.as_str()).or_else(|| data.get("item_id").and_then(|v| v.as_str())) {
                                            tool_index_by_item_id.insert(item_id.to_string(), index);
                                        }
                                        last_tool_index = Some(index);

                                        if !open_indices.contains(&index) {
                                            let event = json!({
                                                "type": "content_block_start",
                                                "index": index,
                                                "content_block": { "type": "tool_use", "id": call_id, "name": name }
                                            });
                                            yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                            open_indices.insert(index);
                                        }
                                    }
                                }
                            }
                            "response.function_call_arguments.delta" => {
                                if let Some(delta) = data.get("delta").and_then(|d| d.as_str()) {
                                    let item_id = data.get("item_id").and_then(|v| v.as_str());
                                    let index = if let Some(id) = item_id {
                                        tool_index_by_item_id.get(id).copied()
                                    } else {
                                        None
                                    }
                                    .or_else(|| tool_item_key_from_event(&data).and_then(|k| index_by_key.get(&k).copied()))
                                    .or(last_tool_index)
                                    .unwrap_or_else(|| {
                                        let assigned = next_content_index;
                                        next_content_index += 1;
                                        assigned
                                    });

                                    if !open_indices.contains(&index) {
                                        let start_event = json!({
                                            "type": "content_block_start",
                                            "index": index,
                                            "content_block": {
                                                "type": "tool_use",
                                                "id": data.get("call_id").and_then(|v| v.as_str()).or(item_id).unwrap_or(""),
                                                "name": data.get("name").and_then(|v| v.as_str()).unwrap_or("")
                                            }
                                        });
                                        yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));
                                        open_indices.insert(index);
                                    }

                                    let event = json!({
                                        "type": "content_block_delta",
                                        "index": index,
                                        "delta": { "type": "input_json_delta", "partial_json": delta }
                                    });
                                    yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                }
                            }
                            "response.function_call_arguments.done" => {
                                let item_id = data.get("item_id").and_then(|v| v.as_str());
                                let index = if let Some(id) = item_id {
                                    tool_index_by_item_id.get(id).copied()
                                } else {
                                    None
                                }
                                .or_else(|| tool_item_key_from_event(&data).and_then(|k| index_by_key.get(&k).copied()))
                                .or(last_tool_index);
                                if let Some(index) = index {
                                    if open_indices.remove(&index) {
                                        let event = json!({"type": "content_block_stop", "index": index});
                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                        if let Some(item_id) = item_id {
                                            tool_index_by_item_id.remove(item_id);
                                        }
                                    }
                                }
                            }
                            "response.refusal.done" | "response.output_text.done" => {
                                let index = current_text_index.take().or_else(|| {
                                    let key = content_part_key(&data);
                                    if let Some(k) = key {
                                        index_by_key.get(&k).copied()
                                    } else {
                                        fallback_open_index
                                    }
                                });
                                if let Some(index) = index {
                                    if open_indices.remove(&index) {
                                        let event = json!({"type": "content_block_stop", "index": index});
                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                        if fallback_open_index == Some(index) {
                                            fallback_open_index = None;
                                        }
                                    }
                                }
                            }
                            "response.reasoning.delta" => {
                                if let Some(delta) = data.get("delta").or_else(|| data.get("text")).and_then(|d| d.as_str()) {
                                    if let Some(index) = current_text_index.take() {
                                        if open_indices.remove(&index) {
                                            let stop_event = json!({"type": "content_block_stop", "index": index});
                                            yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                        }
                                        if fallback_open_index == Some(index) {
                                            fallback_open_index = None;
                                        }
                                    }
                                    let index = resolve_content_index(&data, &mut next_content_index, &mut index_by_key, &mut fallback_open_index);
                                    if !open_indices.contains(&index) {
                                        let start_event = json!({
                                            "type": "content_block_start",
                                            "index": index,
                                            "content_block": { "type": "thinking", "thinking": "" }
                                        });
                                        yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));
                                        open_indices.insert(index);
                                    }
                                    let event = json!({
                                        "type": "content_block_delta",
                                        "index": index,
                                        "delta": { "type": "thinking_delta", "thinking": delta }
                                    });
                                    yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                }
                            }
                            "response.reasoning.done" => {
                                let key = content_part_key(&data);
                                let index = if let Some(k) = key {
                                    index_by_key.get(&k).copied()
                                } else {
                                    fallback_open_index
                                };
                                if let Some(index) = index {
                                    if open_indices.remove(&index) {
                                        let event = json!({"type": "content_block_stop", "index": index});
                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                        if fallback_open_index == Some(index) {
                                            fallback_open_index = None;
                                        }
                                    }
                                }
                            }
                            "response.completed" => {
                                let response_obj = response_object_from_event(&data);
                                let stop_reason = map_responses_stop_reason(
                                    response_obj.get("status").and_then(|s| s.as_str()),
                                    has_tool_use,
                                    response_obj.pointer("/incomplete_details/reason").and_then(|r| r.as_str()),
                                );

                                if !open_indices.is_empty() {
                                    let mut remaining: Vec<u32> = open_indices.iter().copied().collect();
                                    remaining.sort_unstable();
                                    for index in remaining {
                                        let stop_event = json!({"type": "content_block_stop", "index": index});
                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                        open_indices.remove(&index);
                                    }
                                }
                                fallback_open_index = None;

                                let usage_json = response_obj.get("usage").map(|u| build_anthropic_usage_from_responses(Some(u)));
                                let delta_event = json!({
                                    "type": "message_delta",
                                    "delta": {
                                        "stop_reason": stop_reason,
                                        "stop_sequence": null
                                    },
                                    "usage": usage_json
                                });
                                yield Ok(Bytes::from(format!("event: message_delta\ndata: {}\n\n", serde_json::to_string(&delta_event).unwrap_or_default())));
                                let stop_event = json!({"type": "message_stop"});
                                yield Ok(Bytes::from(format!("event: message_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                            }
                            "response.output_item.done" | "response.in_progress" | "response.content_part.done" => {}
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    let error_event = json!({
                        "type": "error",
                        "error": {
                            "type": "stream_error",
                            "message": format!("Stream error: {e}")
                        }
                    });
                    yield Ok(Bytes::from(format!("event: error\ndata: {}\n\n", serde_json::to_string(&error_event).unwrap_or_default())));
                    break;
                }
            }
        }
    }
}

/// Converts Gemini SSE stream (`streamGenerateContent?alt=sse`) to Anthropic SSE format.
pub fn create_anthropic_sse_stream_from_gemini<E: std::error::Error + Send + 'static>(
    stream: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send {
    async_stream::stream! {
        let mut buffer = String::new();
        let mut has_sent_message_start = false;
        let mut next_content_index: u32 = 0;
        let mut current_text_block_index: Option<u32> = None;
        let mut has_tool_use = false;
        let mut finished = false;
        let msg_id = format!("msg_{:012x}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() & 0xffffffffffff);

        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&text);

                    while let Some(pos) = buffer.find("\n\n") {
                        let segment = buffer[..pos].to_string();
                        buffer = buffer[pos + 2..].to_string();

                        if segment.trim().is_empty() {
                            continue;
                        }

                        let data = segment
                            .lines()
                            .find_map(|l| l.strip_prefix("data: ").or_else(|| l.strip_prefix("data:")))
                            .unwrap_or("")
                            .trim();

                        if data.is_empty() || data == "[DONE]" {
                            continue;
                        }

                        let parsed: Value = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        let usage_meta = parsed.get("usageMetadata");
                        let input_tokens = usage_meta
                            .and_then(|u| u.get("promptTokenCount"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let output_tokens = usage_meta
                            .and_then(|u| u.get("candidatesTokenCount"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);

                        if !has_sent_message_start {
                            let event = json!({
                                "type": "message_start",
                                "message": {
                                    "id": msg_id,
                                    "type": "message",
                                    "role": "assistant",
                                    "model": model,
                                    "usage": {
                                        "input_tokens": input_tokens,
                                        "output_tokens": 0
                                    }
                                }
                            });
                            yield Ok(Bytes::from(format!("event: message_start\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                            has_sent_message_start = true;
                        }

                        let candidates = parsed.get("candidates").and_then(|c| c.as_array());
                        let candidate = candidates.and_then(|arr| arr.first());

                        if let Some(candidate) = candidate {
                            let parts = candidate
                                .get("content")
                                .and_then(|c| c.get("parts"))
                                .and_then(|p| p.as_array());

                            if let Some(parts) = parts {
                                for part in parts {
                                    if let Some(text_val) = part.get("text").and_then(|t| t.as_str()) {
                                        if current_text_block_index.is_none() {
                                            let index = next_content_index;
                                            next_content_index += 1;
                                            let start_event = json!({
                                                "type": "content_block_start",
                                                "index": index,
                                                "content_block": {"type": "text", "text": ""}
                                            });
                                            yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));
                                            current_text_block_index = Some(index);
                                        }
                                        if let Some(index) = current_text_block_index {
                                            let delta_event = json!({
                                                "type": "content_block_delta",
                                                "index": index,
                                                "delta": {"type": "text_delta", "text": text_val}
                                            });
                                            yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&delta_event).unwrap_or_default())));
                                        }
                                    } else if let Some(fc) = part.get("functionCall") {
                                        if let Some(index) = current_text_block_index.take() {
                                            let stop_event = json!({"type": "content_block_stop", "index": index});
                                            yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                        }

                                        has_tool_use = true;
                                        let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                                        let args = fc.get("args").cloned().unwrap_or(json!({}));
                                        let tool_id = format!("toolu_{:08x}{:04x}",
                                            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos(),
                                            (next_content_index as u16).wrapping_mul(7919)
                                        );

                                        let index = next_content_index;
                                        next_content_index += 1;

                                        let start_event = json!({
                                            "type": "content_block_start",
                                            "index": index,
                                            "content_block": {
                                                "type": "tool_use",
                                                "id": tool_id,
                                                "name": name,
                                                "input": {}
                                            }
                                        });
                                        yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));

                                        let args_str = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                                        let delta_event = json!({
                                            "type": "content_block_delta",
                                            "index": index,
                                            "delta": {"type": "input_json_delta", "partial_json": args_str}
                                        });
                                        yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&delta_event).unwrap_or_default())));

                                        let stop_event = json!({"type": "content_block_stop", "index": index});
                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                    }
                                }
                            }

                            let finish_reason = candidate.get("finishReason").and_then(|r| r.as_str());
                            if let Some(reason) = finish_reason {
                                if let Some(index) = current_text_block_index.take() {
                                    let stop_event = json!({"type": "content_block_stop", "index": index});
                                    yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                }

                                let stop_reason = if has_tool_use {
                                    "tool_use"
                                } else {
                                    match reason {
                                        "STOP" => "end_turn",
                                        "MAX_TOKENS" => "max_tokens",
                                        _ => "end_turn",
                                    }
                                };

                                let delta_event = json!({
                                    "type": "message_delta",
                                    "delta": {"stop_reason": stop_reason, "stop_sequence": null},
                                    "usage": {"output_tokens": output_tokens}
                                });
                                yield Ok(Bytes::from(format!("event: message_delta\ndata: {}\n\n", serde_json::to_string(&delta_event).unwrap_or_default())));

                                let stop_event = json!({"type": "message_stop"});
                                yield Ok(Bytes::from(format!("event: message_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                finished = true;
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_event = json!({
                        "type": "error",
                        "error": {
                            "type": "stream_error",
                            "message": format!("Stream error: {e}")
                        }
                    });
                    yield Ok(Bytes::from(format!("event: error\ndata: {}\n\n", serde_json::to_string(&error_event).unwrap_or_default())));
                    break;
                }
            }
        }

        if has_sent_message_start && !finished {
            if let Some(index) = current_text_block_index.take() {
                let stop_event = json!({"type": "content_block_stop", "index": index});
                yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
            }
            let delta_event = json!({
                "type": "message_delta",
                "delta": {"stop_reason": "end_turn", "stop_sequence": null},
                "usage": {"output_tokens": 0}
            });
            yield Ok(Bytes::from(format!("event: message_delta\ndata: {}\n\n", serde_json::to_string(&delta_event).unwrap_or_default())));
            let stop_event = json!({"type": "message_stop"});
            yield Ok(Bytes::from(format!("event: message_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
        }
    }
}

#[cfg(test)]
mod tests_gemini_stream {
    use super::*;
    use futures_util::stream;
    use futures_util::StreamExt;

    fn make_gemini_sse(chunks: &[&str]) -> Vec<Result<Bytes, std::io::Error>> {
        chunks
            .iter()
            .map(|c| Ok(Bytes::from(format!("data: {}\n\n", c))))
            .collect()
    }

    #[tokio::test]
    async fn test_gemini_stream_text_response() {
        let chunks = make_gemini_sse(&[
            r#"{"candidates":[{"content":{"parts":[{"text":"Hello"}],"role":"model"}}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":1}}"#,
            r#"{"candidates":[{"content":{"parts":[{"text":" world!"}],"role":"model"},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":3,"totalTokenCount":8}}"#,
        ]);

        let input_stream = stream::iter(chunks);
        let output = create_anthropic_sse_stream_from_gemini(input_stream, "gemini-2.5-flash".to_string());
        tokio::pin!(output);

        let mut events: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = output.next().await {
            events.push(String::from_utf8_lossy(&bytes).to_string());
        }

        let all = events.join("");
        assert!(all.contains("event: message_start"));
        assert!(all.contains("event: content_block_start"));
        assert!(all.contains("\"text\":\"Hello\""));
        assert!(all.contains("\"text\":\" world!\""));
        assert!(all.contains("\"stop_reason\":\"end_turn\""));
        assert!(all.contains("event: message_stop"));
    }

    #[tokio::test]
    async fn test_gemini_stream_function_call() {
        let chunks = make_gemini_sse(&[
            r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"read_file","args":{"path":"/tmp/x"}}}],"role":"model"},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5}}"#,
        ]);

        let input_stream = stream::iter(chunks);
        let output = create_anthropic_sse_stream_from_gemini(input_stream, "gemini-2.5-flash".to_string());
        tokio::pin!(output);

        let mut events: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = output.next().await {
            events.push(String::from_utf8_lossy(&bytes).to_string());
        }

        let all = events.join("");
        assert!(all.contains("\"type\":\"tool_use\""));
        assert!(all.contains("\"name\":\"read_file\""));
        assert!(all.contains("\"stop_reason\":\"tool_use\""));
    }

    #[tokio::test]
    async fn test_gemini_stream_max_tokens() {
        let chunks = make_gemini_sse(&[
            r#"{"candidates":[{"content":{"parts":[{"text":"partial"}],"role":"model"},"finishReason":"MAX_TOKENS"}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":100}}"#,
        ]);

        let input_stream = stream::iter(chunks);
        let output = create_anthropic_sse_stream_from_gemini(input_stream, "gemini-2.5-pro".to_string());
        tokio::pin!(output);

        let mut events: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = output.next().await {
            events.push(String::from_utf8_lossy(&bytes).to_string());
        }

        let all = events.join("");
        assert!(all.contains("\"stop_reason\":\"max_tokens\""));
    }
}
