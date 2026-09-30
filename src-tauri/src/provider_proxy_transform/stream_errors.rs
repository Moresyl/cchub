use bytes::Bytes;
use serde_json::Value;

pub(super) fn error_event(value: &Value, event: Option<&str>) -> Option<Bytes> {
    let payload = value.get("response").unwrap_or(value);
    let failed = payload.get("error").is_some_and(|error| !error.is_null())
        || matches!(event, Some("error" | "response.failed"))
        || matches!(
            value.get("type").and_then(Value::as_str),
            Some("error" | "response.failed")
        )
        || payload.get("status").and_then(Value::as_str) == Some("failed");
    if !failed {
        return None;
    }
    let error = super::openai_error_to_anthropic(502, Some(payload));
    Some(Bytes::from(format!("event: error\ndata: {error}\n\n")))
}
