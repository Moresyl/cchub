use super::*;
use futures_util::{poll, StreamExt};

fn store(limit: u32, queued: u32) -> Store {
    let store = Store::default();
    store
        .configure(AdmissionConfig {
            max_concurrent: limit,
            max_queued: queued,
            ..Default::default()
        })
        .unwrap();
    store
}

fn counts(store: &Store, active: usize, queued: usize) {
    let stats = store.stats().unwrap();
    assert_eq!((stats.active, stats.queued), (active, queued));
    assert_eq!(
        stats
            .entries
            .iter()
            .map(|entry| entry.active)
            .sum::<usize>(),
        active
    );
    assert_eq!(
        stats
            .entries
            .iter()
            .map(|entry| entry.queued)
            .sum::<usize>(),
        queued
    );
}

#[tokio::test]
async fn fifo_cancellation_and_queue_full_do_not_leak_slots() {
    let store = store(1, 2);
    let first = store.acquire("account".into(), "one").await.unwrap();
    let mut second = Box::pin(store.acquire("account".into(), "two"));
    let mut third = Box::pin(store.acquire("account".into(), "three"));
    assert!(poll!(second.as_mut()).is_pending());
    assert!(poll!(third.as_mut()).is_pending());
    assert!(matches!(
        store.acquire("account".into(), "full").await,
        Err(Rejection::QueueFull)
    ));
    counts(&store, 1, 2);
    drop(second);
    counts(&store, 1, 1);
    let mut fourth = Box::pin(store.acquire("account".into(), "four"));
    assert!(poll!(fourth.as_mut()).is_pending());
    drop(first);
    let third = third.await.unwrap();
    assert!(poll!(fourth.as_mut()).is_pending());
    counts(&store, 1, 1);
    drop(third);
    let fourth = fourth.await.unwrap();
    counts(&store, 1, 0);
    drop(fourth);
    assert!(store.0.lock().unwrap().lanes.is_empty());
}

#[tokio::test(start_paused = true)]
async fn deadline_and_grant_race_release_only_their_ticket() {
    let store = store(1, 2);
    let first = store.acquire("one".into(), "first").await.unwrap();
    let mut wait = Box::pin(store.acquire("one".into(), "wait"));
    assert!(poll!(wait.as_mut()).is_pending());
    tokio::time::advance(Duration::from_secs(30)).await;
    drop(first);
    assert!(matches!(wait.await, Err(Rejection::TimedOut)));
    counts(&store, 0, 0);
    let first = store.acquire("one".into(), "first").await.unwrap();
    let mut raced = Box::pin(store.acquire("one".into(), "raced"));
    assert!(poll!(raced.as_mut()).is_pending());
    drop(first); // grant is in the channel but has not been consumed
    counts(&store, 1, 0);
    drop(raced);
    counts(&store, 0, 0);
    let next = store.acquire("one".into(), "next").await.unwrap();
    counts(&store, 1, 0);
    drop(next);
}

#[tokio::test]
async fn limits_update_without_cancelling_work_or_allowing_fifo_overtaking() {
    let store = store(1, 4);
    let first = store.acquire("one".into(), "first").await.unwrap();
    let mut second = Box::pin(store.acquire("one".into(), "second"));
    let mut third = Box::pin(store.acquire("one".into(), "third"));
    assert!(poll!(second.as_mut()).is_pending());
    assert!(poll!(third.as_mut()).is_pending());
    store
        .configure(AdmissionConfig {
            max_concurrent: 2,
            ..Default::default()
        })
        .unwrap();
    let second = second.await.unwrap();
    let mut fourth = Box::pin(store.acquire("one".into(), "fourth"));
    assert!(poll!(fourth.as_mut()).is_pending());
    store
        .configure(AdmissionConfig {
            max_concurrent: 1,
            ..Default::default()
        })
        .unwrap();
    counts(&store, 2, 2);
    drop(first);
    assert!(poll!(third.as_mut()).is_pending());
    drop(second);
    let third = third.await.unwrap();
    assert!(poll!(fourth.as_mut()).is_pending());
    store.configure(AdmissionConfig::default()).unwrap(); // zero lifts the account limit
    let fourth = fourth.await.unwrap();
    counts(&store, 2, 0);
    drop((third, fourth));
    counts(&store, 0, 0);
}

