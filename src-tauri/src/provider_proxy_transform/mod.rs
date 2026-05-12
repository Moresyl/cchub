mod responses;
mod sse_chat;
mod sse_gemini;
mod sse_responses;
pub use responses::{anthropic_to_responses, responses_to_anthropic};
pub use sse_chat::create_anthropic_sse_stream;
pub use sse_gemini::create_anthropic_sse_stream_from_gemini;
pub use sse_responses::create_anthropic_sse_stream_from_responses;

use serde_json::{json, Value};

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
        obj.entry("instructions".to_string())
            .or_insert(Value::String(String::new()));
        obj.entry("tools".to_string())
            .or_insert(Value::Array(Vec::new()));
        obj.entry("parallel_tool_calls".to_string())
            .or_insert(Value::Bool(false));
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

pub(super) fn strip_sse_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    line.strip_prefix(field)
        .and_then(|rest| rest.strip_prefix(':'))
        .map(str::trim_start)
}
