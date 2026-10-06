//! Exit cancellation and stream snapshots share one registration boundary.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

type Finish = Box<dyn FnOnce() + Send>;

pub(in crate::provider_proxy) struct Lifecycle {
    closing: tokio::sync::watch::Sender<bool>,
    streams: Mutex<HashMap<String, Finish>>,
}

impl Default for Lifecycle {
    fn default() -> Self {
        let (closing, _) = tokio::sync::watch::channel(false);
        Self {
            closing,
            streams: Mutex::default(),
        }
    }
}

impl Lifecycle {
    pub(in crate::provider_proxy) fn register(&self, id: String, finish: Finish) {
        let mut streams = self
            .streams
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *self.closing.borrow() {
            drop(streams);
            finish();
        } else {
            streams.insert(id, finish);
        }
    }

    pub(in crate::provider_proxy) fn remove(&self, id: &str) {
        self.streams
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(id);
    }

    pub(in crate::provider_proxy) fn close(&self) {
        let mut streams = self
            .streams
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        self.closing.send_replace(true);
        let snapshots = std::mem::take(&mut *streams);
        drop(streams);
        // Snapshot outside the registry lock; dropping a stream may unregister.
        for (_, finish) in snapshots {
            finish();
        }
    }

    pub(in crate::provider_proxy) fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.closing.subscribe()
    }

    pub(in crate::provider_proxy) async fn until_closing<T>(
        &self,
        work: impl std::future::Future<Output = T>,
    ) -> Option<T> {
        let mut closing = self.subscribe();
        if *closing.borrow() {
            return None;
        }
        tokio::select! {
            biased;
            _ = closing.changed() => None,
            result = work => Some(result),
        }
    }
}

pub(in crate::provider_proxy) fn lifecycle() -> &'static Arc<Lifecycle> {
    static LIFECYCLE: OnceLock<Arc<Lifecycle>> = OnceLock::new();
    LIFECYCLE.get_or_init(|| Arc::new(Lifecycle::default()))
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
