use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use serde_json::{json, Value};

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
        let output =
            create_anthropic_sse_stream_from_gemini(input_stream, "gemini-2.5-flash".to_string());
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
        let output =
            create_anthropic_sse_stream_from_gemini(input_stream, "gemini-2.5-flash".to_string());
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
        let output =
            create_anthropic_sse_stream_from_gemini(input_stream, "gemini-2.5-pro".to_string());
        tokio::pin!(output);

        let mut events: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = output.next().await {
            events.push(String::from_utf8_lossy(&bytes).to_string());
        }

        let all = events.join("");
        assert!(all.contains("\"stop_reason\":\"max_tokens\""));
    }
}
