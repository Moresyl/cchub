use serde_json::{json, Value};

pub fn anthropic_to_gemini(body: Value) -> Result<(Value, String), String> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("gemini-3.6-flash")
        .to_string();

    let model_id = normalize_gemini_model(&model);

    let mut gemini_body = json!({});

    // system instruction
    if let Some(system) = body.get("system") {
        let text = extract_system_text(system);
        if !text.is_empty() {
            gemini_body["systemInstruction"] = json!({
                "parts": [{"text": text}]
            });
        }
    }

    // contents (messages)
    let mut contents: Vec<Value> = Vec::new();
    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let gemini_role = match role {
                "assistant" => "model",
                _ => "user",
            };

            let parts = convert_content_to_parts(msg.get("content"), role);
            if !parts.is_empty() {
                contents.push(json!({
                    "role": gemini_role,
                    "parts": parts
                }));
            }
        }
    }
    gemini_body["contents"] = Value::Array(contents);

    // tools
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let declarations: Vec<Value> = tools
            .iter()
            .filter_map(|tool| {
                let name = tool.get("name").and_then(|n| n.as_str())?;
                let description = tool
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                let input_schema = tool
                    .get("input_schema")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
                Some(json!({
                    "name": name,
                    "description": description,
                    "parameters": sanitize_schema_for_gemini(input_schema)
                }))
            })
            .collect();

        if !declarations.is_empty() {
            gemini_body["tools"] = json!([{"functionDeclarations": declarations}]);
        }
    }

    // generation config
    let mut gen_config = json!({});
    if let Some(max_tokens) = body.get("max_tokens").and_then(|m| m.as_u64()) {
        gen_config["maxOutputTokens"] = json!(max_tokens);
    }
    if let Some(temperature) = body.get("temperature") {
        gen_config["temperature"] = temperature.clone();
    }
    if let Some(top_p) = body.get("top_p") {
        gen_config["topP"] = top_p.clone();
    }
    if let Some(stop) = body.get("stop_sequences").and_then(|s| s.as_array()) {
        gen_config["stopSequences"] = Value::Array(stop.clone());
    }
    if gen_config.as_object().is_some_and(|o| !o.is_empty()) {
        gemini_body["generationConfig"] = gen_config;
    }

    // streaming check
    let stream = body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    if stream {
        gemini_body["_stream"] = json!(true);
    }

    Ok((gemini_body, model_id))
}

pub fn gemini_to_anthropic(gemini_response: Value, model: &str) -> Result<Value, String> {
    let candidates = gemini_response
        .get("candidates")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "Missing candidates in Gemini response".to_string())?;

    let candidate = candidates
        .first()
        .ok_or_else(|| "Empty candidates array".to_string())?;

    let parts = candidate
        .get("content")
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .unwrap_or(&Vec::new())
        .clone();

    let mut content: Vec<Value> = Vec::new();
    for part in &parts {
        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
            content.push(json!({"type": "text", "text": text}));
        } else if let Some(fc) = part.get("functionCall") {
            let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
            let args = fc.get("args").cloned().unwrap_or(json!({}));
            let id = format!("toolu_{}", generate_tool_id());
            content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": args
            }));
        }
    }

    let stop_reason = match gemini_response
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("finishReason"))
        .and_then(|r| r.as_str())
    {
        Some("STOP") => "end_turn",
        Some("MAX_TOKENS") => "max_tokens",
        Some("SAFETY") => "end_turn",
        Some("RECITATION") => "end_turn",
        _ => "end_turn",
    };

    let has_tool_use = content
        .iter()
        .any(|c| c.get("type").and_then(|t| t.as_str()) == Some("tool_use"));
    let actual_stop_reason = if has_tool_use {
        "tool_use"
    } else {
        stop_reason
    };

    let usage = gemini_response
        .get("usageMetadata")
        .cloned()
        .unwrap_or(json!({}));
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("candidatesTokenCount")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    Ok(json!({
        "id": format!("msg_{}", generate_msg_id()),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": model,
        "stop_reason": actual_stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        }
    }))
}

pub fn build_gemini_endpoint(model_id: &str, stream: bool) -> String {
    let action = if stream {
        "streamGenerateContent?alt=sse"
    } else {
        "generateContent"
    };
    format!("v1beta/models/{}:{}", model_id, action)
}

fn normalize_gemini_model(model: &str) -> String {
    let stripped = model
        .strip_prefix('/')
        .unwrap_or(model)
        .strip_prefix("models/")
        .unwrap_or(model);
    stripped.to_string()
}

