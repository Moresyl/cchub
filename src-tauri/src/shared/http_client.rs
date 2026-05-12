use std::sync::OnceLock;
use std::time::Duration;

/// 全局复用的 reqwest::Client 缓存。
///
/// reqwest::Client::build() 会执行 TLS 配置初始化（加载根证书）和创建 connection pool，
/// 反复创建会丢失 HTTP keep-alive 并导致每次请求都重新 TLS 握手。
/// 这里把"无代理 + 默认 UA"和"无代理 + Provider Proxy UA + 无超时"这两类
/// 最热的客户端做静态缓存；带代理 / 自定义 headers 的仍按需 build。
static DEFAULT_CCHUB_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static PROVIDER_PROXY_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn fallback_client() -> reqwest::Client {
    reqwest::Client::new()
}

/// "CCHub" UA + 30s timeout 的共享客户端（无代理场景）。
pub fn shared_cchub_client_30s() -> reqwest::Client {
    DEFAULT_CCHUB_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("CCHub")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| fallback_client())
        })
        .clone()
}

/// 本地 provider proxy 用的共享客户端（无 timeout，因为要支持长连接 SSE）。
pub fn shared_provider_proxy_client() -> reqwest::Client {
    PROVIDER_PROXY_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("CCHub Local Provider Proxy")
                .build()
                .unwrap_or_else(|_| fallback_client())
        })
        .clone()
}

pub fn build_http_client(
    proxy_url: Option<&str>,
    user_agent: Option<&str>,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    // 命中"无代理 + CCHub UA + 30s 超时"的最常见组合时直接返回共享实例。
    if proxy_url.map(str::trim).filter(|v| !v.is_empty()).is_none()
        && matches!(user_agent, Some("CCHub"))
        && timeout == Duration::from_secs(30)
    {
        return Ok(shared_cchub_client_30s());
    }
    let builder = reqwest::Client::builder().timeout(timeout);
    build_http_client_from_builder(builder, proxy_url, user_agent)
}

pub fn build_http_client_no_timeout(
    proxy_url: Option<&str>,
    user_agent: Option<&str>,
) -> Result<reqwest::Client, String> {
    if proxy_url.map(str::trim).filter(|v| !v.is_empty()).is_none()
        && matches!(user_agent, Some("CCHub Local Provider Proxy"))
    {
        return Ok(shared_provider_proxy_client());
    }
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
