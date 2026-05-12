use serde_json::{json, Value};

use super::{clean_schema, resolve_reasoning_effort, supports_reasoning_effort};

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

pub(super) fn map_responses_stop_reason(
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
