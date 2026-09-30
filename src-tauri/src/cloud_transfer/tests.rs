use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

async fn response(body: &[u8]) -> (reqwest::Response, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let bytes = body.to_vec();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = [0; 4096];
        stream.read(&mut request).unwrap();
        let _ = stream.write_all(&bytes);
    });
    let response = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
        .get(format!("http://{address}/backup"))
        .send()
        .await
        .unwrap();
    (response, worker)
}

#[tokio::test]
async fn accepts_exact_limit_and_responses_without_content_length() {
    for wire in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc".as_slice(),
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nabc".as_slice(),
    ] {
        let (response, worker) = response(wire).await;
        assert_eq!(read_bounded(response, 3).await.unwrap(), b"abc");
        worker.join().unwrap();
    }
}

#[tokio::test]
async fn rejects_declared_oversize_before_reading_body() {
    let (response, worker) =
        response(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000000\r\nConnection: close\r\n\r\n")
            .await;
    assert!(read_bounded(response, 4)
        .await
        .unwrap_err()
        .contains("超过"));
    worker.join().unwrap();
}

#[tokio::test]
async fn rejects_chunked_or_close_delimited_overflow() {
    for wire in [b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n".as_slice(), b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nabcdef".as_slice()] {
        let (response, worker) = response(wire).await;
        assert!(read_bounded(response, 5).await.unwrap_err().contains("超过"));
        worker.join().unwrap();
    }
}

#[tokio::test]
async fn interrupted_transfer_returns_no_partial_backup_and_no_response_content() {
    let (response, worker) =
        response(b"HTTP/1.1 200 OK\r\nContent-Length: 30\r\nConnection: close\r\n\r\nsecret-token")
            .await;
    let error = read_bounded(response, 30).await.unwrap_err();
    assert!(error.contains("未修改本地数据"));
    assert!(!error.contains("secret"));
    worker.join().unwrap();
}

#[test]
fn snapshot_paths_stay_in_the_snapshot_directory() {
    assert!(validate_snapshot_path("snapshots/cchub-sync-20260930-120000.sql").is_ok());
    for path in [
        "",
        "../backup.sql",
        "snapshots/../backup.sql",
        "/snapshots/db.sql",
        "snapshots//db.sql",
        "snapshots/%2e%2e",
        "snapshots/..",
        "snapshots/db.sql?token=secret",
        "snapshots/db.sql#x",
        "snapshots/a\\b",
        "snapshots/https:other",
        "snapshots/db\n.sql",
    ] {
        let error = validate_snapshot_path(path).unwrap_err();
        assert!(!error.contains(path) || path.is_empty());
    }
}

#[test]
fn size_and_digest_validation_precedes_restore() {
    let digest = sha256(b"abc");
    assert!(verify_snapshot(b"abc", 3, Some(&digest)).is_ok());
    assert!(verify_snapshot(b"abc", 3, Some(&digest.to_uppercase())).is_ok());
    assert!(verify_snapshot(b"abc", 3, None).is_ok()); // Older WebDAV snapshots.
    assert!(verify_snapshot(b"abc", 4, None).is_err());
    assert!(verify_snapshot(b"abd", 3, Some(&digest)).is_err());
    assert!(verify_snapshot(b"", 0, None).is_err());
    assert!(validate_size_and_digest(SNAPSHOT_LIMIT as u64 + 1, None).is_err());
    assert!(validate_size_and_digest(3, Some(&"z".repeat(64))).is_err());
    assert!(validate_size_and_digest(3, Some("")).is_err());
}
