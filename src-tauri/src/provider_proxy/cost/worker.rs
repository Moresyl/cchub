//! One FIFO writer owns database work independently of async runtime teardown.
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc, Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Default)]
struct Pending {
    count: Mutex<usize>,
    idle: Condvar,
}

struct Completion(Arc<Pending>);

impl Drop for Completion {
    fn drop(&mut self) {
        if let Ok(mut count) = self.0.count.lock() {
            *count -= 1;
            self.0.idle.notify_all();
        }
    }
}

pub(super) struct Writer {
    sender: mpsc::Sender<(Job, Completion)>,
    pending: Arc<Pending>,
}

impl Writer {
    pub(super) fn start() -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel::<(Job, Completion)>();
        std::thread::Builder::new()
            .name("proxy-accounting".into())
            .spawn(move || {
                while let Ok((job, completion)) = receiver.recv() {
                    // A failed job must not strand subsequent paid records.
                    let _ = catch_unwind(AssertUnwindSafe(job));
                    drop(completion);
                }
            })
            .map_err(|_| "Proxy accounting worker could not start".to_string())?;
        Ok(Self {
            sender,
            pending: Arc::default(),
        })
    }

    pub(super) fn submit<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<T, String>>, String> {
        let (reply, receiver) = tokio::sync::oneshot::channel();
        let mut count = self
            .pending
            .count
            .lock()
            .map_err(|_| "Proxy accounting queue is unavailable".to_string())?;
        *count += 1;
        let completion = Completion(self.pending.clone());
        let job = Box::new(move || {
            let result = catch_unwind(AssertUnwindSafe(work))
                .unwrap_or_else(|_| Err("Proxy accounting worker could not complete".into()));
            let _ = reply.send(result);
        });
        // Release the counter lock before a failed send drops Completion.
        drop(count);
        self.sender
            .send((job, completion))
            .map_err(|_| "Proxy accounting queue is unavailable".to_string())?;
        Ok(receiver)
    }

    pub(super) fn drain(&self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        let mut count = self
            .pending
            .count
            .lock()
            .map_err(|_| "Proxy accounting queue is unavailable".to_string())?;
        while *count != 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Proxy accounting is still being saved".into());
            }
            let result = self
                .pending
                .idle
                .wait_timeout(count, remaining)
                .map_err(|_| "Proxy accounting queue is unavailable".to_string())?;
            count = result.0;
        }
        Ok(())
    }
}

pub(super) fn writer() -> Result<&'static Writer, String> {
    static WRITER: OnceLock<Result<Writer, String>> = OnceLock::new();
    WRITER
        .get_or_init(Writer::start)
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
