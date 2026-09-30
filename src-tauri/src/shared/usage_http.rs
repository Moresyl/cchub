//! Bounded, credential-scoped requests shared by balance and quota queries.

use serde_json::Value;
use tokio::time::{timeout_at, Instant};

pub(crate) const MAX_USAGE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct ResponseFailure {
    pub(crate) kind: FailureKind,
    pub(crate) message: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FailureKind {
    InvalidRequest,
    InvalidResponse,
    Http(reqwest::StatusCode),
}

impl ResponseFailure {
    fn invalid(message: &str) -> Self {
        Self {
            kind: FailureKind::InvalidResponse,
            message: message.into(),
        }
    }
}

pub(crate) fn finite_number(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse().ok())
        .filter(|number| number.is_finite())
}

/// Automatic vendor routing requires an exact, credential-free official origin.
pub(crate) fn official_url(raw: &str) -> Option<url::Url> {
    let url = url::Url::parse(raw.trim()).ok()?;
    (url.scheme() == "https"
        && url.host_str().is_some()
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}

pub(crate) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("CCHub Usage")
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let same_origin = attempt
                .previous()
                .first()
                .is_some_and(|first| first.origin() == attempt.url().origin());
            if same_origin && attempt.previous().len() < 5 {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|_| "Failed to initialize usage HTTP client".into())
}

/// Outer errors are transient; inner errors are complete but unusable responses.
/// The deadline covers headers and every body chunk, including fallback attempts.
pub(crate) async fn request_json(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    bearer: bool,
    deadline: Instant,
) -> Result<Result<Value, ResponseFailure>, String> {
    timeout_at(deadline, async {
        let mut request = client.get(url).header("Accept", "application/json");
        request = if bearer {
            request.bearer_auth(key)
        } else {
            request.header("Authorization", key)
        };
        let request = match request.build() {
            Ok(request) => request,
            Err(_) => {
                return Ok(Err(ResponseFailure {
                    kind: FailureKind::InvalidRequest,
                    message: "Invalid usage request or credentials".into(),
                }))
            }
        };
        let mut response = client
            .execute(request)
            .await
            .map_err(|error| format!("Usage request failed: {}", error.without_url()))?;
        let status = response.status();
        if !status.is_success() {
            let message = format!("Usage API returned HTTP {}", status.as_u16());
            return if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                Err(message)
            } else {
                Ok(Err(ResponseFailure {
                    kind: FailureKind::Http(status),
                    message,
                }))
            };
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_USAGE_BYTES as u64)
        {
            return Ok(Err(ResponseFailure::invalid(
                "Usage response exceeds the 2 MiB limit",
            )));
        }
        let mut bytes =
            Vec::with_capacity(response.content_length().unwrap_or(0).min(64 * 1024) as usize);
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("Failed to read usage response: {}", error.without_url()))?
        {
            if chunk.len() > MAX_USAGE_BYTES.saturating_sub(bytes.len()) {
                return Ok(Err(ResponseFailure::invalid(
                    "Usage response exceeds the 2 MiB limit",
                )));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(serde_json::from_slice(&bytes)
            .map_err(|_| ResponseFailure::invalid("Usage API returned invalid JSON")))
    })
    .await
    .map_err(|_| "Usage query timed out".to_string())?
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod redirect_tests;

#[cfg(test)]
pub(crate) mod test_support;
