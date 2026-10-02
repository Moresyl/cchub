use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use bytes::Bytes;

use super::super::UpstreamTarget;

fn base_has_credentials(upstream: &UpstreamTarget) -> bool {
    url::Url::parse(&upstream.base_url).ok().is_some_and(|url| {
        url.query_pairs()
            .any(|(name, _)| ["key", "api_key", "access_token", "token"].contains(&name.as_ref()))
    })
}

pub(in crate::provider_proxy) fn upstream_query(
    query: Option<&str>,
    upstream: &UpstreamTarget,
) -> Option<String> {
    let query = query?;
    let credential_names = ["authorization", "x-api-key", "x-goog-api-key", "api-key"];
    let owns_credentials = upstream
        .headers
        .iter()
        .chain(&upstream.request_header_overrides)
        .any(|(name, _)| credential_names.contains(&name.to_ascii_lowercase().as_str()))
        || base_has_credentials(upstream);
    if !owns_credentials {
        return Some(query.into());
    }
    // Configured base-URL credentials remain in the base URL. Remove only
    // client authentication query fields, which otherwise override them.
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if !["key", "api_key", "access_token", "token"].contains(&name.as_ref()) {
            serializer.append_pair(&name, &value);
        }
    }
    let query = serializer.finish();
    (!query.is_empty()).then_some(query)
}

pub(super) fn forwarded_headers(
    headers: &[(HeaderName, HeaderValue)],
    upstream: &UpstreamTarget,
    path: &str,
) -> Vec<(HeaderName, HeaderValue)> {
    headers
        .iter()
        .filter(|(name, _)| {
            !super::super::is_hop_by_hop_header(name.as_str())
                && !super::super::profiles::should_strip_claude_transform_header(
                    name.as_str(),
                    upstream.claude_api_format,
                    path,
                )
        })
        .cloned()
        .collect()
}

pub(in crate::provider_proxy) fn effective_headers(
    headers: &[(HeaderName, HeaderValue)],
    upstream: &UpstreamTarget,
    extra_headers: &[(String, String)],
) -> Result<HeaderMap, &'static str> {
    let mut merged = HeaderMap::new();
    let credential_names = [
        "authorization",
        "x-api-key",
        "x-goog-api-key",
        "api-key",
        "chatgpt-account-id",
    ];
    let owns_credentials = upstream
        .headers
        .iter()
        .chain(&upstream.request_header_overrides)
        .chain(extra_headers)
        .any(|(name, _)| credential_names.contains(&name.to_ascii_lowercase().as_str()))
        || base_has_credentials(upstream);
    for (name, value) in headers {
        if owns_credentials && credential_names.contains(&name.as_str()) {
            continue;
        }
        merged.append(name, value.clone());
    }
    // Later sources override earlier values by name. Local client credentials
    // must never accompany an upstream credential under the same header name.
    for source in [
        upstream.headers.as_slice(),
        upstream.request_header_overrides.as_slice(),
        extra_headers,
    ] {
        let mut overlay = HeaderMap::new();
        for (name, value) in source {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| "Invalid upstream header name")?;
            let value =
                HeaderValue::from_str(value).map_err(|_| "Invalid upstream header value")?;
            overlay.append(name, value);
        }
        for name in overlay.keys() {
            merged.remove(name);
        }
        for (name, value) in &overlay {
            merged.append(name, value.clone());
        }
    }
    Ok(merged)
}

pub(super) fn request(
    client: &reqwest::Client,
    method: &Method,
    url: &str,
    headers: &HeaderMap,
    compress: bool,
    body: &Bytes,
) -> reqwest::RequestBuilder {
    let mut builder = client.request(method.clone(), url).headers(headers.clone());
    if compress {
        builder = builder.header(reqwest::header::ACCEPT_ENCODING, "gzip, deflate, br");
    }
    if !body.is_empty() {
        builder = builder.body(body.clone());
    }
    builder
}