fn extract_system_text(system: &Value) -> String {
    match system {
        s if s.is_string() => s.as_str().unwrap_or("").to_string(),
        arr if arr.is_array() => arr
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn convert_content_to_parts(content: Option<&Value>, role: &str) -> Vec<Value> {
    let content = match content {
        Some(c) => c,
        None => return Vec::new(),
    };

    if let Some(text) = content.as_str() {
        return vec![json!({"text": text})];
    }

    let blocks = match content.as_array() {
        Some(blocks) => blocks,
        None => return Vec::new(),
    };

    let mut parts: Vec<Value> = Vec::new();
    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match block_type {
            "text" => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    parts.push(json!({"text": text}));
                }
            }
            "tool_use" if role == "assistant" => {
                let name = block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                let input = block.get("input").cloned().unwrap_or(json!({}));
                parts.push(json!({
                    "functionCall": {
                        "name": name,
                        "args": input
                    }
                }));
            }
            "tool_result" if role == "user" => {
                let name = block
                    .get("tool_use_id")
                    .and_then(|id| id.as_str())
                    .unwrap_or("unknown");
                let result_content = match block.get("content") {
                    Some(c) if c.is_string() => c.as_str().unwrap_or("").to_string(),
                    Some(c) if c.is_array() => c
                        .as_array()
                        .map(|blocks| {
                            blocks
                                .iter()
                                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                parts.push(json!({
                    "functionResponse": {
                        "name": name,
                        "response": {"result": result_content}
                    }
                }));
            }
            "thinking" | "redacted_thinking" => {
                // skip thinking blocks
            }
            _ => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    parts.push(json!({"text": text}));
                }
            }
        }
    }

    parts
}

fn sanitize_schema_for_gemini(mut schema: Value) -> Value {
    if let Some(obj) = schema.as_object_mut() {
        obj.remove("$schema");
        obj.remove("$id");
        obj.entry("type".to_string())
            .or_insert_with(|| json!("object"));
        if obj.get("type").and_then(|v| v.as_str()) == Some("object") {
            obj.entry("properties".to_string())
                .or_insert_with(|| json!({}));
        }
    }
    schema
}

fn generate_tool_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:08x}{:04x}", nanos, rand_u16())
}

fn generate_msg_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{:012x}", millis & 0xffffffffffff)
}

fn rand_u16() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    ((t.as_nanos() >> 10) & 0xffff) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_anthropic_to_gemini_basic() {
        let body = json!({
            "model": "gemini-2.5-flash",
            "max_tokens": 4096,
            "system": "You are a helpful assistant.",
            "messages": [
                {"role": "user", "content": "Hello!"},
                {"role": "assistant", "content": "Hi there!"},
                {"role": "user", "content": "How are you?"}
            ]
        });

        let (result, model_id) = anthropic_to_gemini(body).unwrap();
        assert_eq!(model_id, "gemini-2.5-flash");
        assert!(result.get("systemInstruction").is_some());
        let contents = result["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[2]["role"], "user");
    }

    #[test]
    fn test_anthropic_to_gemini_with_tools() {
        let body = json!({
            "model": "gemini-2.5-pro",
            "max_tokens": 1024,
            "tools": [
                {"name": "read_file", "description": "Read a file", "input_schema": {"type": "object", "properties": {"path": {"type": "string"}}}}
            ],
            "messages": [{"role": "user", "content": "Read /tmp/test.txt"}]
        });

        let (result, _) = anthropic_to_gemini(body).unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        let declarations = tools[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(declarations[0]["name"], "read_file");
    }

    #[test]
    fn test_anthropic_to_gemini_tool_use_and_result() {
        let body = json!({
            "model": "gemini-2.5-flash",
            "max_tokens": 4096,
            "messages": [
                {"role": "user", "content": "Read the file"},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "toolu_123", "name": "read_file", "input": {"path": "/tmp/x"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "toolu_123", "content": "file content here"}
                ]}
            ]
        });

        let (result, _) = anthropic_to_gemini(body).unwrap();
        let contents = result["contents"].as_array().unwrap();
        assert_eq!(contents[1]["parts"][0]["functionCall"]["name"], "read_file");
        assert!(contents[2]["parts"][0].get("functionResponse").is_some());
    }

    #[test]
    fn test_gemini_to_anthropic_text_response() {
        let gemini_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello! How can I help you?"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 8
            }
        });

        let result = gemini_to_anthropic(gemini_response, "gemini-2.5-flash").unwrap();
        assert_eq!(result["type"], "message");
        assert_eq!(result["role"], "assistant");
        assert_eq!(result["stop_reason"], "end_turn");
        let content = result["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Hello! How can I help you?");
    }

    #[test]
    fn test_gemini_to_anthropic_function_call() {
        let gemini_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "read_file",
                            "args": {"path": "/tmp/test.txt"}
                        }
                    }],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 15,
                "candidatesTokenCount": 5
            }
        });

        let result = gemini_to_anthropic(gemini_response, "gemini-2.5-flash").unwrap();
        assert_eq!(result["stop_reason"], "tool_use");
        let content = result["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_use");
        assert_eq!(content[0]["name"], "read_file");
    }

    #[test]
    fn test_build_gemini_endpoint() {
        assert_eq!(
            build_gemini_endpoint("gemini-2.5-flash", false),
            "v1beta/models/gemini-2.5-flash:generateContent"
        );
        assert_eq!(
            build_gemini_endpoint("gemini-2.5-pro", true),
            "v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn test_normalize_gemini_model() {
        assert_eq!(
            normalize_gemini_model("gemini-2.5-flash"),
            "gemini-2.5-flash"
        );
        assert_eq!(
            normalize_gemini_model("models/gemini-2.5-pro"),
            "gemini-2.5-pro"
        );
        assert_eq!(
            normalize_gemini_model("/models/gemini-2.5-pro"),
            "gemini-2.5-pro"
        );
    }
}
