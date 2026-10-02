//! Serialize cloud uploads/restores across both storage adapters.
use std::sync::OnceLock;

pub(crate) fn workflow_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[cfg(test)]
mod auto_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelling_a_cloud_workflow_releases_the_shared_slot() {
        let (ready, started) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _guard = workflow_lock().lock().await;
            ready.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        started.await.unwrap();
        assert!(workflow_lock().try_lock().is_err());
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let _guard =
            tokio::time::timeout(std::time::Duration::from_secs(5), workflow_lock().lock())
                .await
                .expect("cancelled transfer must release its lock");
    }
}
