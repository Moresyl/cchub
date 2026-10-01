use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::responses::map_responses_stop_reason;
use super::responses_reasoning::ReasoningBlocks;
use super::stream_limits::{allocate, BLOCK_LIMIT};
use super::strip_sse_field;
use crate::shared::token_usage::{InputTokenBasis, TokenUsage};

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
) -> Option<u32> {
    if let Some(k) = content_part_key(data) {
        if let Some(existing) = index_by_key.get(&k).copied() {
            Some(existing)
        } else {
            let assigned = allocate(next_content_index)?;
            index_by_key.insert(k, assigned);
            Some(assigned)
        }
    } else if let Some(existing) = *fallback_open_index {
        Some(existing)
    } else {
        let assigned = allocate(next_content_index)?;
        *fallback_open_index = Some(assigned);
        Some(assigned)
    }
}

fn web_search_action_input(item: &Value) -> Value {
    let Some(action) = item.get("action").and_then(Value::as_object) else {
        return json!({});
    };
    let mut input = serde_json::Map::new();
    for key in ["query", "queries", "url", "pattern"] {
        if let Some(value) = action.get(key) {
            input.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(input)
}

fn web_search_result_content(item: &Value) -> Value {
    if item.get("status").and_then(Value::as_str) != Some("completed")
        || item.get("error").is_some_and(|error| !error.is_null())
    {
        return json!({
            "type": "web_search_tool_result_error",
            "error_code": "unavailable"
        });
    }
    let results = item
        .pointer("/action/sources")
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
        .collect::<Vec<_>>();
    json!(results)
}

pub fn create_anthropic_sse_stream_from_responses<E: std::error::Error + Send + 'static>(
    stream: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send {
    async_stream::stream! {
        let stream = super::stream_frames::frames(stream);
        let mut buffer = String::new();
        let mut message_id = super::anthropic_message_id(None);
        let mut current_model: Option<String> = None;
        let mut has_sent_message_start = false;
        let mut usage_state = TokenUsage::default();
        let mut has_usage = false;
        let mut has_tool_use = false;
        let mut next_content_index: u32 = 0;
        let mut index_by_key: HashMap<String, u32> = HashMap::new();
        let mut open_indices: HashSet<u32> = HashSet::new();
        let mut fallback_open_index: Option<u32> = None;
        let mut current_text_index: Option<u32> = None;
        let mut tool_index_by_item_id: HashMap<String, u32> = HashMap::new();
        let mut last_tool_index: Option<u32> = None;
        let mut reasoning = ReasoningBlocks::default();

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
                        for line in block.trim_start_matches('\u{feff}').lines() {
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
                        let data: Value = match serde_json::from_str(&data_str) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        if !super::stream_limits::valid_response_ids(&data) {
                            yield Ok(super::stream_errors::api_error_event("Upstream content identity exceeded the byte limit")); return;
                        }
                        let event_name = event_type.as_deref().filter(|event| !event.is_empty())
                            .or_else(|| data.get("type").and_then(Value::as_str)).unwrap_or("");
                        if let Some(reading) = response_object_from_event(&data).get("usage").and_then(TokenUsage::parse) {
                            usage_state.merge(&reading);
                            has_usage = true;
                        }

                        if let Some(error) = super::stream_errors::error_event(&data, Some(event_name)) {
                            yield Ok(error);
                            return;
                        }
                        if !has_sent_message_start && (matches!(event_name,
                            "response.created" | "response.content_part.added" |
                            "response.output_text.delta" | "response.refusal.delta" |
                            "response.output_item.added" | "response.output_item.done" |
                            "response.function_call_arguments.delta" | "response.reasoning.delta" |
                            "response.completed") || ReasoningBlocks::handles(event_name)) {
                            if matches!(event_name, "response.created" | "response.completed") {
                                let response_obj = response_object_from_event(&data);
                                message_id = super::anthropic_message_id(response_obj.get("id").and_then(Value::as_str));
                                if let Some(model) = response_obj.get("model").and_then(|m| m.as_str()) {
                                    current_model = Some(model.to_string());
                                }
                            }
                            let mut usage = usage_state.anthropic(InputTokenBasis::IncludesCache);
                            usage["output_tokens"] = json!(0);
                            let start = json!({"type":"message_start", "message":{
                                "id":message_id.clone(), "type":"message", "role":"assistant",
                                "model":current_model.clone().unwrap_or_default(), "usage":usage
                            }});
                            yield Ok(Bytes::from(format!("event: message_start\ndata: {start}\n\n")));
                            has_sent_message_start = true;
                        }
                        let reasoning_item_done = event_name == "response.output_item.done" && data.pointer("/item/type").and_then(Value::as_str) == Some("reasoning");
                        if ReasoningBlocks::handles(event_name) || reasoning_item_done {
                            let result = if reasoning_item_done {
                                reasoning.finish_item(&data, &mut next_content_index)
                            } else { reasoning.handle(event_name, &data, &mut next_content_index) };
                            let events = match result {
                                Ok(events) => events,
                                Err(message) => {
                                    let error = json!({"type":"error", "error":{"type":"api_error", "message":message}});
                                    yield Ok(Bytes::from(format!("event: error\ndata: {error}\n\n")));
                                    return;
                                }
                            };
                            if events.iter().any(|event| matches!(event["type"].as_str(), Some("content_block_start" | "content_block_delta"))) {
                                if let Some(index) = current_text_index.take() {
                                    if open_indices.remove(&index) {
                                        let stop = json!({"type":"content_block_stop", "index":index});
                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {stop}\n\n")));
                                    }
                                    if fallback_open_index == Some(index) { fallback_open_index = None; }
                                }
                            }
                            for event in events {
                                yield Ok(Bytes::from(format!("event: {}\ndata: {event}\n\n", event["type"].as_str().expect("reasoning event type"))));
                            }
                            continue;
                        }
                        let opens_other_content = matches!(event_name, "response.output_text.delta" | "response.refusal.delta")
                            || (event_name == "response.content_part.added" && matches!(data.pointer("/part/type").and_then(Value::as_str), Some("output_text" | "refusal")))
                            || event_name == "response.function_call_arguments.delta"
                            || (event_name == "response.output_item.added" && matches!(data.pointer("/item/type").and_then(Value::as_str), Some("function_call" | "web_search_call")));
                        if opens_other_content || event_name == "response.completed" {
                            for event in reasoning.close_all() {
                                yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {event}\n\n")));
                            }
                        }
                        match event_name {
                            "response.created" => {}
                            "response.content_part.added" => {

                                if let Some(part) = data.get("part") {
                                    let part_type = part.get("type").and_then(|t| t.as_str());
                                    if matches!(part_type, Some("output_text") | Some("refusal")) {
                                        let index = if let Some(index) = current_text_index {
                                            index
                                        } else {
                                            let Some(index) = resolve_content_index(&data, &mut next_content_index, &mut index_by_key, &mut fallback_open_index) else {
                                                yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                            };
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
                                        let Some(index) = resolve_content_index(&data, &mut next_content_index, &mut index_by_key, &mut fallback_open_index) else {
                                            yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                        };
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
                                    if item_type == "web_search_call" {
                                        if let Some(index) = current_text_index.take() {
                                            if open_indices.remove(&index) {
                                                let stop_event = json!({"type": "content_block_stop", "index": index});
                                                yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                            }
                                        }
                                        let Some(index) = allocate(&mut next_content_index) else {
                                            yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                        };
                                        let start_event = json!({
                                            "type": "content_block_start",
                                            "index": index,
                                            "content_block": {
                                                "type": "server_tool_use",
                                                "id": item.get("id").and_then(Value::as_str).unwrap_or(""),
                                                "name": "web_search",
                                                "input": web_search_action_input(item)
                                            }
                                        });
                                        yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));
                                        let stop_event = json!({"type": "content_block_stop", "index": index});
                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                    } else if item_type == "function_call" {
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

                                        let call_id = item.get("call_id").and_then(|i| i.as_str()).unwrap_or("");
                                        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                        let index = if let Some(k) = tool_item_key_from_added(&data, item) {
                                            if let Some(existing) = index_by_key.get(&k).copied() {
                                                existing
                                            } else {
                                                let Some(assigned) = allocate(&mut next_content_index) else {
                                                    yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                                };
                                                index_by_key.insert(k, assigned);
                                                assigned
                                            }
                                        } else {
                                            let Some(assigned) = allocate(&mut next_content_index) else {
                                                yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                            };
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
                                    has_tool_use = true;
                                    let item_id = data.get("item_id").and_then(|v| v.as_str());
                                    let existing = if let Some(id) = item_id {
                                        tool_index_by_item_id.get(id).copied()
                                    } else {
                                        None
                                    }
                                    .or_else(|| tool_item_key_from_event(&data).and_then(|k| index_by_key.get(&k).copied()))
                                    .or(last_tool_index);
                                    let index = if let Some(existing) = existing { existing } else {
                                        let Some(assigned) = allocate(&mut next_content_index) else {
                                            yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                        };
                                        assigned
                                    };

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
                            "response.output_text.annotation.added" => {
                                let annotation = data.get("annotation").unwrap_or(&data);
                                if annotation.get("type").and_then(Value::as_str) == Some("url_citation") {
                                    if let Some(index) = current_text_index {
                                        if let Some(url) = annotation.get("url").and_then(Value::as_str) {
                                            let event = json!({
                                                "type": "content_block_delta",
                                                "index": index,
                                                "delta": {
                                                    "type": "citations_delta",
                                                    "citation": {
                                                        "type": "web_search_result_location",
                                                        "url": url,
                                                        "title": annotation.get("title").and_then(Value::as_str).unwrap_or(url),
                                                        "encrypted_index": "",
                                                        "cited_text": ""
                                                    }
                                                }
                                            });
                                            yield Ok(Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
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
                                let usage_json = has_usage.then(|| usage_state.anthropic(InputTokenBasis::IncludesCache));
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
                                return;
                            }
                            "response.output_item.done" => {
                                if let Some(item) = data.get("item") {
                                    if item.get("type").and_then(Value::as_str) == Some("web_search_call") {
                                        let Some(index) = allocate(&mut next_content_index) else {
                                            yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                        };
                                        let start_event = json!({
                                            "type": "content_block_start",
                                            "index": index,
                                            "content_block": {
                                                "type": "web_search_tool_result",
                                                "tool_use_id": item.get("id").and_then(Value::as_str).unwrap_or(""),
                                                "content": web_search_result_content(item)
                                            }
                                        });
                                        yield Ok(Bytes::from(format!("event: content_block_start\ndata: {}\n\n", serde_json::to_string(&start_event).unwrap_or_default())));
                                        let stop_event = json!({"type": "content_block_stop", "index": index});
                                        yield Ok(Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", serde_json::to_string(&stop_event).unwrap_or_default())));
                                    }
                                }
                            }
                            "response.in_progress" | "response.content_part.done" => {}
                            _ => {}
                        }
                    }
                }
                Err(error) => {
                    yield Ok(super::stream_errors::decoder_error_event(&error));
                    return;
                }
            }
        }
        yield Ok(super::stream_errors::interrupted_event());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn streams_hosted_search_blocks_results_and_citations() {
        let upstream = [
            "event: response.created\ndata: {\"response\":{\"id\":\"resp_1\",\"model\":\"gpt-5\"}}\n\n",
            "event: response.output_item.added\ndata: {\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"action\":{\"query\":\"CCHub\"}}}\n\n",
            "event: response.output_item.done\ndata: {\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_1\",\"status\":\"completed\",\"action\":{\"sources\":[{\"url\":\"https://example.com\",\"title\":\"Example\"}]}}}\n\n",
            "event: response.content_part.added\ndata: {\"part\":{\"type\":\"output_text\"},\"item_id\":\"msg_1\",\"content_index\":0}\n\n",
            "event: response.output_text.annotation.added\ndata: {\"annotation\":{\"type\":\"url_citation\",\"url\":\"https://example.com\",\"title\":\"Example\"}}\n\n",
            "event: response.completed\ndata: {\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":8,\"output_tokens\":2}}}\n\n",
        ];
        let source = futures_util::stream::iter(
            upstream
                .into_iter()
                .map(|event| Ok::<Bytes, std::io::Error>(Bytes::from(event))),
        );
        let chunks = create_anthropic_sse_stream_from_responses(source)
            .collect::<Vec<_>>()
            .await;
        let output = chunks
            .into_iter()
            .map(|chunk| String::from_utf8_lossy(&chunk.expect("chunk")).into_owned())
            .collect::<String>();

        assert!(output.contains("server_tool_use"));
        assert!(output.contains("web_search_tool_result"));
        assert!(output.contains("web_search_result_location"));
        assert!(output.contains("https://example.com"));
    }
}
