use serde_json::{json, Value};

use super::{clean_schema, resolve_reasoning_effort, supports_reasoning_effort};

fn is_anthropic_web_search_tool(tool: &Value) -> bool {
    tool.get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "web_search" || kind.starts_with("web_search_"))
}

fn validate_web_search_direct_mode(tool: &Value) -> Result<(), String> {
    let tool_type = tool
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("web_search");
    let defaults_to_direct = match tool_type {
        "web_search" | "web_search_20250305" => true,
        "web_search_20260209" | "web_search_20260318" => false,
        _ => {
            return Err(format!(
                "Anthropic WebSearch version '{tool_type}' is not supported by the Responses bridge"
            ))
        }
    };
    if tool.get("response_inclusion").is_some() {
        return Err(
            "Anthropic WebSearch response_inclusion cannot be represented by the Responses bridge"
                .to_string(),
        );
    }
    match tool.get("allowed_callers") {
        None if defaults_to_direct => Ok(()),
        None => Err(format!(
            "Anthropic WebSearch version '{tool_type}' requires allowed_callers to be exactly [\"direct\"]"
        )),
        Some(Value::Array(callers))
            if callers.len() == 1 && callers[0].as_str() == Some("direct") =>
        {
            Ok(())
        }
        Some(_) => Err(
            "Anthropic WebSearch allowed_callers must be exactly [\"direct\"] for the Responses bridge"
                .to_string(),
        ),
    }
}

fn web_search_to_responses(
    tool: &Value,
    is_codex_oauth: bool,
) -> Result<(Value, Option<u64>), String> {
    validate_web_search_direct_mode(tool)?;
    if tool
        .get("blocked_domains")
        .and_then(Value::as_array)
        .is_some_and(|domains| !domains.is_empty())
    {
        return Err(
            "Anthropic WebSearch blocked_domains cannot be represented by the Responses API"
                .to_string(),
        );
    }
    let max_uses = match tool.get("max_uses") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64().filter(|limit| *limit > 0).ok_or_else(|| {
            "Anthropic WebSearch max_uses must be a positive integer".to_string()
        })?),
    };
    let mut response_tool = json!({ "type": "web_search" });
    if is_codex_oauth {
        response_tool["external_web_access"] = json!(true);
    }
    if let Some(domains) = tool
        .get("allowed_domains")
        .and_then(Value::as_array)
        .filter(|domains| !domains.is_empty())
    {
        response_tool["filters"] = json!({ "allowed_domains": domains });
    }
    if let Some(location) = tool.get("user_location").filter(|value| value.is_object()) {
        response_tool["user_location"] = location.clone();
    }
    Ok((response_tool, max_uses))
}

fn forced_hosted_web_search_name(body: &Value) -> Option<&str> {
    let selected = body
        .get("tool_choice")?
        .as_object()?
        .get("name")?
        .as_str()?;
    body.get("tools")?
        .as_array()?
        .iter()
        .any(|tool| {
            is_anthropic_web_search_tool(tool)
                && tool.get("name").and_then(Value::as_str) == Some(selected)
        })
        .then_some(selected)
}

