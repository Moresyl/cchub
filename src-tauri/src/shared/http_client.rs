use std::time::Duration;

pub fn build_http_client(
    proxy_url: Option<&str>,
    user_agent: Option<&str>,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder().timeout(timeout);
    build_http_client_from_builder(builder, proxy_url, user_agent)
}

pub fn build_http_client_no_timeout(
    proxy_url: Option<&str>,
    user_agent: Option<&str>,
) -> Result<reqwest::Client, String> {
    build_http_client_from_builder(reqwest::Client::builder(), proxy_url, user_agent)
}

pub fn default_http_client() -> reqwest::Client {
    reqwest::Client::new()
}

fn build_http_client_from_builder(
    mut builder: reqwest::ClientBuilder,
    proxy_url: Option<&str>,
    user_agent: Option<&str>,
) -> Result<reqwest::Client, String> {
    if let Some(user_agent) = user_agent {
        builder = builder.user_agent(user_agent);
    }

    if let Some(proxy_url) = proxy_url.map(str::trim).filter(|value| !value.is_empty()) {
        let proxy =
            reqwest::Proxy::all(proxy_url).map_err(|error| format!("Invalid proxy: {error}"))?;
        builder = builder.proxy(proxy);
    }

    builder
        .build()
        .map_err(|error| format!("Failed to build HTTP client: {error}"))
}

pub fn build_http_client_with_headers(
    proxy_url: Option<&str>,
    headers: reqwest::header::HeaderMap,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(timeout);

    if let Some(proxy_url) = proxy_url.map(str::trim).filter(|value| !value.is_empty()) {
        let proxy =
            reqwest::Proxy::all(proxy_url).map_err(|error| format!("Invalid proxy: {error}"))?;
        builder = builder.proxy(proxy);
    }

    builder
        .build()
        .map_err(|error| format!("Failed to build HTTP client: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_client_without_timeout() {
        let client = build_http_client_no_timeout(None, Some("CCHub Test"));

        assert!(client.is_ok());
    }

    #[test]
    fn rejects_invalid_proxy_url() {
        let error = build_http_client(None, None, Duration::from_secs(1))
            .and_then(|_| build_http_client(Some("not a proxy url"), None, Duration::from_secs(1)))
            .expect_err("invalid proxy should fail");

        assert!(error.contains("Invalid proxy"));
    }
}
