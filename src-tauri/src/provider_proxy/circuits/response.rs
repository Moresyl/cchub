use super::CircuitLease;
use crate::provider_proxy::forward::streaming_health::StreamHealth;

struct ResponseLease<F: FnOnce()> {
    profile: CircuitLease,
    endpoint: CircuitLease,
    successful_status: bool,
    health: StreamHealth,
    on_success: Option<F>,
}

impl<F: FnOnce()> ResponseLease<F> {
    fn fail(&mut self) {
        self.on_success.take();
        self.endpoint.failure();
        self.profile.failure();
    }

    fn finish(&mut self) {
        if self.health.failed() {
            self.fail();
        } else if self.successful_status && self.health.verified() {
            if let Some(on_success) = self.on_success.take() {
                let endpoint = self.endpoint.success();
                let profile = self.profile.success();
                if endpoint && profile {
                    on_success();
                }
            }
        }
    }
}

impl<F: FnOnce()> Drop for ResponseLease<F> {
    fn drop(&mut self) {
        if self.health.failed() {
            self.fail();
        } else if self.health.delivered_successfully() {
            self.finish();
        }
        // Otherwise the individual leases release their probe without success.
    }
}

pub(in crate::provider_proxy) fn track_body(
    body: axum::body::Body,
    profile: CircuitLease,
    endpoint: CircuitLease,
    successful_status: bool,
    health: StreamHealth,
    on_success: impl FnOnce() + Send + 'static,
) -> axum::body::Body {
    use futures_util::StreamExt;
    // Capture outside the generator so never-polled bodies also release probes.
    let owner = ResponseLease {
        profile,
        endpoint,
        successful_status,
        health,
        on_success: Some(on_success),
    };
    axum::body::Body::from_stream(async_stream::stream! {
        let mut owner = owner;
        let stream = body.into_data_stream();
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if owner.health.failed() { owner.fail(); }
                    yield Ok(bytes);
                }
                Err(error) => {
                    owner.fail();
                    yield Err(error);
                    return;
                }
            }
        }
        owner.finish();
    })
}
