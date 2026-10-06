//! Serialize accounting work without holding a runtime worker across SQLite IO.
// Match the proxy's existing global active-request ceiling. Reserving before
// upstream work bounds detached records even when clients repeatedly disconnect.
static REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8192);

pub(super) fn close_requests() {
    REQUESTS.close();
}

pub(super) fn active_requests() -> usize {
    8192 - REQUESTS.available_permits()
}

#[derive(Clone)]
pub(in crate::provider_proxy) struct AccountingLease {
    _permit: std::sync::Arc<tokio::sync::SemaphorePermit<'static>>,
}

pub(in crate::provider_proxy) async fn reserve() -> Result<AccountingLease, String> {
    reserve_from(&REQUESTS).await
}

async fn reserve_from(
    capacity: &'static tokio::sync::Semaphore,
) -> Result<AccountingLease, String> {
    capacity
        .acquire()
        .await
        .map(|permit| AccountingLease {
            _permit: std::sync::Arc::new(permit),
        })
        .map_err(|_| "Proxy accounting is unavailable".into())
}

pub(super) fn write<T: Send + 'static>(
    lease: &AccountingLease,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl std::future::Future<Output = Result<T, String>> + Send + 'static {
    let lease = lease.clone();
    // Enqueue before returning the future. The thread and its records survive
    // cancellation and teardown of the async runtime that submitted the work.
    let submitted = super::worker::writer().and_then(|writer| {
        writer.submit(move || {
            let _lease = lease;
            work()
        })
    });
    async move {
        submitted?
            .await
            .map_err(|_| "Proxy accounting task could not complete".to_string())?
    }
}

pub(in crate::provider_proxy) fn drain(timeout: std::time::Duration) -> Result<(), String> {
    super::worker::writer()?.drain(timeout)
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
