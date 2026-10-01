use axum::http::StatusCode;
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use std::time::Duration;

use super::timeouts::ResponseStream;
use crate::provider_proxy::{ClaudeApiFormat, ProxyUsageMetrics};
use crate::shared::gemini_usage::GeminiUsage;
use crate::shared::token_usage::InputTokenBasis;

#[path = "streaming_preflight/event.rs"]
mod event;
use event::{classify, Decision};

const MAX_PREFIX_BYTES: usize = 256 * 1024;
const MAX_PREFIX_WAIT: Duration = Duration::from_secs(1);

pub(super) struct Failure {
    pub status: StatusCode,
    pub message: String,
    pub usage: Option<ProxyUsageMetrics>,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message,
            usage: None,
        }
    }
}

impl Failure {
    pub(super) fn retryable(&self) -> bool {
        crate::provider_proxy::is_retryable_upstream_status(self.status)
    }

    pub(super) fn response(&self, path: &str) -> axum::http::Response<axum::body::Body> {
        let error = serde_json::json!({"code": self.status.as_u16(), "type": if self.status == StatusCode::TOO_MANY_REQUESTS { "rate_limit_error" } else { "api_error" }, "message": self.message});
        let value = if matches!(
            super::streaming_errors::Protocol::for_path(path),
            Some(super::streaming_errors::Protocol::Messages)
        ) {
            serde_json::json!({"type":"error", "error":error})
        } else {
            serde_json::json!({"error":error})
        };
        crate::provider_proxy::build_json_response_from_value(
            self.status,
            &Default::default(),
            &value,
        )
    }
}

#[derive(Default)]
struct Inspector {
    frame: Vec<u8>,
    previous_cr: bool,
    line_length: usize,
    usage: ProxyUsageMetrics,
    gemini: GeminiUsage,
}

impl Inspector {
    fn push(&mut self, byte: u8, basis: InputTokenBasis) -> Decision {
        if self.previous_cr {
            self.previous_cr = false;
            if byte == b'\n' {
                return Decision::Lead;
            }
        }
        let byte = if byte == b'\r' {
            self.previous_cr = true;
            b'\n'
        } else {
            byte
        };
        self.frame.push(byte);
        if byte != b'\n' {
            self.line_length += 1;
            return Decision::Lead;
        }
        let empty = self.line_length == 0;
        self.line_length = 0;
        if !empty {
            return Decision::Lead;
        }
        let decision = match std::str::from_utf8(&self.frame) {
            Ok(frame) => {
                crate::provider_proxy::usage::scan_stream_usage_buffer(
                    &mut String::new(),
                    frame.trim_start_matches('\u{feff}'),
                    &mut self.usage,
                    &mut self.gemini,
                    basis,
                );
                classify(frame)
            }
            Err(_) => Decision::Commit,
        };
        self.frame.clear();
        decision
    }
}

fn replay(prefix: Bytes, current: Option<Bytes>, stream: ResponseStream) -> ResponseStream {
    let chunks = [Some(prefix).filter(|bytes| !bytes.is_empty()), current];
    Box::pin(futures_util::stream::iter(chunks.into_iter().flatten().map(Ok)).chain(stream))
}

// Only known initialization events are held. Any output, side-effect, unknown
// event, byte cap or grace deadline commits the exact prefix and disables replay.
pub(super) async fn prepare(
    mut stream: ResponseStream,
    path: &str,
    transform: Option<ClaudeApiFormat>,
) -> Result<ResponseStream, Failure> {
    let mut prefix = BytesMut::new();
    let mut inspector = Inspector::default();
    let basis = crate::provider_proxy::usage::source_input_basis(path, transform);
    let deadline = tokio::time::Instant::now() + MAX_PREFIX_WAIT;
    loop {
        let next = tokio::select! {
            biased;
            _ = tokio::time::sleep_until(deadline) => return Ok(replay(prefix.freeze(), None, stream)),
            next = stream.next() => next,
        };
        let bytes = match next {
            Some(Ok(bytes)) => bytes,
            Some(Err(error)) if !prefix.is_empty() => {
                return Err(Failure {
                    status: StatusCode::BAD_GATEWAY,
                    message: crate::provider_proxy_transform::stream_failure_message(&error)
                        .to_string(),
                    usage: Some(inspector.usage),
                });
            }
            Some(Err(error)) => return Err(error.to_string().into()),
            None => return Ok(replay(prefix.freeze(), None, stream)),
        };
        let remaining = MAX_PREFIX_BYTES - prefix.len();
        for byte in bytes.iter().take(remaining) {
            match inspector.push(*byte, basis) {
                Decision::Lead => {}
                Decision::Commit => return Ok(replay(prefix.freeze(), Some(bytes), stream)),
                Decision::Failed(status) => {
                    return Err(Failure {
                        status,
                        message: format!(
                            "Upstream stream failed before output (HTTP {})",
                            status.as_u16()
                        ),
                        usage: Some(inspector.usage),
                    })
                }
            }
        }
        if bytes.len() >= remaining {
            return Ok(replay(prefix.freeze(), Some(bytes), stream));
        }
        prefix.extend_from_slice(&bytes);
    }
}

#[cfg(test)]
#[path = "streaming_preflight/tests.rs"]
mod tests;
