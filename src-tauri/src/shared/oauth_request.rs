use serde_json::Value;
use std::future::Future;
use std::time::Duration;

pub(crate) struct TokenLease {
    pub(crate) account_id: String,
    pub(crate) revision: String,
    pub(crate) token: String,
}

#[derive(Debug, PartialEq)]
pub(crate) enum ResourceError {
    Transport,
    Timeout,
    Http(u16),
    InvalidPayload,
    TooLarge,
}

pub(crate) fn client(
    proxy_url: Option<&str>,
    user_agent: &str,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .user_agent(user_agent)
        .timeout(timeout)
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
        }));
    if let Some(proxy) = proxy_url.map(str::trim).filter(|value| !value.is_empty()) {
        builder = builder.proxy(
            reqwest::Proxy::all(proxy).map_err(|_| "Invalid OAuth query proxy".to_string())?,
        );
    } else if cfg!(test) {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .map_err(|_| "Failed to initialize OAuth query client".into())
}

pub(crate) trait TokenProvider: Sync {
    type Error: From<ResourceError>;

    fn lease(
        &self,
        account_id: Option<&str>,
    ) -> impl Future<Output = Result<TokenLease, Self::Error>> + Send;
    fn validate(&self, lease: &TokenLease) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn recover(
        &self,
        lease: &TokenLease,
    ) -> impl Future<Output = Result<TokenLease, Self::Error>> + Send;
    fn expire(&self, lease: &TokenLease) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub(crate) async fn get_json<P, F>(
    provider: &P,
    account_id: Option<&str>,
    request: F,
) -> Result<Value, P::Error>
where
    P: TokenProvider,
    F: Fn(&TokenLease) -> reqwest::RequestBuilder,
{
    // One deadline covers cache reads, refresh, both requests and streaming bodies.
    get_json_before(
        provider,
        account_id,
        request,
        tokio::time::Instant::now() + Duration::from_secs(45),
    )
    .await
}

async fn get_json_before<P, F>(
    provider: &P,
    account_id: Option<&str>,
    request: F,
    deadline: tokio::time::Instant,
) -> Result<Value, P::Error>
where
    P: TokenProvider,
    F: Fn(&TokenLease) -> reqwest::RequestBuilder,
{
    tokio::time::timeout_at(deadline, async {
        let mut lease = provider.lease(account_id).await?;
        for attempt in 0..2 {
            let response = request(&lease).send().await;
            provider.validate(&lease).await?;
            let response = response.map_err(|_| ResourceError::Transport)?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                if attempt == 0 {
                    lease = provider.recover(&lease).await?;
                    continue;
                }
                provider.expire(&lease).await?;
                return Err(ResourceError::Http(401).into());
            }
            let result = read_json(response).await;
            // A removed or replaced account owns neither a late success nor error.
            provider.validate(&lease).await?;
            return result.map_err(Into::into);
        }
        unreachable!("the second attempt always returns")
    })
    .await
    .map_err(|_| P::Error::from(ResourceError::Timeout))?
}

pub(crate) async fn read_json(mut response: reqwest::Response) -> Result<Value, ResourceError> {
    if !response.status().is_success() {
        // Error bodies may contain credentials or proxy challenge HTML.
        return Err(ResourceError::Http(response.status().as_u16()));
    }
    const MAX_BYTES: usize = 2 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BYTES as u64)
    {
        return Err(ResourceError::TooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ResourceError::Transport)?
    {
        if chunk.len() > MAX_BYTES.saturating_sub(bytes.len()) {
            return Err(ResourceError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| ResourceError::InvalidPayload)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_support;
