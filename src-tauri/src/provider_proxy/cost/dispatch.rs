//! Serialize accounting work without holding a runtime worker across SQLite IO.
static WRITES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
// Match the proxy's existing global active-request ceiling. Reserving before
// upstream work bounds detached records even when clients repeatedly disconnect.
static REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8192);

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
    // Enqueue immediately, before the first await. Cancellation while waiting
    // for the writer must retain a paid record just like cancellation during IO.
    let task = tokio::spawn(async move {
        let permit = WRITES.lock().await;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _lease = lease;
            work()
        })
        .await
        .map_err(|_| "Proxy accounting worker could not complete".to_string())?
    });
    async move {
        task.await
            .map_err(|_| "Proxy accounting task could not complete".to_string())?
    }
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
