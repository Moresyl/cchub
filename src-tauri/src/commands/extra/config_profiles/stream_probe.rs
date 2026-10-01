use futures_util::{Stream, StreamExt};
use serde_json::Value;
use std::time::{Duration, Instant};

use super::StreamCheckRequestSpec;
use crate::provider_proxy_transform::stream_frames::{event_data, frames};

const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) struct ProbeOutcome {
    pub status: &'static str,
    pub message: String,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
}

pub(super) async fn execute(
    client: reqwest::Client,
    request: StreamCheckRequestSpec,
) -> ProbeOutcome {
    let started = Instant::now();
    let checked = tokio::time::timeout(CHECK_TIMEOUT, async {
        let mut builder = client
            .post(&request.endpoint)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream, application/json");
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }
        let response = builder.json(&request.body).send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                return (
                    None,
                    Err(if error.is_timeout() {
                        "Model request timed out"
                    } else {
                        "Model request could not be sent"
                    }),
                )
            }
        };
        let code = response.status().as_u16();
        if !response.status().is_success() {
            // Do not copy server bodies or URLs into errors: they may echo credentials.
            return (Some(code), Err(http_failure(code)));
        }
        let sse = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("text/event-stream"))
            });
        (
            Some(code),
            verify_response(response.bytes_stream(), sse).await,
        )
    })
    .await;
    let (http_status, result) = match checked {
        Ok(result) => result,
        Err(_) => (None, Err("Model request timed out before a complete reply")),
    };
    ProbeOutcome {
        status: if result.is_ok() { "healthy" } else { "error" },
        message: result
            .map(|()| "Received a verified model reply")
            .unwrap_or_else(|message| message)
            .to_string(),
        http_status,
        latency_ms: Some(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
    }
}

fn http_failure(code: u16) -> &'static str {
    match code {
        401 => "Model authentication was rejected (HTTP 401)",
        403 => "Model access was denied (HTTP 403)",
        404 => "Model or endpoint was not found (HTTP 404)",
        429 => "Model request was rate-limited or quota was exhausted (HTTP 429)",
        500..=599 => "Model provider returned a server error (HTTP 5xx)",
        _ => "Model request returned an unsuccessful HTTP status",
    }
}

async fn verify_response<S, E>(stream: S, sse: bool) -> Result<(), &'static str>
where
    S: Stream<Item = Result<bytes::Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    let bounded = stream.scan(0usize, |total, chunk| {
        let value = match chunk {
            Ok(chunk) if chunk.len() <= MAX_RESPONSE_BYTES.saturating_sub(*total) => {
                *total += chunk.len();
                Ok(chunk)
            }
            Ok(_) => Err(std::io::Error::other(
                "Model reply exceeded the probe limit",
            )),
            Err(_) => Err(std::io::Error::other("Model reply was interrupted")),
        };
        std::future::ready(Some(value))
    });
    let mut reply = ReplyState::default();
    if sse {
        let events = frames(bounded);
        tokio::pin!(events);
        while let Some(frame) = events.next().await {
            let frame = frame.map_err(|_| {
                "Model stream was interrupted, malformed or exceeded the probe limit"
            })?;
            let text =
                std::str::from_utf8(&frame).map_err(|_| "Model stream contained invalid text")?;
            if text.lines().any(|line| {
                line.strip_prefix("event:")
                    .is_some_and(|event| matches!(event.trim(), "error" | "response.failed"))
            }) {
                return Err("Model stream reported an error");
            }
            let Some(data) = event_data(text) else {
                continue;
            };
            if data.trim() == "[DONE]" {
                return reply
                    .active
                    .then_some(())
                    .ok_or("Model stream ended without a model reply");
            }
            let value: Value =
                serde_json::from_str(&data).map_err(|_| "Model stream contained invalid JSON")?;
            if reply.observe(&value)? {
                return Ok(());
            }
        }
        Err("Model stream ended without a complete model reply")
    } else {
        tokio::pin!(bounded);
        let mut body = Vec::new();
        while let Some(chunk) = bounded.next().await {
            body.extend_from_slice(
                &chunk.map_err(|_| "Model reply was interrupted or exceeded the probe limit")?,
            );
        }
        let value: Value = serde_json::from_slice(&body)
            .map_err(|_| "Endpoint did not return a valid JSON model reply")?;
        reply
            .observe(&value)?
            .then_some(())
            .ok_or("Endpoint returned JSON without a complete model reply")
    }
}

#[derive(Default)]
struct ReplyState {
    active: bool,
}

impl ReplyState {
    fn observe(&mut self, value: &Value) -> Result<bool, &'static str> {
        let kind = value.get("type").and_then(Value::as_str);
        if value.get("error").is_some_and(|error| !error.is_null())
            || matches!(kind, Some("error" | "response.failed"))
            || value
                .pointer("/response/error")
                .is_some_and(|error| !error.is_null())
            || value.pointer("/response/status").and_then(Value::as_str) == Some("failed")
        {
            return Err("Model provider reported an error");
        }
        if kind == Some("message_start")
            && value
                .pointer("/message/id")
                .and_then(Value::as_str)
                .is_some()
        {
            self.active = true;
        }
        if kind == Some("message_stop") {
            return Ok(self.active);
        }
        if kind == Some("message")
            && value.get("id").and_then(Value::as_str).is_some()
            && value.get("content").and_then(Value::as_array).is_some()
            && value.get("stop_reason").and_then(Value::as_str).is_some()
        {
            return Ok(true);
        }
        let response = if matches!(kind, Some("response.completed" | "response.incomplete")) {
            value.get("response")
        } else if kind == Some("response") {
            Some(value)
        } else {
            None
        };
        if let Some(response) = response {
            return Ok(response.get("id").and_then(Value::as_str).is_some()
                && matches!(
                    response.get("status").and_then(Value::as_str),
                    Some("completed" | "incomplete")
                )
                && response.get("output").and_then(Value::as_array).is_some());
        }
        if let Some(choices) = value.get("choices").and_then(Value::as_array) {
            for choice in choices {
                let part = choice.get("delta").or_else(|| choice.get("message"));
                self.active |= part.is_some_and(|part| {
                    part.get("role").and_then(Value::as_str) == Some("assistant")
                        || ["content", "reasoning_content"].iter().any(|field| {
                            part.get(field)
                                .and_then(Value::as_str)
                                .is_some_and(|text| !text.is_empty())
                        })
                        || part
                            .get("tool_calls")
                            .and_then(Value::as_array)
                            .is_some_and(|calls| !calls.is_empty())
                });
                if self.active
                    && choice
                        .get("finish_reason")
                        .and_then(Value::as_str)
                        .is_some_and(|reason| !reason.is_empty())
                {
                    return Ok(true);
                }
            }
        }
        if let Some(candidates) = value.get("candidates").and_then(Value::as_array) {
            if candidates.iter().any(|item| {
                item.pointer("/content/parts")
                    .and_then(Value::as_array)
                    .is_some()
                    && matches!(
                        item.get("finishReason").and_then(Value::as_str),
                        Some("STOP" | "MAX_TOKENS")
                    )
            }) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests;
