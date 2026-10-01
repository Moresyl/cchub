use futures_util::StreamExt;
use serde_json::Value;

#[derive(Debug, PartialEq)]
pub(crate) enum RefreshResponseError {
    ReauthRequired,
    Rejected(u16),
    InvalidPayload,
    Transport,
}

fn rejected_credentials(status: u16, value: &Value) -> bool {
    let error = value.get("error");
    let code = error.and_then(Value::as_str).or_else(|| {
        error
            .and_then(|value| value.get("code"))
            .and_then(Value::as_str)
    });
    matches!(status, 400 | 401 | 403)
        && matches!(
            code,
            Some(
                "invalid_grant"
                    | "invalid_token"
                    | "invalid_refresh_token"
                    | "refresh_token_expired"
                    | "refresh_token_reused"
                    | "refresh_token_invalidated"
            )
        )
}

pub(crate) async fn read_refresh_response(
    response: reqwest::Response,
) -> Result<Value, RefreshResponseError> {
    let status = response.status();
    // A rejected credential remains rejected even if the server returns no JSON.
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(RefreshResponseError::ReauthRequired);
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| RefreshResponseError::Transport)?;
        if chunk.len() > (64 * 1024_usize).saturating_sub(bytes.len()) {
            return Err(RefreshResponseError::InvalidPayload);
        }
        bytes.extend_from_slice(&chunk);
    }
    let value = serde_json::from_slice::<Value>(&bytes);
    if value
        .as_ref()
        .is_ok_and(|value| rejected_credentials(status.as_u16(), value))
    {
        return Err(RefreshResponseError::ReauthRequired);
    }
    if !status.is_success() {
        return Err(RefreshResponseError::Rejected(status.as_u16()));
    }
    value.map_err(|_| RefreshResponseError::InvalidPayload)
}

#[cfg(test)]
mod tests;
