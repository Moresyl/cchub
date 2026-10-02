use axum::http::{HeaderName, HeaderValue};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Turn {
    pub marker: Option<String>,
    pub within: bool,
}

pub(super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn id(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control))
        .then_some(value)
}

fn session(headers: &[(HeaderName, HeaderValue)], body: &Value) -> Option<String> {
    for name in [
        "x-grok-conv-id",
        "x-claude-code-session-id",
        "claude-code-session-id",
        "session_id",
        "x-session-id",
        "x-grok-session-id",
    ] {
        let values = headers
            .iter()
            .filter(|(key, _)| key.as_str() == name)
            .collect::<Vec<_>>();
        if values.is_empty() {
            continue;
        }
        if values.len() != 1 {
            return None;
        }
        return id(values[0].1.to_str().ok()?).map(str::to_string);
    }
    if let Some(value) = body.pointer("/metadata/session_id") {
        return id(value.as_str()?).map(str::to_string);
    }
    let user = body.pointer("/metadata/user_id")?.as_str()?;
    if user.len() > 2048 {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(user) {
        return id(value.get("session_id")?.as_str()?).map(str::to_string);
    }
    id(user.strip_prefix("user_")?.rsplit_once("_session_")?.1).map(str::to_string)
}

pub(super) fn request_key(
    tool: &str,
    path: &str,
    policy: &str,
    group: Option<&str>,
    headers: &[(HeaderName, HeaderValue)],
    body: &Value,
) -> Option<String> {
    let session = session(headers, body)?;
    // Length prefixes prevent delimiter collisions; retain only the digest,
    // never client credentials, conversation IDs or prompt text in runtime.
    let mut digest = Sha256::new();
    let mut add = |value: &[u8]| {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value);
    };
    for value in [tool, path, policy, group.unwrap_or(""), &session] {
        add(value.as_bytes());
    }
    for name in ["authorization", "x-api-key"] {
        add(name.as_bytes());
        for (_, value) in headers.iter().filter(|(key, _)| key.as_str() == name) {
            add(value.as_bytes());
        }
    }
    for field in [
        "model",
        "thinking",
        "output_config",
        "reasoning",
        "reasoning_effort",
    ] {
        add(serde_json::to_string(&body.get(field)).ok()?.as_bytes());
    }
    Some(format!("{:x}", digest.finalize()))
}

pub(super) fn turn(body: &Value) -> Turn {
    let mut result = Turn::default();
    let mut count = 0u64;
    let messages = body
        .get("messages")
        .or_else(|| body.get("contents"))
        .or_else(|| body.get("input"));
    if let Some(text) = messages.and_then(Value::as_str) {
        result.marker = Some(hash(text.as_bytes()));
    }
    if let Some(messages) = messages.and_then(Value::as_array) {
        for message in messages {
            let role = message.get("role").and_then(Value::as_str);
            let blocks = message.get("content").or_else(|| message.get("parts"));
            let kind = message.get("type").and_then(Value::as_str);
            let output = matches!(
                kind,
                Some("function_call_output" | "custom_tool_call_output" | "tool_search_output")
            );
            // Partial native histories can refer to calls in a previous response.
            // A meaningful call_id links a result; an unlinked function/custom
            // output is independent input, not a continuation of the old turn.
            let linked = output
                && message
                    .get("call_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| !id.trim().is_empty() && !id.chars().any(char::is_control));
            let standalone = !linked
                && matches!(
                    kind,
                    Some("function_call_output" | "custom_tool_call_output")
                );
            let tools = role == Some("tool")
                || linked
                || blocks.and_then(Value::as_array).is_some_and(|blocks| {
                    blocks.iter().any(|block| {
                        block.get("type").and_then(Value::as_str) == Some("tool_result")
                            || block.get("functionResponse").is_some()
                    })
                });
            if tools {
                result.within = true;
            } else if role == Some("user") || standalone {
                count += 1;
                result.within = false;
                result.marker = Some(hash(
                    serde_json::to_string(&(
                        count,
                        if standalone { Some(message) } else { blocks },
                    ))
                    .unwrap_or_default()
                    .as_bytes(),
                ));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests;
