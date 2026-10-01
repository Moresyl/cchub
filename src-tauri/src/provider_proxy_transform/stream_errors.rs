use bytes::Bytes;
use serde_json::Value;

const CONNECTION_LOST: &str = "Upstream connection lost before stream completion";

pub(super) fn interrupted_event() -> Bytes {
    api_error_event(CONNECTION_LOST)
}

/// Keep a recognizable transport failure while never echoing URLs or payloads.
/// Wrappers retain typed IO errors, so invalid data is not advertised as a retryable connection loss.
pub(crate) fn stream_failure_message(error: &std::io::Error) -> &'static str {
    let mut cause: &(dyn std::error::Error + 'static) = error;
    let mut invalid = false;
    let mut timeout = false;
    for _ in 0..16 {
        if let Some(frame) = cause.downcast_ref::<super::stream_frames::FrameError>() {
            return match frame {
                super::stream_frames::FrameError::Limit => {
                    "Upstream SSE event exceeded the 8 MiB limit"
                }
                super::stream_frames::FrameError::Utf8 => {
                    "Upstream SSE event contains invalid UTF-8"
                }
                super::stream_frames::FrameError::Incomplete => CONNECTION_LOST,
            };
        }
        let next = if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            invalid |= io.kind() == std::io::ErrorKind::InvalidData;
            timeout |= io.kind() == std::io::ErrorKind::TimedOut;
            io.get_ref()
                .map(|next| next as &(dyn std::error::Error + 'static))
        } else {
            cause.source()
        };
        let Some(next) = next else { break };
        cause = next;
    }
    if invalid {
        "Upstream stream contains invalid data"
    } else if timeout {
        "Upstream connection timed out before stream completion"
    } else {
        CONNECTION_LOST
    }
}

pub(super) fn api_error_event(message: &str) -> Bytes {
    let error = serde_json::json!({"type":"error","error":{"type":"api_error","message":message}});
    Bytes::from(format!("event: error\ndata: {error}\n\n"))
}

pub(super) fn decoder_error_event(error: &std::io::Error) -> Bytes {
    api_error_event(stream_failure_message(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[test]
    fn nested_errors_preserve_transport_timeout_and_data_categories_without_secrets() {
        for (kind, expected) in [
            (std::io::ErrorKind::ConnectionReset, "connection lost"),
            (std::io::ErrorKind::TimedOut, "connection timed out"),
            (std::io::ErrorKind::InvalidData, "invalid data"),
        ] {
            let mut error = std::io::Error::new(kind, "private credential");
            for _ in 0..5 {
                error = std::io::Error::other(error);
            }
            let message = stream_failure_message(&error);
            assert!(message.contains(expected));
            assert!(!message.contains("private"));
            if kind == std::io::ErrorKind::InvalidData {
                assert!(!message.contains("connection"));
            }
        }
        let error = std::io::Error::other(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            super::super::stream_frames::FrameError::Limit,
        ));
        assert!(stream_failure_message(&error).contains("8 MiB limit"));
        assert!(!stream_failure_message(&error).contains("connection"));
    }

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
            assert!(std::str::from_utf8(chunks[0].as_ref().unwrap())
                .unwrap()
                .contains("connection lost"));
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
