use axum::http::{HeaderName, HeaderValue, Method};
use bytes::Bytes;

use super::super::UpstreamTarget;

pub(super) fn request(
    client: &reqwest::Client,
    method: &Method,
    url: &str,
    headers: &[(HeaderName, HeaderValue)],
    upstream: &UpstreamTarget,
    extra_headers: &[(String, String)],
    compress: bool,
    body: &Bytes,
) -> reqwest::RequestBuilder {
    let mut builder = client.request(method.clone(), url);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    for (name, value) in &upstream.headers {
        builder = builder.header(name, value);
    }
    for (name, value) in &upstream.request_header_overrides {
        builder = builder.header(name, value);
    }
    for (name, value) in extra_headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    if compress {
        builder = builder.header(reqwest::header::ACCEPT_ENCODING, "gzip, deflate, br");
    }
    if !body.is_empty() {
        builder = builder.body(body.clone());
    }
    builder
}
