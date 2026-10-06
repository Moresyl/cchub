use super::*;
use futures_util::StreamExt;

const USAGE: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}\n\n";

#[tokio::test(flavor = "current_thread")]
async fn cancelling_a_stream_never_waits_for_the_accounting_database_lock() {
    let upstream = streaming_tests::split_server_with_pending(USAGE, true).await;
    let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
    let response = forward(app.handle().clone(), true).await;
    let mut body = response.into_body().into_data_stream();
    let mut received = Vec::new();
    while !received.ends_with(b"\n\n") {
        received.extend_from_slice(&body.next().await.unwrap().unwrap());
    }
    let holder_app = app.handle().clone();
    let (locked, ready) = tokio::sync::oneshot::channel();
    let (release, blocked) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let db = holder_app.state::<DbState>();
        let _conn = db.0.lock().unwrap();
        locked.send(()).unwrap();
        blocked.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    ready.await.unwrap();
    let before = Instant::now();
    drop(body);
    let latency = before.elapsed();
    release.send(()).unwrap();
    holder.join().unwrap();
    assert!(
        latency < Duration::from_millis(100),
        "stream cancellation blocked for {latency:?}"
    );
    streaming_tests::assert_single_outcome(&app, 499, 0, 7);
}

#[test]
fn an_unpolled_body_can_record_cancellation_after_its_runtime_has_stopped() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (upstream, app, response) = runtime.block_on(async {
        let upstream = server(StatusCode::OK, "text/event-stream", USAGE).await;
        let app = app(&[("p1", &upstream.url, vec![])], OptimizerConfig::default());
        let response = forward(app.handle().clone(), true).await;
        (upstream, app, response)
    });
    drop(runtime);
    drop(response);
    streaming_tests::assert_single_outcome(&app, 499, 0, 0);
    drop(upstream);
}
