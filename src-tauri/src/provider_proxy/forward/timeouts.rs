use bytes::{Bytes, BytesMut};
use futures_util::{Stream, StreamExt};
use std::{future::Future, pin::Pin, time::Duration};
use tokio::time::Instant;

use crate::proxy_optimizer::OptimizerConfig;

#[derive(Clone, Copy)]
pub(super) struct Deadline(Option<Instant>);

impl Deadline {
    fn after_seconds(seconds: u64) -> Self {
        // Config validation bounds every duration before an attempt acquires a lease.
        Self((seconds > 0).then(|| Instant::now() + Duration::from_secs(seconds)))
    }

    fn sooner(self, other: Self) -> Self {
        Self(match (self.0, other.0) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (left, right) => left.or(right),
        })
    }

    pub(super) async fn wait<T>(
        self,
        future: impl Future<Output = T>,
        phase: &str,
    ) -> Result<T, String> {
        if let Some(deadline) = self.0 {
            tokio::time::timeout_at(deadline, future)
                .await
                .map_err(|_| format!("Upstream {phase} timeout"))
        } else {
            Ok(future.await)
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct AttemptBudget {
    pub(super) first: Deadline,
    body: Deadline,
}

impl AttemptBudget {
    pub(super) fn new(config: &OptimizerConfig, streaming: bool) -> Self {
        let body = Deadline::after_seconds(config.non_streaming_timeout);
        Self {
            first: if streaming {
                Deadline::after_seconds(config.streaming_first_byte_timeout)
            } else {
                body
            },
            body,
        }
    }
}

pub(super) async fn send(
    builder: reqwest::RequestBuilder,
    budget: AttemptBudget,
) -> Result<reqwest::Response, String> {
    budget
        .first
        .wait(builder.send(), "response headers")
        .await?
        .map_err(|error| error.without_url().to_string())
}

pub(super) async fn read_response_body_limited(
    response: reqwest::Response,
    max_bytes: usize,
    budget: AttemptBudget,
) -> Result<(reqwest::StatusCode, reqwest::header::HeaderMap, Bytes), String> {
    let status = response.status();
    let headers = response.headers().clone();
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(format!("Upstream response body exceeds {max_bytes} bytes"));
    }
    let mut body = BytesMut::new();
    let mut stream = response.bytes_stream();
    loop {
        let deadline = if body.is_empty() {
            budget.first.sooner(budget.body)
        } else {
            budget.body
        };
        let Some(chunk) = deadline.wait(stream.next(), "response body").await? else {
            break;
        };
        let chunk = chunk
            .map_err(|error| format!("Response body could not be read: {}", error.without_url()))?;
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err(format!("Upstream response body exceeds {max_bytes} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok((status, headers, body.freeze()))
}

pub(super) type ResponseStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

pub(super) async fn prepare_raw_stream(
    response: reqwest::Response,
    first_deadline: Deadline,
    idle_seconds: u64,
) -> Result<ResponseStream, String> {
    let mut stream = Box::pin(response.bytes_stream());
    // Commit client headers only after the first raw byte. A stalled endpoint can
    // still fail over here, before any client-visible response has been sent.
    let first = loop {
        match first_deadline.wait(stream.next(), "first byte").await? {
            Some(Ok(bytes)) if bytes.is_empty() => continue,
            Some(Ok(bytes)) => break bytes,
            Some(Err(error)) => return Err(error.without_url().to_string()),
            None => return Err("Upstream stream ended before its first byte".into()),
        }
    };
    Ok(Box::pin(async_stream::stream! {
        yield Ok(first);
        let mut deadline = Deadline::after_seconds(idle_seconds);
        loop {
            match deadline.wait(stream.next(), "stream idle").await {
                Ok(Some(Ok(bytes))) if bytes.is_empty() => continue,
                Ok(Some(Ok(bytes))) => {
                    yield Ok(bytes);
                    deadline = Deadline::after_seconds(idle_seconds);
                }
                Ok(Some(Err(error))) => {
                    yield Err(std::io::Error::other(error.without_url().to_string()));
                    return;
                }
                Err(error) => {
                    yield Err(std::io::Error::new(std::io::ErrorKind::TimedOut, error));
                    return;
                }
                Ok(None) => return,
            }
        }
    }))
}
