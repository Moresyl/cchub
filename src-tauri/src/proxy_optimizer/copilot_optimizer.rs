use std::collections::HashSet;

use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CopilotClassification {
    pub initiator: &'static str,
    pub is_warmup: bool,
    pub is_compact: bool,
    pub is_subagent: bool,
}

pub fn classify_request(
    body: &Value,
    has_anthropic_beta: bool,
    compact_detection: bool,
    subagent_detection: bool,
) -> CopilotClassification {
    let is_compact = compact_detection && is_compact_request(body);
    let is_subagent = subagent_detection && detect_subagent(body);

    let messages = match body.get("messages").and_then(|m| m.as_array()) {
        Some(msgs) if !msgs.is_empty() => msgs,
        _ => {
            return CopilotClassification {
                initiator: "user",
                is_warmup: is_warmup_request(body, has_anthropic_beta, false),
                is_compact: false,
                is_subagent,
            }
        }
    };

    let last_msg = &messages[messages.len() - 1];
    let role = last_msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

    if role != "user" {
        return CopilotClassification {
            initiator: if is_subagent { "agent" } else { "user" },
            is_warmup: false,
            is_compact,
            is_subagent,
        };
    }

    let is_user_initiated = match last_msg.get("content") {
        Some(content) if content.is_array() => {
            let blocks = content.as_array().unwrap();
            !blocks
                .iter()
                .any(|block| block.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
        }
        Some(content) if content.is_string() => true,
        _ => false,
    };

    let initiator = if is_subagent || !is_user_initiated || is_compact {
        "agent"
    } else {
        "user"
    };

    CopilotClassification {
        initiator,
        is_warmup: initiator == "user" && is_warmup_request(body, has_anthropic_beta, is_compact),
        is_compact,
        is_subagent,
    }
}

fn is_warmup_request(body: &Value, has_anthropic_beta: bool, is_compact: bool) -> bool {
    if !has_anthropic_beta || is_compact {
        return false;
    }
    !matches!(body.get("tools"), Some(tools) if tools.is_array() && !tools.as_array().unwrap().is_empty())
}

fn is_compact_request(body: &Value) -> bool {
    let system_text = extract_system_text(body);
    if system_text.starts_with("You are a helpful AI assistant tasked with summarizing conversations") {
        return true;
    }

    let messages = match body.get("messages").and_then(|m| m.as_array()) {
        Some(msgs) => msgs,
        None => return false,
    };

    if let Some(last_msg) = messages.last() {
        if last_msg.get("role").and_then(|r| r.as_str()) != Some("user") {
            return false;
        }

        let text = extract_text_from_message(last_msg);

        if text.contains("CRITICAL: Respond with TEXT ONLY. Do NOT call any tools.") {
            return true;
        }

        if text.contains("Pending Tasks:") && text.contains("Current Work:") {
            return true;
        }
    }

    false
}

pub fn merge_tool_results(mut body: Value) -> Value {
    let messages = match body.get_mut("messages").and_then(|m| m.as_array_mut()) {
        Some(msgs) if !msgs.is_empty() => msgs,
        _ => return body,
    };

    for msg in messages.iter_mut() {
        if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        let content = match msg.get("content").and_then(|c| c.as_array()) {
            Some(blocks) => blocks,
            None => continue,
        };

        let mut tool_results: Vec<Value> = Vec::new();
        let mut text_blocks: Vec<Value> = Vec::new();
        let mut valid = true;

        for block in content {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("tool_result") => tool_results.push(block.clone()),
                Some("text") => text_blocks.push(block.clone()),
                _ => {
                    valid = false;
                    break;
                }
            }
        }

        if !valid || tool_results.is_empty() || text_blocks.is_empty() {
            continue;
        }

        let merged = merge_blocks_into_tool_results(tool_results, text_blocks);
        msg["content"] = Value::Array(merged);
    }

    let messages = body["messages"].as_array().unwrap().clone();
    if messages.len() <= 1 {
        return body;
    }

    let mut merged_msgs: Vec<Value> = Vec::with_capacity(messages.len());
    let mut i = 0;

    while i < messages.len() {
        if is_tool_result_only_message(&messages[i]) {
            let mut combined_content: Vec<Value> = Vec::new();
            while i < messages.len() && is_tool_result_only_message(&messages[i]) {
                if let Some(content) = messages[i].get("content").and_then(|c| c.as_array()) {
                    combined_content.extend(content.iter().cloned());
                }
                i += 1;
            }
            if !combined_content.is_empty() {
                merged_msgs.push(serde_json::json!({
                    "role": "user",
                    "content": combined_content
                }));
            }
        } else {
            merged_msgs.push(messages[i].clone());
            i += 1;
        }
    }

    body["messages"] = Value::Array(merged_msgs);
    body
}

