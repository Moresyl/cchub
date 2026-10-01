use super::*;
use futures_util::FutureExt;
use tokio::sync::mpsc;

const CONTENT: &[u8] = b"event: custom\ndata: {\"type\":\"fixture\",\"opaque\":999999999999999999999999999999,\"signature\":\"abc==\"}\n\n";
type Sender = mpsc::Sender<Result<Bytes, std::io::Error>>;

fn harness() -> (Sender, ResponseStream, StreamHealth) {
    let (sender, receiver) = mpsc::channel(8);
    let raw: ResponseStream = Box::pin(futures_util::stream::unfold(
        receiver,
        |mut receiver| async { receiver.recv().await.map(|chunk| (chunk, receiver)) },
    ));
    let (raw, activity) = observe_activity(raw);
    let health = StreamHealth::default();
    let normalized = crate::provider_proxy_transform::normalize_sse_stream(raw);
    let observed = super::super::streaming_health::observe(normalized, health.clone());
    let frames = crate::provider_proxy_transform::stream_frames::frames(observed);
    let translated = frames.filter_map(|chunk| async {
        match chunk {
            Ok(bytes) if bytes.as_ref() == CONTENT => Some(Ok(bytes)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        }
    });
    let output = keep_alive(Box::pin(translated), activity, health.clone());
    let output = super::super::streaming_health::observe_delivery(output, health.clone());
    (sender, Box::pin(output), health)
}

async fn send(sender: &Sender, bytes: &'static [u8]) {
    sender.send(Ok(Bytes::from_static(bytes))).await.unwrap();
}

fn pending(stream: &mut ResponseStream) {
    assert!(stream.next().now_or_never().is_none());
}

async fn ping(stream: &mut ResponseStream) {
    assert_eq!(stream.next().await.unwrap().unwrap().as_ref(), PING);
}

#[tokio::test(start_paused = true)]
async fn activity_can_ping_but_elapsed_time_alone_cannot() {
    let (sender, mut output, health) = harness();
    send(&sender, b": alive\n\n").await;
    ping(&mut output).await;
    tokio::time::advance(Duration::from_secs(30)).await;
    pending(&mut output);
    send(&sender, b": alive\n\n").await;
    ping(&mut output).await;
    drop(sender);
    assert!(output.next().await.is_none());
    assert!(!health.finished());
    assert!(!health.delivered_successfully());
}

#[tokio::test(start_paused = true)]
async fn activity_is_coalesced_and_real_output_resets_the_quiet_gap() {
    let (sender, mut output, _) = harness();
    send(&sender, b": alive\n\n").await;
    ping(&mut output).await;
    for _ in 0..100 {
        send(&sender, b": alive\n\n").await;
        pending(&mut output);
    }
    tokio::time::advance(Duration::from_millis(999)).await;
    send(&sender, b": alive\n\n").await;
    pending(&mut output);
    tokio::time::advance(Duration::from_millis(1)).await;
    send(&sender, b": alive\n\n").await;
    ping(&mut output).await;
    tokio::time::advance(Duration::from_millis(300)).await;
    send(&sender, CONTENT).await;
    assert_eq!(output.next().await.unwrap().unwrap().as_ref(), CONTENT);
    tokio::time::advance(Duration::from_millis(700)).await;
    send(&sender, b": alive\n\n").await;
    pending(&mut output);
    tokio::time::advance(Duration::from_millis(300)).await;
    send(&sender, b": alive\n\n").await;
    ping(&mut output).await;
}

#[tokio::test(start_paused = true)]
async fn partial_comments_crlf_and_utf8_signal_activity_before_framing() {
    let (sender, mut output, _) = harness();
    for fragment in [
        b": partial".as_slice(),
        b"\r",
        b"\n\r\n",
        b"data: \xe4",
        b"\xbd",
        b"\xa0",
    ] {
        send(&sender, fragment).await;
        ping(&mut output).await;
        tokio::time::advance(QUIET_GAP).await;
    }
}

#[tokio::test(start_paused = true)]
async fn terminal_events_suppress_queued_and_future_activity() {
    for terminal in [
        b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".as_slice(),
        b"event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n",
        b"event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"failed\"}}\n\n",
    ] {
        let (sender, mut output, health) = harness();
        send(&sender, b": alive\n\n").await;
        ping(&mut output).await;
        tokio::time::advance(QUIET_GAP).await;
        send(&sender, terminal).await;
        pending(&mut output);
        assert!(health.finished());
        send(&sender, b": alive\n\n").await;
        pending(&mut output);
        assert!(!health.delivered_successfully());
        drop(sender);
        assert!(output.next().await.is_none());
    }
}

#[tokio::test(start_paused = true)]
async fn empty_chunks_do_not_ping_and_transport_errors_end_the_stream() {
    let (sender, mut output, health) = harness();
    send(&sender, b"").await;
    pending(&mut output);
    sender
        .send(Err(std::io::Error::other("fixture failure")))
        .await
        .unwrap();
    assert!(output
        .next()
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("fixture failure"));
    assert!(health.failed());
    assert!(output.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn real_output_has_priority_and_opaque_bytes_are_unchanged() {
    let (sender, mut output, _) = harness();
    send(&sender, CONTENT).await;
    assert_eq!(output.next().await.unwrap().unwrap().as_ref(), CONTENT);
    pending(&mut output);
    drop(sender);
    assert!(output.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_closed_activity_channel_does_not_spin_or_discard_adapter_output() {
    let (sender, receiver) = mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    let raw: ResponseStream = Box::pin(futures_util::stream::unfold(
        receiver,
        |mut receiver| async { receiver.recv().await.map(|chunk| (chunk, receiver)) },
    ));
    let (raw, activity) = observe_activity(raw);
    let ignored = raw.filter_map(|_| async { None });
    let tail = futures_util::stream::once(async {
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(Bytes::from_static(CONTENT))
    });
    let mut output = keep_alive(
        Box::pin(ignored.chain(tail)),
        activity,
        StreamHealth::default(),
    );
    send(&sender, b": alive\n\n").await;
    ping(&mut output).await;
    drop(sender);
    pending(&mut output);
    tokio::time::advance(Duration::from_secs(5)).await;
    assert_eq!(output.next().await.unwrap().unwrap().as_ref(), CONTENT);
    assert!(output.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn dropping_the_client_drops_the_source_without_background_work() {
    let (sender, mut output, health) = harness();
    send(&sender, b": alive\n\n").await;
    ping(&mut output).await;
    drop(output);
    assert!(sender.is_closed());
    assert!(!health.delivered_successfully());
}
