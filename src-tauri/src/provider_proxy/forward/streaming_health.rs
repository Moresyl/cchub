use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::Value;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

const FAILED: u8 = 1;
const COMPLETED: u8 = 2;
const REQUIRES_COMPLETION: u8 = 4;
const UNVERIFIED: u8 = 8;

#[derive(Clone, Default)]
pub(crate) struct StreamHealth(Arc<AtomicU8>);

impl StreamHealth {
    pub(crate) fn failed(&self) -> bool {
        self.0.load(Ordering::Relaxed) & FAILED != 0
    }
    fn fail(&self) {
        self.0.fetch_or(FAILED, Ordering::Relaxed);
    }
    pub(super) fn requiring_completion() -> Self {
        Self(Arc::new(AtomicU8::new(REQUIRES_COMPLETION)))
    }
    pub(crate) fn verified(&self) -> bool {
        self.0.load(Ordering::Relaxed) & UNVERIFIED == 0
    }
    fn completed(&self) {
        self.0.fetch_or(COMPLETED, Ordering::Relaxed);
    }
    fn unknown_frame(&self) {
        self.0.fetch_or(UNVERIFIED, Ordering::Relaxed);
    }
    fn incomplete(&self) -> bool {
        let flags = self.0.load(Ordering::Relaxed);
        flags & REQUIRES_COMPLETION != 0 && flags & (FAILED | COMPLETED | UNVERIFIED) == 0
    }
}

fn is_error(value: &Value) -> bool {
    value.get("error").is_some_and(|error| !error.is_null())
        || matches!(
            value.get("type").and_then(Value::as_str),
            Some("error" | "response.failed")
        )
        || value
            .pointer("/response/error")
            .is_some_and(|error| !error.is_null())
        || value.pointer("/response/status").and_then(Value::as_str) == Some("failed")
}

// Inspect original events before adapters can translate an error into an Ok SSE chunk.
// Retain at most one MiB of an incomplete frame; bytes pass through unchanged.
pub(super) fn observe<S, E>(
    stream: S,
    health: StreamHealth,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    async_stream::stream! {
        let mut buffer = Vec::new();
        let mut skipping_frame = false;
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match &chunk {
                Err(_) => health.fail(),
                Ok(bytes) => {
                    for byte in bytes {
                        buffer.push(*byte);
                        if buffer.ends_with(b"\n\n") || buffer.ends_with(b"\r\n\r\n") {
                            if !skipping_frame { inspect_frame(&buffer, &health); }
                            buffer.clear();
                            skipping_frame = false;
                        } else if buffer.len() > 1024 * 1024 {
                            health.unknown_frame();
                            buffer.drain(..buffer.len() - 3);
                            skipping_frame = true;
                        }
                    }
                }
            }
            yield chunk.map_err(|error| std::io::Error::other(error.to_string()));
        }
        if !skipping_frame { inspect_frame(&buffer, &health); }
        if health.incomplete() {
            health.fail();
            yield Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Upstream stream ended before completion"));
        }
    }
}

fn inspect_frame(frame: &[u8], health: &StreamHealth) {
    let Ok(text) = std::str::from_utf8(frame) else {
        return;
    };
    let mut data = Vec::new();
    let mut event = None;
    for line in text.lines() {
        if let Some(kind) = line.strip_prefix("event:") {
            event = Some(kind.trim());
            if matches!(kind.trim(), "error" | "response.failed") {
                health.fail();
            }
        }
        if let Some(value) = line.strip_prefix("data:") {
            data.push(value.strip_prefix(' ').unwrap_or(value));
        }
    }
    let payload = if data.is_empty() {
        text.to_string()
    } else {
        data.join("\n")
    };
    if payload.trim() == "[DONE]" {
        health.completed();
    }
    if let Ok(value) = serde_json::from_str::<Value>(&payload) {
        if is_error(&value) {
            health.fail();
        }
        let terminal = matches!(
            event,
            Some("message_stop" | "response.completed" | "response.incomplete")
        ) || matches!(
            value.get("type").and_then(Value::as_str),
            Some("message_stop" | "response.completed" | "response.incomplete")
        ) || value
            .get("choices")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("finish_reason")
                        .and_then(Value::as_str)
                        .is_some_and(|reason| !reason.is_empty())
                })
            })
            || value
                .get("candidates")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item.get("finishReason")
                            .and_then(Value::as_str)
                            .is_some_and(|reason| !reason.is_empty())
                    })
                });
        if terminal {
            health.completed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn terminal_event_names_are_recognized_but_empty_finish_reasons_are_not() {
        for (frame, failed) in [
            ("event: response.completed\ndata: {\"response\":{\"status\":\"completed\"}}\n\n", false),
            ("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n", false),
            ("data: {\"choices\":[{\"finish_reason\":\"\"}]}\n\n", true),
            ("event: response.completed\ndata: {\"response\":{\"error\":{\"message\":\"failed\"}}}\n\n", true),
        ] {
            let health = StreamHealth::requiring_completion();
            let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(frame.as_bytes()))]);
            let _ = observe(stream, health.clone()).collect::<Vec<_>>().await;
            assert_eq!(health.failed(), failed, "{frame}");
        }
    }

    #[tokio::test]
    async fn split_crlf_unicode_and_multiline_error_is_detected_without_changing_bytes() {
        let frame = "event: error\r\ndata: {\"type\":\"error\",\r\ndata: \"error\":{\"message\":\"限流\"}}\r\n\r\n";
        let health = StreamHealth::default();
        let chunks = frame
            .as_bytes()
            .iter()
            .map(|byte| Ok::<_, std::io::Error>(Bytes::copy_from_slice(&[*byte])))
            .collect::<Vec<_>>();
        let output = observe(futures_util::stream::iter(chunks), health.clone())
            .collect::<Vec<_>>()
            .await;
        assert!(health.failed());
        let output: Vec<u8> = output
            .into_iter()
            .flat_map(|bytes| bytes.unwrap().to_vec())
            .collect();
        assert_eq!(output, frame.as_bytes());
    }

    #[tokio::test]
    async fn content_mentioning_errors_and_incomplete_token_budget_are_not_failures() {
        let frame = "data: {\"delta\":{\"text\":\"event: error\"}}\n\nevent: response.completed\ndata: {\"response\":{\"status\":\"incomplete\",\"error\":null}}\n\n";
        let health = StreamHealth::default();
        let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(
            frame.as_bytes(),
        ))]);
        let output = observe(stream, health.clone()).collect::<Vec<_>>().await;
        assert_eq!(output.len(), 1);
        assert!(!health.failed());
    }

    #[tokio::test]
    async fn oversized_frame_does_not_hide_a_following_error() {
        let mut frame = vec![b'x'; 1024 * 1024 + 20];
        frame.extend_from_slice(b"\n\nevent: error\ndata: {}\n\n");
        let health = StreamHealth::default();
        let stream =
            futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from(frame.clone()))]);
        let output = observe(stream, health.clone()).collect::<Vec<_>>().await;
        assert!(health.failed());
        assert_eq!(output[0].as_ref().unwrap().as_ref(), frame);
    }
}
