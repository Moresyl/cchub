use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde_json::json;

#[derive(Clone, Copy)]
pub(super) enum Protocol {
    Messages,
    Responses,
    Chat,
    Gemini,
}

impl Protocol {
    pub(super) fn for_path(path: &str) -> Option<Self> {
        let path = path.trim_matches('/');
        if path == "messages" || path.ends_with("/messages") {
            Some(Self::Messages)
        } else if path == "responses" || path.ends_with("/responses") {
            Some(Self::Responses)
        } else if path.ends_with("chat/completions") {
            Some(Self::Chat)
        } else if path.contains(":streamGenerateContent") {
            Some(Self::Gemini)
        } else {
            None
        }
    }
}

fn failure_event(protocol: Protocol, provider: &str) -> Bytes {
    let message = format!("{provider}: upstream stream was interrupted before completion");
    let (name, value) = match protocol {
        Protocol::Messages => (
            Some("error"),
            json!({"type":"error","error":{"type":"api_error","message":message}}),
        ),
        Protocol::Responses => (
            Some("response.failed"),
            json!({"type":"response.failed","response":{"object":"response","status":"failed","error":{"code":"server_error","message":message}}}),
        ),
        Protocol::Chat => (
            None,
            json!({"error":{"type":"api_error","message":message}}),
        ),
        Protocol::Gemini => (
            None,
            json!({"error":{"code":502,"status":"UNAVAILABLE","message":message}}),
        ),
    };
    let event = name
        .map(|name| format!("event: {name}\n"))
        .unwrap_or_default();
    // End a partial upstream frame before appending a standalone failure event.
    Bytes::from(format!("\n\n{event}data: {value}\n\n"))
}

pub(super) fn terminate<S>(
    stream: S,
    protocol: Protocol,
    provider: String,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
{
    async_stream::stream! {
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => yield Ok(bytes),
                Err(_) => {
                    yield Ok(failure_event(protocol, &provider));
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "streaming_errors/tests.rs"]
mod tests;
