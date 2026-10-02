use axum::http::StatusCode;
use serde_json::Value;

#[derive(Debug, PartialEq)]
pub(super) enum Decision {
    Lead,
    Commit,
    Failed(StatusCode),
}

fn empty(value: &Value) -> bool {
    value.is_null()
        || value.as_str() == Some("")
        || value.as_array().is_some_and(Vec::is_empty)
        || value.as_object().is_some_and(serde_json::Map::is_empty)
}

fn status(value: &Value) -> StatusCode {
    let error = value
        .pointer("/response/error")
        .filter(|v| !v.is_null())
        .or_else(|| value.get("error"))
        .unwrap_or(value);
    for field in ["code", "status_code", "status"] {
        let code = error
            .get(field)
            .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()));
        if let Some(status) = code
            .filter(|code| (400..600).contains(code))
            .and_then(|code| StatusCode::from_u16(code as u16).ok())
        {
            return status;
        }
    }
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let kind = error
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let name = error
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    for hint in [code, kind, name] {
        let status = match hint {
            "rate_limit_error"
            | "rate_limit_exceeded"
            | "insufficient_quota"
            | "RESOURCE_EXHAUSTED" => 429,
            "overloaded_error" | "UNAVAILABLE" => 503,
            "authentication_error" | "invalid_api_key" | "UNAUTHENTICATED" => 401,
            "permission_error" | "permission_denied" | "bio_policy" | "PERMISSION_DENIED" => 403,
            "invalid_request_error" | "context_length_exceeded" | "INVALID_ARGUMENT" => 400,
            "not_found_error" | "model_not_found" | "NOT_FOUND" => 404,
            "server_error" | "api_error" | "INTERNAL" => 502,
            _ => continue,
        };
        return StatusCode::from_u16(status).unwrap();
    }
    StatusCode::BAD_GATEWAY
}

fn lead(value: &Value, kind: &str) -> bool {
    match kind {
        "ping" => true,
        "message_start" => value.pointer("/message/content").is_none_or(empty),
        "response.created" | "response.in_progress" | "response.queued" => {
            value.pointer("/response/output").is_none_or(empty)
        }
        "content_block_start" => value.get("content_block").is_some_and(|block| {
            block.get("type").and_then(Value::as_str) == Some("text")
                && block.get("text").is_some_and(empty)
                && block.as_object().is_some_and(|map| {
                    map.keys()
                        .all(|key| matches!(key.as_str(), "type" | "text"))
                })
        }),
        "content_block_delta" => value.get("delta").is_some_and(|delta| {
            delta.get("type").and_then(Value::as_str) == Some("text_delta")
                && delta.get("text").is_some_and(empty)
                && delta.as_object().is_some_and(|map| {
                    map.keys()
                        .all(|key| matches!(key.as_str(), "type" | "text"))
                })
        }),
        "response.output_text.delta" => value.get("delta").is_some_and(empty),
        "" => {
            if let Some(choices) = value.get("choices").and_then(Value::as_array) {
                return choices.iter().all(|choice| {
                    choice.get("finish_reason").is_none_or(empty)
                        && choice
                            .get("delta")
                            .and_then(Value::as_object)
                            .is_some_and(|delta| {
                                delta.iter().all(|(key, value)| {
                                    key == "role"
                                        || (matches!(
                                            key.as_str(),
                                            "content"
                                                | "reasoning_content"
                                                | "reasoning"
                                                | "tool_calls"
                                                | "function_call"
                                                | "refusal"
                                        ) && empty(value))
                                })
                            })
                        && choice.get("message").is_none_or(empty)
                });
            }
            if let Some(candidates) = value.get("candidates").and_then(Value::as_array) {
                return value
                    .pointer("/promptFeedback/blockReason")
                    .is_none_or(empty)
                    && candidates.iter().all(|candidate| {
                        candidate.get("finishReason").is_none_or(empty)
                            && candidate.pointer("/content/parts").is_none_or(|parts| {
                                parts.as_array().is_some_and(|parts| {
                                    parts.iter().all(|part| {
                                        part.as_object().is_some_and(|map| {
                                            map.len() == 1 && map.get("text").is_some_and(empty)
                                        })
                                    })
                                })
                            })
                    });
            }
            // A usage-only event is metadata; unknown payloads commit immediately.
            value.as_object().is_some_and(|map| {
                !map.is_empty()
                    && map.keys().all(|key| {
                        matches!(
                            key.as_str(),
                            "usage" | "usageMetadata" | "model" | "id" | "object" | "created"
                        )
                    })
            })
        }
        _ => false,
    }
}

