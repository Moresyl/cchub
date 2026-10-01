use bytes::Bytes;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::Instant;

use super::streaming_health::StreamHealth;
use super::timeouts::ResponseStream;

const QUIET_GAP: Duration = Duration::from_secs(1);
const PING: &[u8] = b"event: ping\ndata: {\"type\":\"ping\"}\n\n";

// The watch retains only one notification, never provider bytes or an
// unbounded queue. It is driven by actual reads and has no background task.
pub(super) fn observe_activity(source: ResponseStream) -> (ResponseStream, watch::Receiver<()>) {
    let (sender, receiver) = watch::channel(());
    let source = source.inspect(move |chunk| {
        if chunk.as_ref().is_ok_and(|bytes| !bytes.is_empty()) {
            sender.send_replace(());
        }
    });
    (Box::pin(source), receiver)
}

// All current adapters deliver Anthropic events. A ping needs no message ID,
// content, usage or completion. Emit only when the provider was heard from;
// a silent/stalled provider still reaches its existing upstream idle deadline.
pub(super) fn keep_alive(
    mut stream: ResponseStream,
    mut activity: watch::Receiver<()>,
    health: StreamHealth,
) -> ResponseStream {
    Box::pin(async_stream::stream! {
        let mut last_delivery: Option<Instant> = None;
        let mut listening = true;
        loop {
            tokio::select! {
                biased;
                chunk = stream.next() => {
                    let Some(chunk) = chunk else { return; };
                    let failed = chunk.is_err();
                    if chunk.as_ref().is_ok_and(|bytes| !bytes.is_empty()) {
                        last_delivery = Some(Instant::now());
                    }
                    yield chunk;
                    if failed { return; }
                }
                changed = activity.changed(), if listening => {
                    if changed.is_err() { listening = false; continue; }
                    if !health.finished() && last_delivery.is_none_or(|last| last.elapsed() >= QUIET_GAP) {
                        last_delivery = Some(Instant::now());
                        yield Ok(Bytes::from_static(PING));
                    }
                }
            }
        }
    })
}

#[cfg(test)]
#[path = "streaming_keepalive/tests.rs"]
mod tests;