pub fn deterministic_request_id(body: &Value, session_id: &str) -> String {
    let last_user_content = find_last_user_content(body);

    match last_user_content {
        Some(content) => {
            let mut hasher = Sha256::new();
            hasher.update(session_id.as_bytes());
            hasher.update(content.as_bytes());
            let result = hasher.finalize();

            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&result[..16]);
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;

            Uuid::from_bytes(bytes).to_string()
        }
        None => Uuid::new_v4().to_string(),
    }
}

pub fn deterministic_interaction_id(session_id: &str) -> Option<String> {
    if session_id.is_empty() {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(b"interaction:");
    hasher.update(session_id.as_bytes());
    let result = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&result[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    Some(Uuid::from_bytes(bytes).to_string())
}

fn detect_subagent(body: &Value) -> bool {
    if extract_system_text(body).contains("__SUBAGENT_MARKER__") {
        return true;
    }

    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
                continue;
            }
            let text = extract_text_from_message(msg);
            if text.contains("__SUBAGENT_MARKER__") {
                return true;
            }
        }
    }

    if let Some(user_id) = body.pointer("/metadata/user_id").and_then(|v| v.as_str()) {
        if user_id.contains("_agent_") {
            return true;
        }
    }

    false
}

pub fn sanitize_orphan_tool_results(mut body: Value) -> Value {
    let messages = match body.get_mut("messages").and_then(|m| m.as_array_mut()) {
        Some(msgs) if msgs.len() >= 2 => msgs,
        _ => return body,
    };

    for i in 1..messages.len() {
        if messages[i].get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }

        let prev_tool_use_ids: HashSet<String> =
            if messages[i - 1].get("role").and_then(|r| r.as_str()) == Some("assistant") {
                messages[i - 1]
                    .get("content")
                    .and_then(|c| c.as_array())
                    .map(|blocks| {
                        blocks
                            .iter()
                            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                            .filter_map(|b| b.get("id").and_then(|id| id.as_str()).map(String::from))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                HashSet::new()
            };

        let content = match messages[i].get_mut("content").and_then(|c| c.as_array_mut()) {
            Some(blocks) => blocks,
            None => continue,
        };

        for block in content.iter_mut() {
            if block.get("type").and_then(|t| t.as_str()) != Some("tool_result") {
                continue;
            }
            let tool_use_id = block
                .get("tool_use_id")
                .and_then(|id| id.as_str())
                .unwrap_or("");
            if tool_use_id.is_empty() || !prev_tool_use_ids.contains(tool_use_id) {
                let content_text = match block.get("content") {
                    Some(c) if c.is_string() => c.as_str().unwrap_or("").to_string(),
                    Some(c) if c.is_array() => c
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => String::new(),
                };
                *block = serde_json::json!({
                    "type": "text",
                    "text": format!("[Tool result for {}]: {}", tool_use_id, content_text)
                });
            }
        }
    }

    body
}

pub fn strip_thinking_blocks(mut body: Value) -> Value {
    let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
        return body;
    };

    for msg in messages.iter_mut() {
        if msg.get("role").and_then(|r| r.as_str()) != Some("assistant") {
            continue;
        }
        let Some(content) = msg.get_mut("content").and_then(|c| c.as_array_mut()) else {
            continue;
        };
        content.retain(|block| {
            !matches!(
                block.get("type").and_then(|t| t.as_str()),
                Some("thinking") | Some("redacted_thinking")
            )
        });
    }

    body
}

// ─── 内部辅助 ─────────────────────────────────

fn extract_system_text(body: &Value) -> String {
    match body.get("system") {
        Some(s) if s.is_string() => s.as_str().unwrap_or("").to_string(),
        Some(arr) if arr.is_array() => arr
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn find_last_user_content(body: &Value) -> Option<String> {
    let messages = body.get("messages").and_then(|m| m.as_array())?;

    for msg in messages.iter().rev() {
        if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        let content = msg.get("content")?;

        if let Some(s) = content.as_str() {
            return Some(s.to_string());
        }

        if let Some(blocks) = content.as_array() {
            let filtered: Vec<Value> = blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) != Some("tool_result"))
                .map(|b| {
                    let mut b = b.clone();
                    if let Some(obj) = b.as_object_mut() {
                        obj.remove("cache_control");
                    }
                    b
                })
                .collect();

            if !filtered.is_empty() {
                return Some(serde_json::to_string(&filtered).unwrap_or_default());
            }
        }
    }

    None
}

fn merge_blocks_into_tool_results(
    mut tool_results: Vec<Value>,
    text_blocks: Vec<Value>,
) -> Vec<Value> {
    if tool_results.len() == text_blocks.len() {
        for (tr, tb) in tool_results.iter_mut().zip(text_blocks.iter()) {
            append_text_to_tool_result(tr, tb);
        }
    } else {
        if let Some(last_tr) = tool_results.last_mut() {
            for tb in &text_blocks {
                append_text_to_tool_result(last_tr, tb);
            }
        }
    }
    tool_results
}

