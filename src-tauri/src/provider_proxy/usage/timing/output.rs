use serde_json::Value;

fn text(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

fn call(value: &Value) -> bool {
    text(value.get("name")) || text(value.get("arguments"))
}

fn block(value: &Value) -> bool {
    match value.get("type").and_then(Value::as_str) {
        Some("text" | "output_text") => text(value.get("text")),
        Some("thinking") => text(value.get("thinking")),
        Some("redacted_thinking") => text(value.get("data")),
        Some("tool_use" | "function_call") => call(value),
        Some("refusal") => text(value.get("refusal")),
        _ => false,
    }
}

// Recognize payload content, not lifecycle, role, usage, finish or keepalive
// metadata. Unknown protocols remain unmeasured rather than inventing TTFT.
pub(super) fn has_output(event: &str, value: &Value) -> bool {
    let kind = if event.is_empty() {
        value.get("type").and_then(Value::as_str).unwrap_or("")
    } else {
        event
    };
    match kind {
        "content_block_delta" => {
            let Some(delta) = value.get("delta") else {
                return false;
            };
            match delta.get("type").and_then(Value::as_str) {
                Some("text_delta") => text(delta.get("text")),
                Some("thinking_delta") => text(delta.get("thinking")),
                Some("input_json_delta") => text(delta.get("partial_json")),
                _ => false,
            }
        }
        "content_block_start" => value.get("content_block").is_some_and(block),
        "message_start" => value
            .pointer("/message/content")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(block)),
        "response.output_text.delta"
        | "response.refusal.delta"
        | "response.function_call_arguments.delta"
        | "response.reasoning.delta"
        | "response.reasoning_text.delta"
        | "response.reasoning_summary_text.delta"
        | "response.audio.delta"
        | "response.output_audio.delta"
        | "response.audio_transcript.delta"
        | "response.output_audio_transcript.delta" => text(value.get("delta")),
        "response.output_item.added" => value.get("item").is_some_and(block),
        "response.content_part.added" => value.get("part").is_some_and(block),
        "" => {
            let chat = value
                .get("choices")
                .and_then(Value::as_array)
                .is_some_and(|choices| {
                    choices.iter().any(|choice| {
                        let Some(delta) = choice.get("delta") else {
                            return false;
                        };
                        ["content", "reasoning_content", "reasoning", "refusal"]
                            .iter()
                            .any(|field| text(delta.get(field)))
                            || delta.get("function_call").is_some_and(call)
                            || delta
                                .get("tool_calls")
                                .and_then(Value::as_array)
                                .is_some_and(|calls| {
                                    calls
                                        .iter()
                                        .any(|item| item.get("function").is_some_and(call))
                                })
                            || delta.get("audio").is_some_and(|audio| {
                                text(audio.get("data")) || text(audio.get("transcript"))
                            })
                    })
                });
            chat || value
                .get("candidates")
                .and_then(Value::as_array)
                .is_some_and(|candidates| {
                    candidates.iter().any(|candidate| {
                        candidate
                            .pointer("/content/parts")
                            .and_then(Value::as_array)
                            .is_some_and(|parts| {
                                parts.iter().any(|part| {
                                    text(part.get("text"))
                                        || part.get("functionCall").is_some_and(call)
                                        || part
                                            .get("inlineData")
                                            .is_some_and(|data| text(data.get("data")))
                                })
                            })
                    })
                })
        }
        _ => false,
    }
}
