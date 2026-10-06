use super::*;

#[test]
fn draining_waits_for_queued_records_and_reports_incomplete_io() {
    let writer = Writer::start().unwrap();
    let (started, ready) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let record = writer
        .submit(move || {
            started.send(()).unwrap();
            blocked.recv_timeout(Duration::from_secs(2)).unwrap();
            Ok(())
        })
        .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        writer.drain(Duration::from_millis(10)).unwrap_err(),
        "Proxy accounting is still being saved"
    );
    release.send(()).unwrap();
    writer.drain(Duration::from_secs(2)).unwrap();
    assert!(record.blocking_recv().unwrap().is_ok());
}

#[test]
fn records_survive_async_runtime_teardown_in_fifo_order() {
    let writer = Arc::new(Writer::start().unwrap());
    let (started, ready) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let output = Arc::new(Mutex::new(Vec::new()));
    let first_output = output.clone();
    let first = writer
        .submit(move || {
            started.send(()).unwrap();
            blocked.recv_timeout(Duration::from_secs(2)).unwrap();
            first_output.lock().unwrap().push(1);
            Ok(())
        })
        .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let second_output = output.clone();
    runtime.block_on(async {
        drop(
            writer
                .submit(move || {
                    second_output.lock().unwrap().push(2);
                    Ok(())
                })
                .unwrap(),
        );
    });
    drop(runtime);
    drop(first);
    release.send(()).unwrap();
    writer.drain(Duration::from_secs(2)).unwrap();
    assert_eq!(*output.lock().unwrap(), vec![1, 2]);
}

#[test]
fn a_panicking_record_releases_pending_count_and_keeps_the_writer_alive() {
    let writer = Writer::start().unwrap();
    let failed = writer
        .submit::<()>(|| panic!("private record detail"))
        .unwrap();
    let next = writer.submit(|| Ok(7)).unwrap();
    assert_eq!(
        failed.blocking_recv().unwrap().unwrap_err(),
        "Proxy accounting worker could not complete"
    );
    assert_eq!(next.blocking_recv().unwrap().unwrap(), 7);
    writer.drain(Duration::from_secs(2)).unwrap();
}