#[tokio::test(start_paused = true)]
async fn changing_queue_policy_keeps_original_deadlines_and_reservations() {
    let store = store(1, 4);
    let first = store.acquire("one".into(), "first").await.unwrap();
    let mut wait = Box::pin(store.acquire("one".into(), "wait"));
    assert!(poll!(wait.as_mut()).is_pending());
    store
        .configure(AdmissionConfig {
            max_concurrent: 1,
            max_queued: 0,
            queue_timeout_secs: 600,
            ..Default::default()
        })
        .unwrap();
    assert!(matches!(
        store.acquire("one".into(), "new").await,
        Err(Rejection::QueueFull)
    ));
    tokio::time::advance(Duration::from_secs(30)).await;
    assert!(matches!(wait.await, Err(Rejection::TimedOut)));
    counts(&store, 1, 0);
    drop(first);
}

#[tokio::test]
async fn account_overrides_are_isolated_and_global_caps_are_bounded() {
    let store = store(1, 0);
    let key = "a".repeat(64);
    let mut config = AdmissionConfig {
        max_concurrent: 1,
        max_queued: 0,
        ..Default::default()
    };
    config.account_limits.insert(key.clone(), 2);
    store.configure(config).unwrap();
    let a = store.acquire(key.clone(), "a").await.unwrap();
    let b = store.acquire(key.clone(), "b").await.unwrap();
    assert!(matches!(
        store.acquire(key, "c").await,
        Err(Rejection::QueueFull)
    ));
    let other = store.acquire("other".into(), "other").await.unwrap();
    drop((a, b, other));
    let mut permits = Vec::new();
    for index in 0..MAX_LANES {
        permits.push(store.acquire(index.to_string(), "one").await.unwrap());
    }
    assert!(matches!(
        store.acquire("overflow".into(), "one").await,
        Err(Rejection::Capacity)
    ));
    drop(permits);
    counts(&store, 0, 0);
    store
        .configure(AdmissionConfig {
            max_queued: 0,
            ..Default::default()
        })
        .unwrap();
    let mut permits = Vec::new();
    for _ in 0..MAX_ACTIVE {
        permits.push(store.acquire("same".into(), "same").await.unwrap());
    }
    assert!(matches!(
        store.acquire("same".into(), "overflow").await,
        Err(Rejection::QueueFull)
    ));
    drop(permits);
    counts(&store, 0, 0);
}

#[tokio::test]
async fn total_waiting_capacity_cannot_be_bypassed_with_more_lanes() {
    let store = store(1, 256);
    let mut active = Vec::new();
    let mut waiters = Vec::new();
    for index in 0..16 {
        let key = index.to_string();
        active.push(store.acquire(key.clone(), "one").await.unwrap());
        for _ in 0..256 {
            let mut waiter = Box::pin(store.acquire(key.clone(), "wait"));
            assert!(poll!(waiter.as_mut()).is_pending());
            waiters.push(waiter);
        }
    }
    let extra = store.acquire("extra".into(), "extra").await.unwrap();
    assert!(matches!(
        store.acquire("extra".into(), "overflow").await,
        Err(Rejection::QueueFull)
    ));
    counts(&store, 17, MAX_WAITERS);
    drop(waiters);
    counts(&store, 17, 0);
    drop((active, extra));
    counts(&store, 0, 0);
}

#[tokio::test]
async fn streaming_owner_survives_headers_and_releases_on_drop_eof_and_error() {
    let store = store(1, 1);
    let pending = axum::body::Body::from_stream(futures_util::stream::pending::<
        Result<bytes::Bytes, std::io::Error>,
    >());
    let body = track_body(pending, store.acquire("one".into(), "one").await.unwrap());
    counts(&store, 1, 0);
    drop(body); // never polled
    counts(&store, 0, 0);
    let body = track_body(
        axum::body::Body::from("content"),
        store.acquire("one".into(), "one").await.unwrap(),
    );
    assert_eq!(axum::body::to_bytes(body, 100).await.unwrap(), "content");
    counts(&store, 0, 0);
    let error = futures_util::stream::once(async {
        Err::<bytes::Bytes, _>(std::io::Error::other("private error"))
    });
    let mut body = track_body(
        axum::body::Body::from_stream(error),
        store.acquire("one".into(), "one").await.unwrap(),
    )
    .into_data_stream();
    assert!(body.next().await.unwrap().is_err());
    drop(body);
    counts(&store, 0, 0);
}

