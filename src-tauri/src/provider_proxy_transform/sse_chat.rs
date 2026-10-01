use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};

use super::stream_limits::{allocate, BLOCK_LIMIT, MAX_ID_BYTES, MAX_PENDING_ARGS};

#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    #[serde(default)]
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
        let stream = super::stream_frames::frames(stream);
        let mut buffer = String::new();
        let mut message_id = None;
        let mut current_model = None;
        let mut next_content_index: u32 = 0;
        let mut has_sent_message_start = false;
        let mut has_finish_reason = false;
        let mut current_non_tool_block_type: Option<&'static str> = None;
        let mut current_non_tool_block_index: Option<u32> = None;
        let mut tool_blocks_by_index: HashMap<usize, ToolBlockState> = HashMap::new();
        let mut open_tool_block_indices: HashSet<u32> = HashSet::new();
        let mut pending_args_bytes: usize = 0;

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

                        if let Some(data) = super::stream_frames::event_data(&line) {
                            let data = data.as_str();
                                if data.trim() == "[DONE]" {
                                    let event = json!({"type": "message_stop"});
                                    yield Ok(Bytes::from(format!("event: message_stop\ndata: {}\n\n", serde_json::to_string(&event).unwrap_or_default())));
                                    return;
                                }

                                if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(error) = super::stream_errors::error_event(&value, None) {
                                        yield Ok(error);
                                        return;
                                    }
                                }
                                if let Ok(chunk) = serde_json::from_str::<OpenAIStreamChunk>(data) {
                                    if message_id.is_none() {
                                        message_id = Some(super::anthropic_message_id(Some(&chunk.id)));
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
                                                let Some(index) = allocate(&mut next_content_index) else {
                                                    yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                                };
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
                                                    let Some(index) = allocate(&mut next_content_index) else {
                                                        yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                                    };
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
                                                if tool_call.id.as_ref().is_some_and(|id| id.len() > MAX_ID_BYTES)
                                                    || tool_call.function.as_ref().and_then(|function| function.name.as_ref()).is_some_and(|name| name.len() > MAX_ID_BYTES) {
                                                    yield Ok(super::stream_errors::api_error_event("Upstream tool identity exceeded the byte limit")); return;
                                                }
                                                if !tool_blocks_by_index.contains_key(&tool_call.index) {
                                                    let Some(index) = allocate(&mut next_content_index) else {
                                                        yield Ok(super::stream_errors::api_error_event(BLOCK_LIMIT)); return;
                                                    };
                                                    tool_blocks_by_index.insert(tool_call.index, ToolBlockState {
                                                        anthropic_index: index, id: String::new(), name: String::new(), started: false, pending_args: String::new(),
                                                    });
                                                }
                                                let (anthropic_index, id, name, should_start, pending_after_start, immediate_delta) = {
                                                    let state = tool_blocks_by_index.get_mut(&tool_call.index).expect("registered tool");

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
                                                        pending_args_bytes -= state.pending_args.len();
                                                        Some(std::mem::take(&mut state.pending_args))
                                                    } else {
                                                        None
                                                    };
                                                    let args_delta = tool_call.function.as_ref().and_then(|f| f.arguments.clone());
                                                    let immediate_delta = if let Some(args) = args_delta {
                                                        if state.started {
                                                            Some(args)
                                                        } else {
                                                            if pending_args_bytes.saturating_add(args.len()) > MAX_PENDING_ARGS {
                                                                yield Ok(super::stream_errors::api_error_event("Upstream pending tool arguments exceeded the 8 MiB limit")); return;
                                                            }
                                                            pending_args_bytes += args.len();
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
                                            has_finish_reason = !finish_reason.is_empty();
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
                Err(error) => {
                    yield Ok(super::stream_errors::decoder_error_event(&error));
                    return;
                }
            }
        }
        if has_finish_reason {
            yield Ok(Bytes::from_static(b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"));
        } else {
            yield Ok(super::stream_errors::interrupted_event());
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
