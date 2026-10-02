use super::super::{
    admission::{Permit, Rejection},
    build_proxy_error, LocalProviderProxyRuntime, UpstreamTarget,
};
use axum::{
    body::Body,
    http::{Response, StatusCode},
};
use tauri::{AppHandle, Manager};

pub(super) async fn prepare<R: tauri::Runtime>(
    app: &AppHandle<R>,
    upstream: &UpstreamTarget,
    headers: &[(axum::http::HeaderName, axum::http::HeaderValue)],
    extra: &[(String, String)],
    queued_bytes: usize,
    path: &str,
    query: Option<&str>,
) -> Result<(axum::http::HeaderMap, Permit), Response<Body>> {
    let invalid = |error: &str| build_proxy_error(StatusCode::BAD_REQUEST, error.into());
    let mut headers =
        super::transport::effective_headers(headers, upstream, extra).map_err(invalid)?;
    let url = super::super::build_upstream_request_url(
        &upstream.base_url,
        path,
        query,
        upstream.use_full_url,
    )
    .map_err(|error| invalid(&error))?;
    let url = url::Url::parse(&url).map_err(|_| invalid("Invalid upstream URL"))?;
    let key = super::super::admission::scope_key(upstream, &headers, &url).map_err(invalid)?;
    let permit = acquire(app, upstream, key, queued_bytes).await?;
    if let Some(principal) = upstream
        .managed_principal
        .as_ref()
        .filter(|_| super::super::admission::uses_managed_identity(upstream, &headers, &url))
    {
        let fresh = principal
            .current_headers(app)
            .await
            .map_err(|error| build_proxy_error(StatusCode::SERVICE_UNAVAILABLE, error.into()))?;
        // Only renew credentials; preserve client classification, protocol and
        // caller-approved non-authentication transport headers.
        for (name, value) in fresh {
            if ["authorization", "x-api-key", "chatgpt-account-id"].contains(&name.as_str()) {
                let name = axum::http::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| invalid("Invalid account header"))?;
                let value = axum::http::HeaderValue::from_str(&value)
                    .map_err(|_| invalid("Invalid account credential"))?;
                headers.insert(name, value);
            }
        }
    }
    Ok((headers, permit))
}

pub(super) async fn acquire<R: tauri::Runtime>(
    app: &AppHandle<R>,
    upstream: &UpstreamTarget,
    key: String,
    queued_bytes: usize,
) -> Result<Permit, Response<Body>> {
    let store = app
        .state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .map_err(|_| rejected(Rejection::Unavailable))?
        .admission
        .clone();
    let permit = store
        .acquire_with_bytes(key, &upstream.profile_name, queued_bytes)
        .await
        .map_err(rejected)?;
    if upstream
        .managed_principal
        .as_ref()
        .is_some_and(|principal| !principal.if_current(app, || {}))
    {
        return Err(build_proxy_error(
            StatusCode::CONFLICT,
            "The account changed while waiting; retry with the current account configuration"
                .into(),
        ));
    }
    Ok(permit)
}

fn rejected(reason: Rejection) -> Response<Body> {
    let message = match reason {
        Rejection::QueueFull => "The local account queue is full; retry later",
        Rejection::TimedOut => "The local account queue wait expired; retry later",
        Rejection::Capacity => "Local request capacity is exhausted; retry later",
        Rejection::Unavailable => "Local request admission is unavailable",
    };
    let mut response = build_proxy_error(StatusCode::SERVICE_UNAVAILABLE, message.into());
    response
        .headers_mut()
        .insert("retry-after", axum::http::HeaderValue::from_static("1"));
    response
}