pub fn anthropic_to_responses(body: Value, is_codex_oauth: bool) -> Result<Value, String> {
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

    let forced_web_search_name = forced_hosted_web_search_name(&body);
    let mut web_search_max_uses = None;
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let mut response_tools = Vec::new();
        for tool in tools
            .iter()
            .filter(|tool| tool.get("type").and_then(Value::as_str) != Some("BatchTool"))
        {
            if is_anthropic_web_search_tool(tool) {
                let name = tool
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("web_search");
                if forced_web_search_name.is_some_and(|selected| selected != name) {
                    continue;
                }
                let (response_tool, max_uses) = web_search_to_responses(tool, is_codex_oauth)?;
                if let Some(limit) = max_uses {
                    web_search_max_uses =
                        Some(web_search_max_uses.map_or(limit, |current: u64| current.min(limit)));
                }
                response_tools.push(response_tool);
            } else if forced_web_search_name.is_none() {
                response_tools.push(json!({
                    "type": "function",
                    "name": tool.get("name").and_then(Value::as_str).unwrap_or(""),
                    "description": tool.get("description"),
                    "parameters": clean_schema(tool.get("input_schema").cloned().unwrap_or(json!({})))
                }));
            }
        }
        if !response_tools.is_empty() {
            result["tools"] = json!(response_tools);
        }
    }

    if let Some(limit) = web_search_max_uses {
        if is_codex_oauth {
            if forced_web_search_name.is_none() {
                return Err("Anthropic WebSearch max_uses on the Codex OAuth backend requires forcing that hosted tool".to_string());
            }
            let existing = result
                .get("instructions")
                .and_then(Value::as_str)
                .unwrap_or("");
            let cap =
                format!("You must perform no more than {limit} web search calls in this response.");
            result["instructions"] = json!(if existing.is_empty() {
                cap
            } else {
                format!("{existing}\n\n{cap}")
            });
        } else {
            result["max_tool_calls"] = json!(limit);
        }
    }

    if forced_web_search_name.is_some() {
        result["tool_choice"] = json!("required");
    } else if let Some(v) = body.get("tool_choice") {
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

fn search_action_from_input(input: Option<&serde_json::Map<String, Value>>) -> Value {
    let mut action = serde_json::Map::new();
    let action_type = if input.is_some_and(|value| value.contains_key("url")) {
        "open_page"
    } else if input.is_some_and(|value| value.contains_key("pattern")) {
        "find_in_page"
    } else {
        "search"
    };
    action.insert("type".to_string(), json!(action_type));
    if let Some(input) = input {
        for key in ["query", "queries", "url", "pattern"] {
            if let Some(value) = input.get(key) {
                action.insert(key.to_string(), value.clone());
            }
        }
    }
    Value::Object(action)
}

fn responses_web_search_call_from_anthropic_blocks(
    tool_use: &Value,
    tool_result: Option<&Value>,
) -> Value {
    let id = tool_use.get("id").and_then(Value::as_str).unwrap_or("");
    let mut action = search_action_from_input(tool_use.get("input").and_then(Value::as_object));
    let failed = tool_result.is_some_and(|result| {
        result.get("is_error").and_then(Value::as_bool) == Some(true)
            || result
                .get("content")
                .and_then(Value::as_array)
                .is_some_and(|blocks| {
                    blocks.iter().any(|block| {
                        block
                            .get("type")
                            .and_then(Value::as_str)
                            .is_some_and(|kind| kind.ends_with("_error"))
                    })
                })
    });
    if let Some(results) = tool_result
        .and_then(|result| result.get("content"))
        .and_then(Value::as_array)
    {
        let sources = results
            .iter()
            .filter(|result| {
                result.get("type").and_then(Value::as_str) == Some("web_search_result")
            })
            .filter_map(|result| {
                let url = result.get("url")?.as_str()?;
                Some(json!({
                    "url": url,
                    "title": result.get("title").and_then(Value::as_str).unwrap_or(url),
                    "page_age": result.get("page_age").cloned().unwrap_or(Value::Null)
                }))
            })
            .collect::<Vec<_>>();
        if !sources.is_empty() {
            action["sources"] = json!(sources);
        }
    }
    json!({
        "type": "web_search_call",
        "id": id,
        "status": if failed { "failed" } else { "completed" },
        "action": action
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
                        "server_tool_use" => {
                            if !message_content.is_empty() {
                                input.push(
                                    json!({ "role": role, "content": message_content.clone() }),
                                );
                                message_content.clear();
                            }
                            let id = block.get("id").and_then(Value::as_str).unwrap_or("");
                            let result = blocks.iter().find(|candidate| {
                                candidate.get("type").and_then(Value::as_str)
                                    == Some("web_search_tool_result")
                                    && candidate.get("tool_use_id").and_then(Value::as_str)
                                        == Some(id)
                            });
                            input.push(responses_web_search_call_from_anthropic_blocks(
                                block, result,
                            ));
                        }
                        "web_search_tool_result" => {}
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

fn web_search_results_from_sources(item: &Value) -> Vec<Value> {
    item.pointer("/action/sources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|source| {
            let url = source.get("url")?.as_str()?;
            Some(json!({
                "type": "web_search_result",
                "url": url,
                "title": source.get("title").and_then(Value::as_str).unwrap_or(url),
                "encrypted_content": "",
                "page_age": source.get("page_age").cloned().unwrap_or(Value::Null)
            }))
        })
        .collect()
}

fn citations_from_annotations(annotations: Option<&Vec<Value>>) -> Vec<Value> {
    annotations
        .into_iter()
        .flatten()
        .filter(|annotation| annotation.get("type").and_then(Value::as_str) == Some("url_citation"))
        .filter_map(|annotation| {
            let url = annotation.get("url")?.as_str()?;
            Some(json!({
                "type": "web_search_result_location",
                "url": url,
                "title": annotation.get("title").and_then(Value::as_str).unwrap_or(url),
                "encrypted_index": "",
                "cited_text": ""
            }))
        })
        .collect()
}

fn web_search_results_from_output(output: &[Value]) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter_map(|block| block.get("annotations").and_then(Value::as_array))
        .flatten()
        .filter_map(|annotation| {
            let url = annotation.get("url")?.as_str()?;
            seen.insert(url.to_string()).then(|| {
                json!({
                    "type": "web_search_result",
                    "url": url,
                    "title": annotation.get("title").and_then(Value::as_str).unwrap_or(url),
                    "encrypted_content": "",
                    "page_age": null
                })
            })
        })
        .collect()
}

pub fn responses_to_anthropic(body: Value) -> Result<Value, String> {
    let output = body
        .get("output")
        .and_then(|o| o.as_array())
        .ok_or_else(|| "No output in response".to_string())?;

    let mut content = Vec::new();
    let mut has_tool_use = false;
    let fallback_search_results = web_search_results_from_output(output);

    for item in output {
        match item.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "message" => {
                if let Some(msg_content) = item.get("content").and_then(|c| c.as_array()) {
                    for block in msg_content {
                        match block.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                            "output_text" => {
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    if !text.is_empty() {
                                        let citations = citations_from_annotations(
                                            block.get("annotations").and_then(Value::as_array),
                                        );
                                        let mut text_block = json!({"type": "text", "text": text});
                                        if !citations.is_empty() {
                                            text_block["citations"] = json!(citations);
                                        }
                                        content.push(text_block);
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
            "web_search_call" => {
                let id = item.get("id").and_then(Value::as_str).unwrap_or("");
                let input = search_action_from_input(item.get("action").and_then(Value::as_object));
                content.push(json!({
                    "type": "server_tool_use",
                    "id": id,
                    "name": "web_search",
                    "input": input
                }));
                let results = {
                    let direct = web_search_results_from_sources(item);
                    if direct.is_empty() {
                        fallback_search_results.clone()
                    } else {
                        direct
                    }
                };
                let failed = item.get("status").and_then(Value::as_str) != Some("completed")
                    || item.get("error").is_some_and(|error| !error.is_null());
                content.push(json!({
                    "type": "web_search_tool_result",
                    "tool_use_id": id,
                    "content": if failed {
                        json!({ "type": "web_search_tool_result_error", "error_code": "unavailable" })
                    } else {
                        json!(results)
                    }
                }));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridges_anthropic_hosted_web_search_to_responses() {
        let transformed = anthropic_to_responses(
            json!({
                "model": "gpt-5",
                "messages": [{"role": "user", "content": "latest release"}],
                "tools": [{
                    "type": "web_search_20250305",
                    "name": "web_search",
                    "max_uses": 3,
                    "allowed_domains": ["example.com"]
                }]
            }),
            false,
        )
        .expect("transform");

        assert_eq!(transformed["tools"][0]["type"], "web_search");
        assert_eq!(
            transformed["tools"][0]["filters"]["allowed_domains"][0],
            "example.com"
        );
        assert_eq!(transformed["max_tool_calls"], 3);
    }

    #[test]
    fn rejects_web_search_constraints_that_cannot_be_preserved() {
        let error = anthropic_to_responses(
            json!({
                "tools": [{
                    "type": "web_search_20250305",
                    "name": "web_search",
                    "blocked_domains": ["private.example"]
                }]
            }),
            false,
        )
        .expect_err("blocked domains must fail closed");
        assert!(error.contains("blocked_domains"));
    }

    #[test]
    fn maps_web_search_calls_results_and_citations_back_to_anthropic() {
        let transformed = responses_to_anthropic(json!({
            "id": "resp_1",
            "model": "gpt-5",
            "status": "completed",
            "output": [
                {
                    "type": "web_search_call",
                    "id": "ws_1",
                    "status": "completed",
                    "action": {
                        "type": "search",
                        "query": "CCHub",
                        "sources": [{"url": "https://example.com", "title": "Example"}]
                    }
                },
                {
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": "Result",
                        "annotations": [{
                            "type": "url_citation",
                            "url": "https://example.com",
                            "title": "Example"
                        }]
                    }]
                }
            ],
            "usage": {"input_tokens": 10, "output_tokens": 4}
        }))
        .expect("response transform");

        assert_eq!(transformed["content"][0]["type"], "server_tool_use");
        assert_eq!(transformed["content"][1]["type"], "web_search_tool_result");
        assert_eq!(
            transformed["content"][1]["content"][0]["url"],
            "https://example.com"
        );
        assert_eq!(
            transformed["content"][2]["citations"][0]["url"],
            "https://example.com"
        );
        assert_eq!(transformed["stop_reason"], "end_turn");
    }

    #[test]
    fn codex_oauth_requires_forced_search_before_emulating_max_uses() {
        let error = anthropic_to_responses(
            json!({
                "tools": [{
                    "type": "web_search_20250305",
                    "name": "web_search",
                    "max_uses": 1
                }]
            }),
            true,
        )
        .expect_err("unforced cap is not representable");
        assert!(error.contains("requires forcing"));
    }
}