fn append_text_to_tool_result(tool_result: &mut Value, text_block: &Value) {
    let text = text_block
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if text.trim().is_empty() {
        return;
    }

    match tool_result.get("content") {
        Some(c) if c.is_string() => {
            let existing = c.as_str().unwrap_or("");
            tool_result["content"] = Value::String(format!("{existing}\n{text}"));
        }
        Some(c) if c.is_array() => {
            let arr = tool_result["content"].as_array_mut().unwrap();
            arr.push(serde_json::json!({"type": "text", "text": text}));
        }
        _ => {
            tool_result["content"] = Value::String(text.to_string());
        }
    }
}

fn extract_text_from_message(msg: &Value) -> String {
    match msg.get("content") {
        Some(content) if content.is_string() => content.as_str().unwrap_or("").to_string(),
        Some(content) if content.is_array() => {
            let blocks = content.as_array().unwrap();
            blocks
                .iter()
                .filter_map(|block| {
                    if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                        block.get("text").and_then(|t| t.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
        _ => String::new(),
    }
}

fn is_tool_result_only_message(msg: &Value) -> bool {
    if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
        return false;
    }
    match msg.get("content").and_then(|c| c.as_array()) {
        Some(blocks) if !blocks.is_empty() => blocks
            .iter()
            .all(|block| block.get("type").and_then(|t| t.as_str()) == Some("tool_result")),
        _ => false,
    }
}

pub fn normalize_copilot_model_id(model: &str) -> Option<String> {
    if !model.starts_with("claude-") {
        return None;
    }

    let mut s = model.to_string();

    // [1m] → -1m
    if let Some(bracket_pos) = s.find('[') {
        if let Some(close_pos) = s[bracket_pos..].find(']') {
            let inner = s[bracket_pos + 1..bracket_pos + close_pos].to_string();
            s = format!("{}-{}", &s[..bracket_pos], inner);
        }
    }

    // strip 8-digit date suffix
    let parts: Vec<&str> = s.split('-').collect();
    let has_date = parts.last().is_some_and(|p| p.len() == 8 && p.chars().all(|c| c.is_ascii_digit()));
    if has_date {
        s = parts[..parts.len() - 1].join("-");
    }

    // convert consecutive numeric segments joined by dash to dot notation
    // e.g. "3-5" → "3.5", "4-6" → "4.6"
    let final_parts: Vec<&str> = s.split('-').collect();
    let mut result_parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < final_parts.len() {
        if !final_parts[i].is_empty()
            && final_parts[i].chars().all(|c| c.is_ascii_digit())
            && i + 1 < final_parts.len()
            && !final_parts[i + 1].is_empty()
            && final_parts[i + 1].chars().all(|c| c.is_ascii_digit())
        {
            result_parts.push(format!("{}.{}", final_parts[i], final_parts[i + 1]));
            i += 2;
        } else {
            result_parts.push(final_parts[i].to_string());
            i += 1;
        }
    }
    s = result_parts.join("-");

    if s == model {
        return None;
    }
    Some(s)
}

pub fn apply_copilot_model_normalization(body: &mut Value) {
    if let Some(model) = body.get("model").and_then(|m| m.as_str()).map(|s| s.to_string()) {
        if let Some(normalized) = normalize_copilot_model_id(&model) {
            body["model"] = Value::String(normalized);
        }
    }
}

// ─── 测试 ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_classify_user_text_message() {
        let body = json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let result = classify_request(&body, false, true, false);
        assert_eq!(result.initiator, "user");
        assert!(!result.is_compact);
    }

    #[test]
    fn test_classify_tool_result_only() {
        let body = json!({
            "messages": [
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "toolu_123", "content": "file contents"}
                ]}
            ]
        });
        let result = classify_request(&body, true, true, false);
        assert_eq!(result.initiator, "agent");
        assert!(!result.is_warmup);
    }

    #[test]
    fn test_classify_compact_system_prompt() {
        let body = json!({
            "system": "You are a helpful AI assistant tasked with summarizing conversations. Please provide a concise summary.",
            "messages": [{"role": "user", "content": "Summarize"}]
        });
        let result = classify_request(&body, false, true, false);
        assert_eq!(result.initiator, "agent");
        assert!(result.is_compact);
    }

    #[test]
    fn test_classify_compact_disabled() {
        let body = json!({
            "system": "You are a helpful AI assistant tasked with summarizing conversations.",
            "messages": [{"role": "user", "content": "Summarize"}]
        });
        let result = classify_request(&body, false, false, false);
        assert_eq!(result.initiator, "user");
        assert!(!result.is_compact);
    }

    #[test]
    fn test_warmup_with_beta_no_tools() {
        let body = json!({
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let result = classify_request(&body, true, true, false);
        assert!(result.is_warmup);
    }

    #[test]
    fn test_not_warmup_with_tools() {
        let body = json!({
            "tools": [{"name": "Read", "description": "Read", "input_schema": {}}],
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let result = classify_request(&body, true, true, false);
        assert!(!result.is_warmup);
    }

    #[test]
    fn test_subagent_detection_marker() {
        let body = json!({
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "<system-reminder>\n{\"__SUBAGENT_MARKER__\":{\"session_id\":\"abc\"}}\n</system-reminder>\nSearch"}
                ]}
            ]
        });
        let result = classify_request(&body, false, true, true);
        assert_eq!(result.initiator, "agent");
        assert!(result.is_subagent);
    }

    #[test]
    fn test_subagent_detection_disabled() {
        let body = json!({
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "{\"__SUBAGENT_MARKER__\":{}} Search"}
                ]}
            ]
        });
        let result = classify_request(&body, false, true, false);
        assert_eq!(result.initiator, "user");
        assert!(!result.is_subagent);
    }

    #[test]
    fn test_merge_tool_results_intra_message() {
        let body = json!({
            "messages": [
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "t1", "content": "file contents"},
                    {"type": "text", "text": "skill output"}
                ]}
            ]
        });
        let result = merge_tool_results(body);
        let content = result["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "tool_result");
        assert!(content[0]["content"].as_str().unwrap().contains("skill output"));
    }

    #[test]
    fn test_merge_cross_message() {
        let body = json!({
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "t1", "name": "Read", "input": {}},
                    {"type": "tool_use", "id": "t2", "name": "Read", "input": {}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "t1", "content": "file1"}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "t2", "content": "file2"}
                ]}
            ]
        });
        let result = merge_tool_results(body);
        let messages = result["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        let merged = messages[1]["content"].as_array().unwrap();
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_deterministic_request_id_stable() {
        let body = json!({"messages": [{"role": "user", "content": "Hello"}]});
        let id1 = deterministic_request_id(&body, "session1");
        let id2 = deterministic_request_id(&body, "session1");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_deterministic_request_id_varies_by_session() {
        let body = json!({"messages": [{"role": "user", "content": "Hello"}]});
        let id1 = deterministic_request_id(&body, "s1");
        let id2 = deterministic_request_id(&body, "s2");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_interaction_id_stable() {
        let id1 = deterministic_interaction_id("session_abc");
        let id2 = deterministic_interaction_id("session_abc");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_interaction_id_empty_is_none() {
        assert!(deterministic_interaction_id("").is_none());
    }

    #[test]
    fn test_sanitize_orphan_tool_results_basic() {
        let body = json!({
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "tool_1", "name": "read", "input": {}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "tool_1", "content": "ok"},
                    {"type": "tool_result", "tool_use_id": "orphan", "content": "data"}
                ]}
            ]
        });
        let result = sanitize_orphan_tool_results(body);
        let content = result["messages"][1]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_result");
        assert_eq!(content[1]["type"], "text");
    }

    #[test]
    fn test_strip_thinking_blocks_basic() {
        let body = json!({
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "hmm"},
                    {"type": "text", "text": "hello"},
                    {"type": "redacted_thinking", "data": "x"}
                ]}
            ]
        });
        let result = strip_thinking_blocks(body);
        let content = result["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["text"], "hello");
    }

    #[test]
    fn test_normalize_copilot_model_id() {
        assert_eq!(
            normalize_copilot_model_id("claude-sonnet-4-6-20250514"),
            Some("claude-sonnet-4.6".to_string())
        );
        assert_eq!(
            normalize_copilot_model_id("claude-opus-4-6-20250514"),
            Some("claude-opus-4.6".to_string())
        );
        assert_eq!(
            normalize_copilot_model_id("claude-3-5-sonnet-20241022"),
            Some("claude-3.5-sonnet".to_string())
        );
        assert_eq!(
            normalize_copilot_model_id("claude-3-5-haiku-20241022"),
            Some("claude-3.5-haiku".to_string())
        );
        assert_eq!(
            normalize_copilot_model_id("claude-sonnet-4-6"),
            Some("claude-sonnet-4.6".to_string())
        );
        assert_eq!(
            normalize_copilot_model_id("claude-sonnet-4-6[1m]"),
            Some("claude-sonnet-4.6-1m".to_string())
        );
        assert_eq!(normalize_copilot_model_id("gpt-4o"), None);
        assert_eq!(normalize_copilot_model_id("claude-sonnet-4.6"), None);
    }
}
