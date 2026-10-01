use bytes::Bytes;
use serde_json::Value;

pub(super) fn interrupted_event() -> Bytes {
    Bytes::from_static(b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"Upstream stream was interrupted before completion\"}}\n\n")
}

pub(super) fn api_error_event(message: &str) -> Bytes {
    let error = serde_json::json!({"type":"error","error":{"type":"api_error","message":message}});
    Bytes::from(format!("event: error\ndata: {error}\n\n"))
}

pub(super) fn decoder_error_event(error: &std::io::Error) -> Bytes {
    match error
        .get_ref()
        .and_then(|error| error.downcast_ref::<super::stream_frames::FrameError>())
    {
        Some(error) => api_error_event(&error.to_string()),
        None => interrupted_event(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[tokio::test]
    async fn all_adapters_use_api_errors_without_exposing_raw_transport_details() {
        use crate::provider_proxy_transform::{
            create_anthropic_sse_stream, create_anthropic_sse_stream_from_gemini,
            create_anthropic_sse_stream_from_responses,
        };
        let source = || {
            futures_util::stream::iter([Err::<Bytes, _>(std::io::Error::other(
                "private URL and credential",
            ))])
        };
        let outputs = [
            create_anthropic_sse_stream(source())
                .collect::<Vec<_>>()
                .await,
            create_anthropic_sse_stream_from_responses(source())
                .collect::<Vec<_>>()
                .await,
            create_anthropic_sse_stream_from_gemini(source(), "fixture".into())
                .collect::<Vec<_>>()
                .await,
        ];
        for chunks in outputs {
            assert_eq!(chunks.len(), 1);
            assert_eq!(chunks[0].as_ref().unwrap(), &interrupted_event());
        }
    }
}

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
