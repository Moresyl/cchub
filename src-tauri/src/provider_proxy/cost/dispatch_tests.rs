use super::*;
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[tokio::test(flavor = "current_thread")]
async fn blocking_accounting_does_not_stop_the_runtime() {
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, blocked) = mpsc::channel();
    let task = tokio::spawn(write(&reserve().await.unwrap(), move || {
        started.send(()).unwrap();
        blocked.recv_timeout(Duration::from_secs(2)).unwrap();
        Ok(())
    }));
    ready.await.unwrap();
    let heartbeat = tokio::time::timeout(Duration::from_millis(100), async {
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;
    })
    .await;
    release.send(()).unwrap();
    task.await.unwrap().unwrap();
    assert!(heartbeat.is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_waiter_does_not_release_a_running_write() {
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, blocked) = mpsc::channel();
    let sequence = Arc::new(std::sync::Mutex::new(Vec::new()));
    let first_sequence = sequence.clone();
    let first = tokio::spawn(write(&reserve().await.unwrap(), move || {
        started.send(()).unwrap();
        blocked.recv_timeout(Duration::from_secs(2)).unwrap();
        first_sequence.lock().unwrap().push(1);
        Ok(())
    }));
    ready.await.unwrap();
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    assert!(WRITES.try_lock().is_err());
    let second_sequence = sequence.clone();
    let second = tokio::spawn(write(&reserve().await.unwrap(), move || {
        second_sequence.lock().unwrap().push(2);
        Ok(())
    }));
    tokio::task::yield_now().await;
    release.send(()).unwrap();
    second.await.unwrap().unwrap();
    assert_eq!(*sequence.lock().unwrap(), vec![1, 2]);
}

#[tokio::test(flavor = "current_thread")]
async fn worker_failure_is_safe_and_does_not_poison_following_writes() {
    let lease = reserve().await.unwrap();
    let failure = write::<()>(&lease, || panic!("private worker details")).await;
    assert_eq!(
        failure.unwrap_err(),
        "Proxy accounting worker could not complete"
    );
    assert_eq!(write(&lease, || Ok(7)).await.unwrap(), 7);
    assert_eq!(
        write::<()>(&lease, || Err("write rejected".into()))
            .await
            .unwrap_err(),
        "write rejected"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_a_queued_future_still_finishes_the_record() {
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, blocked) = mpsc::channel();
    let first = write(&reserve().await.unwrap(), move || {
        started.send(()).unwrap();
        blocked.recv_timeout(Duration::from_secs(2)).unwrap();
        Ok(())
    });
    ready.await.unwrap();
    let (stored, completed) = tokio::sync::oneshot::channel();
    let queued_lease = reserve().await.unwrap();
    let retained = Arc::downgrade(&queued_lease._permit);
    let second = write(&queued_lease, move || {
        stored.send(()).unwrap();
        Ok(())
    });
    // This future was never polled: ownership must already belong to the writer.
    drop(second);
    drop(queued_lease);
    assert!(retained.upgrade().is_some());
    release.send(()).unwrap();
    first.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), completed)
        .await
        .unwrap()
        .unwrap();
    // A following write observes release of the earlier writer and its lease.
    write(&reserve().await.unwrap(), || Ok(())).await.unwrap();
    assert!(retained.upgrade().is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn request_capacity_is_held_until_a_detached_write_finishes() {
    static CAPACITY: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let lease = reserve_from(&CAPACITY).await.unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, blocked) = mpsc::channel();
    let writing = write(&lease, move || {
        started.send(()).unwrap();
        blocked.recv_timeout(Duration::from_secs(2)).unwrap();
        Ok(())
    });
    drop(lease);
    ready.await.unwrap();
    assert_eq!(CAPACITY.available_permits(), 0);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), reserve_from(&CAPACITY))
            .await
            .is_err()
    );
    release.send(()).unwrap();
    writing.await.unwrap();
    assert_eq!(CAPACITY.available_permits(), 1);
    assert!(reserve_from(&CAPACITY).await.is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn unavailable_accounting_capacity_reports_a_safe_error() {
    static CLOSED: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(0);
    CLOSED.close();
    assert!(
        matches!(reserve_from(&CLOSED).await, Err(error) if error == "Proxy accounting is unavailable")
    );
}
