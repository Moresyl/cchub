use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[test]
fn close_snapshots_every_registered_stream_once_and_wakes_waiters() {
    let lifecycle = Lifecycle::default();
    let closing = lifecycle.subscribe();
    let finished = Arc::new(AtomicUsize::new(0));
    for id in ["one", "two"] {
        let finished = finished.clone();
        lifecycle.register(
            id.into(),
            Box::new(move || {
                finished.fetch_add(1, Ordering::SeqCst);
            }),
        );
    }
    lifecycle.close();
    lifecycle.close();
    assert!(*closing.borrow());
    assert_eq!(finished.load(Ordering::SeqCst), 2);
}

#[test]
fn finished_streams_unregister_and_late_streams_are_snapshotted_immediately() {
    let lifecycle = Lifecycle::default();
    let finished = Arc::new(AtomicUsize::new(0));
    let removed = finished.clone();
    lifecycle.register(
        "removed".into(),
        Box::new(move || {
            removed.fetch_add(1, Ordering::SeqCst);
        }),
    );
    lifecycle.remove("removed");
    lifecycle.close();
    assert_eq!(finished.load(Ordering::SeqCst), 0);
    let late = finished.clone();
    lifecycle.register(
        "late".into(),
        Box::new(move || {
            late.fetch_add(1, Ordering::SeqCst);
        }),
    );
    assert_eq!(finished.load(Ordering::SeqCst), 1);
    assert!(lifecycle.streams.lock().unwrap().is_empty());
}

#[test]
fn snapshot_callbacks_can_unregister_without_deadlocking() {
    let lifecycle = Arc::new(Lifecycle::default());
    let weak = Arc::downgrade(&lifecycle);
    lifecycle.register(
        "one".into(),
        Box::new(move || {
            weak.upgrade().unwrap().remove("one");
        }),
    );
    lifecycle.close();
    assert!(lifecycle.streams.lock().unwrap().is_empty());
}

#[tokio::test]
async fn shutdown_cancels_a_waiting_request_and_releases_its_resources() {
    let lifecycle = Arc::new(Lifecycle::default());
    let capacity = Arc::new(tokio::sync::Semaphore::new(1));
    let permit = capacity.clone().acquire_owned().await.unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let worker_lifecycle = lifecycle.clone();
    let request = tokio::spawn(async move {
        worker_lifecycle
            .until_closing(async move {
                let _permit = permit;
                started.send(()).unwrap();
                std::future::pending::<()>().await;
            })
            .await
    });
    ready.await.unwrap();
    assert_eq!(capacity.available_permits(), 0);
    lifecycle.close();
    assert!(request.await.unwrap().is_none());
    assert_eq!(capacity.available_permits(), 1);
}

#[tokio::test]
async fn requests_started_after_shutdown_are_never_polled() {
    let lifecycle = Lifecycle::default();
    lifecycle.close();
    assert!(lifecycle
        .until_closing(async {
            panic!("upstream must not be requested");
        })
        .await
        .is_none());
}

#[tokio::test]
async fn completed_requests_keep_their_result_before_shutdown() {
    let lifecycle = Lifecycle::default();
    assert_eq!(lifecycle.until_closing(async { 7 }).await, Some(7));
}
