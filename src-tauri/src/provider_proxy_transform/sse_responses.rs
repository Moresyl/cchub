use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::responses::{build_anthropic_usage_from_responses, map_responses_stop_reason};
use super::strip_sse_field;

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