#[test]
fn invalid_settings_leave_current_policy_intact_and_old_serialization_is_compatible() {
    let store = store(1, 4);
    let invalid = AdmissionConfig {
        max_concurrent: 1001,
        ..Default::default()
    };
    assert_eq!(store.configure(invalid), Err(Rejection::Unavailable));
    assert_eq!(store.0.lock().unwrap().config.max_concurrent, 1);
    for (field, value) in [
        ("maxConcurrent", 1001),
        ("maxQueued", 257),
        ("queueTimeoutSecs", 0),
        ("queueTimeoutSecs", 601),
    ] {
        let config: AdmissionConfig =
            serde_json::from_value(serde_json::json!({field: value})).unwrap();
        assert!(config.validate().is_err());
    }
    let mut config = AdmissionConfig::default();
    config.account_limits.insert("private-token".into(), 1);
    assert!(config.validate().is_err());
    let mut raw = serde_json::to_value(crate::proxy_optimizer::OptimizerConfig::default()).unwrap();
    raw.as_object_mut().unwrap().remove("admission");
    let old: crate::proxy_optimizer::OptimizerConfig = serde_json::from_value(raw).unwrap();
    assert_eq!(old.admission.max_concurrent, 0);
    assert_eq!(old.admission.queue_timeout_secs, 30);
}

#[tokio::test(start_paused = true)]
async fn recently_used_accounts_remain_configurable_but_the_catalog_expires_and_has_a_bound() {
    let store = store(0, 1);
    let name = format!("{}\n", "账户".repeat(100));
    let first = store.acquire("same".into(), &name).await.unwrap();
    let second = store.acquire("same".into(), &name).await.unwrap();
    let stats = store.stats().unwrap();
    assert_eq!(stats.entries[0].profiles.len(), 1);
    assert_eq!(stats.entries[0].profiles[0].chars().count(), 128);
    assert!(!stats.entries[0].profiles[0].contains('\n'));
    drop((first, second));
    counts(&store, 0, 0);
    assert_eq!(store.stats().unwrap().entries.len(), 1);
    for index in 0..MAX_RECENT + 2 {
        drop(store.acquire(index.to_string(), "account").await.unwrap());
    }
    assert_eq!(store.stats().unwrap().entries.len(), MAX_RECENT);
    assert_eq!(store.0.lock().unwrap().recent.len(), MAX_RECENT);
    tokio::time::advance(RECENT_TTL).await;
    assert!(store.stats().unwrap().entries.is_empty());
    assert!(store.0.lock().unwrap().recent.is_empty());
}

#[tokio::test(start_paused = true)]
async fn queued_request_bytes_are_bounded_and_released_on_grant_cancel_and_timeout() {
    let store = store(1, 4);
    let first = store.acquire("one".into(), "first").await.unwrap();
    let mut queued = Box::pin(store.acquire_with_bytes("one".into(), "queue", MAX_QUEUED_BYTES));
    assert!(poll!(queued.as_mut()).is_pending());
    assert_eq!(store.stats().unwrap().queued_bytes, MAX_QUEUED_BYTES);
    assert!(matches!(
        store.acquire_with_bytes("one".into(), "full", 1).await,
        Err(Rejection::QueueFull)
    ));
    drop(queued);
    assert_eq!(store.stats().unwrap().queued_bytes, 0);
    let mut queued = Box::pin(store.acquire_with_bytes("one".into(), "queue", MAX_QUEUED_BYTES));
    assert!(poll!(queued.as_mut()).is_pending());
    drop(first);
    let next = queued.await.unwrap();
    assert_eq!(store.stats().unwrap().queued_bytes, 0);
    let mut queued = Box::pin(store.acquire_with_bytes("one".into(), "expire", MAX_QUEUED_BYTES));
    assert!(poll!(queued.as_mut()).is_pending());
    tokio::time::advance(Duration::from_secs(30)).await;
    assert!(matches!(queued.await, Err(Rejection::TimedOut)));
    assert_eq!(store.stats().unwrap().queued_bytes, 0);
    drop(next);
    counts(&store, 0, 0);
}
