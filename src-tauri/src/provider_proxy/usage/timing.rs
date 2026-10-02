use std::sync::{Arc, Mutex};
use std::time::Instant;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};

#[path = "timing/output.rs"]
mod output;

const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::provider_proxy) struct StreamTiming {
    pub first_output_ms: Option<u64>,
    pub generation_ms: Option<u64>,
}

#[derive(Default)]
struct Inspector {
    frame: Vec<u8>,
    previous_cr: bool,
    line_length: usize,
    unreliable: bool,
    first: Option<u64>,
    last: Option<u64>,
}

impl Inspector {
    fn observe(&mut self, bytes: &[u8], elapsed_ms: u64) {
        if self.unreliable {
            return;
        }
        for &byte in bytes {
            if self.previous_cr {
                self.previous_cr = false;
                if byte == b'\n' {
                    continue;
                }
            }
            let byte = if byte == b'\r' {
                self.previous_cr = true;
                b'\n'
            } else {
                byte
            };
            if self.frame.len() >= MAX_FRAME_BYTES {
                // An oversized event may contain earlier output we cannot inspect.
                // Disable timing for this attempt; bytes still pass through exactly.
                self.unreliable = true;
                self.frame.clear();
                return;
            }
            self.frame.push(byte);
            if byte != b'\n' {
                self.line_length += 1;
                continue;
            }
            let empty = self.line_length == 0;
            self.line_length = 0;
            if empty {
                self.finish_frame(elapsed_ms);
                self.frame.clear();
            }
        }
    }

    fn finish_frame(&mut self, elapsed_ms: u64) {
        let Ok(frame) = std::str::from_utf8(&self.frame) else {
            self.unreliable = true;
            return;
        };
        let mut event = "";
        let mut data = Vec::new();
        for line in frame.trim_start_matches('\u{feff}').lines() {
            if let Some(value) = line.strip_prefix("event:") {
                event = value.trim();
            }
            if let Some(value) = line.strip_prefix("data:") {
                data.push(value.strip_prefix(' ').unwrap_or(value));
            }
        }
        if data.is_empty() {
            return;
        }
        if let Ok(value) = serde_json::from_str(&data.join("\n")) {
            if output::has_output(event, &value) {
                self.first.get_or_insert(elapsed_ms);
                self.last = Some(elapsed_ms);
            }
        }
    }

    fn snapshot(&self) -> StreamTiming {
        if self.unreliable {
            return StreamTiming::default();
        }
        StreamTiming {
            first_output_ms: self.first,
            generation_ms: self
                .first
                .zip(self.last)
                .map(|(first, last)| last.saturating_sub(first)),
        }
    }
}

#[derive(Clone)]
pub(in crate::provider_proxy) struct StreamTimingCapture {
    started_at: Instant,
    inspector: Arc<Mutex<Inspector>>,
}

impl StreamTimingCapture {
    pub(in crate::provider_proxy) fn new(started_at: Instant) -> Self {
        Self {
            started_at,
            inspector: Arc::new(Mutex::new(Inspector::default())),
        }
    }

    pub(in crate::provider_proxy) fn snapshot(&self) -> StreamTiming {
        self.inspector
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshot()
    }
}

// Observe original upstream receipt before preflight buffering or adapters.
// No response payload survives in the shared capture, only bounded parser state.
pub(in crate::provider_proxy) fn observe_stream_timing<S, E>(
    stream: S,
    capture: StreamTimingCapture,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: Send + 'static,
{
    async_stream::stream! {
        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            if let Ok(bytes) = &chunk {
                let elapsed = capture.started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                capture.inspector.lock().unwrap_or_else(|error| error.into_inner()).observe(bytes, elapsed);
            }
            yield chunk;
        }
    }
}

#[cfg(test)]
#[path = "timing/tests.rs"]
mod tests;