// Content and tool deltas win over a co-located error: replay must never duplicate
// output or an instruction that a downstream agent could already execute.
fn has_output(value: &Value) -> bool {
    value
        .get("choices")
        .and_then(Value::as_array)
        .is_some_and(|choices| {
            choices.iter().any(|choice| {
                choice
                    .get("delta")
                    .and_then(Value::as_object)
                    .is_some_and(|delta| {
                        delta
                            .iter()
                            .any(|(key, value)| key != "role" && !empty(value))
                    })
                    || choice.get("message").is_some_and(|message| !empty(message))
            })
        })
        || value
            .pointer("/response/output")
            .is_some_and(|output| !empty(output))
        || value
            .pointer("/message/content")
            .is_some_and(|content| !empty(content))
        || value.get("delta").is_some_and(|delta| match delta {
            Value::String(text) => !text.is_empty(),
            Value::Object(map) => map
                .iter()
                .any(|(key, value)| key != "type" && !empty(value)),
            _ => !empty(delta),
        })
        || value.get("content_block").is_some_and(|block| {
            block.get("type").and_then(Value::as_str) != Some("text")
                || block.get("text").is_some_and(|text| !empty(text))
        })
        || value
            .get("candidates")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.pointer("/content/parts").is_some_and(|parts| {
                        parts.as_array().map_or_else(
                            || !empty(parts),
                            |parts| {
                                parts.iter().any(|part| {
                                    // An empty text part is initialization, just
                                    // like Chat's empty role/content chunk. Keep
                                    // unknown or tool parts conservative.
                                    !part.as_object().is_some_and(|map| {
                                        map.len() == 1 && map.get("text").is_some_and(empty)
                                    })
                                })
                            },
                        )
                    })
                })
            })
}

pub(super) fn classify(frame: &str) -> Decision {
    let frame = frame.trim_start_matches('\u{feff}');
    let mut event = "";
    let mut data = Vec::new();
    for line in frame.lines() {
        if let Some(name) = line.strip_prefix("event:") {
            event = name.trim();
        }
        if let Some(value) = line.strip_prefix("data:") {
            data.push(value.strip_prefix(' ').unwrap_or(value));
        }
    }
    if data.is_empty() {
        if matches!(event, "error" | "response.failed") {
            return Decision::Failed(StatusCode::BAD_GATEWAY);
        }
        return if matches!(event, "" | "ping") {
            Decision::Lead
        } else {
            Decision::Commit
        };
    }
    let Ok(value) = serde_json::from_str::<Value>(&data.join("\n")) else {
        return Decision::Commit;
    };
    if has_output(&value) {
        return Decision::Commit;
    }
    let kind = value.get("type").and_then(Value::as_str).unwrap_or(event);
    if matches!(event, "error" | "response.failed")
        || matches!(kind, "error" | "response.failed")
        || value.get("error").is_some_and(|error| !error.is_null())
        || value
            .pointer("/response/error")
            .is_some_and(|error| !error.is_null())
        || value.pointer("/response/status").and_then(Value::as_str) == Some("failed")
    {
        return Decision::Failed(status(&value));
    }
    if lead(&value, kind) {
        Decision::Lead
    } else {
        Decision::Commit
    }
}
