use super::*;
use crate::cloud_credentials::tests::MemoryStore;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

/// Local HTTP evidence for both authenticated transports; never contacts a real account.
pub(crate) fn fixture(responses: Vec<Vec<u8>>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        let mut captured = Vec::new();
        for response in responses {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let (header_end, length) = loop {
                let mut chunk = [0; 2048];
                let count = stream.read(&mut chunk).unwrap();
                assert_ne!(count, 0, "request ended before headers");
                bytes.extend_from_slice(&chunk[..count]);
                assert!(bytes.len() <= 1024 * 1024);
                if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    break (end + 4, length);
                }
            };
            while bytes.len() < header_end + length {
                let mut chunk = [0; 2048];
                let count = stream.read(&mut chunk).unwrap();
                assert_ne!(count, 0, "request ended before body");
                bytes.extend_from_slice(&chunk[..count]);
            }
            captured.push(String::from_utf8(bytes).unwrap());
            stream.write_all(&response).unwrap();
        }
        captured
    });
    (format!("http://{address}"), worker)
}

pub(crate) fn reply(status: u16, etag: Option<&str>, bytes: &[u8]) -> Vec<u8> {
    let etag = etag
        .map(|value| format!("ETag: {value}\r\n"))
        .unwrap_or_default();
    let mut response = format!(
        "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n{etag}Connection: close\r\n\r\n",
        bytes.len()
    )
    .into_bytes();
    response.extend_from_slice(bytes);
    response
}

fn headers(etag: &str) -> HeaderMap {
    let mut result = HeaderMap::new();
    result.insert(ETAG, HeaderValue::from_str(etag).unwrap());
    result
}

#[test]
fn viewing_does_not_accept_a_revision_or_allow_automatic_overwrite() {
    let store = MemoryStore::default();
    let observed = observe("location", b"one", &headers("\"v1\""));
    for _ in 0..2 {
        let review = review(&store, "location", Some(&observed)).unwrap();
        assert!(review.requires_confirmation && review.conditional_supported);
        assert!(authorize(&store, "location", Some(&observed), None).is_err());
    }
    assert_eq!(store.get(&account("location")).unwrap(), None);
    assert!(matches!(
        authorize(&store, "location", Some(&observed), Some(&observed.token)).unwrap(),
        WriteCondition::Matches(_)
    ));
    assert_eq!(store.get(&account("location")).unwrap(), None);
    accept(&store, "location", &observed).unwrap();
    assert!(
        !review(&store, "location", Some(&observed))
            .unwrap()
            .requires_confirmation
    );
    assert!(authorize(&store, "location", Some(&observed), None).is_ok());
}

#[test]
fn reviewed_and_accepted_revisions_cannot_follow_another_remote_update() {
    let store = MemoryStore::default();
    let original = observe("location", b"one", &headers("\"v1\""));
    accept(&store, "location", &original).unwrap();
    for newer in [
        observe("location", b"two", &headers("\"v1\"")),
        observe("location", b"one", &headers("\"v2\"")),
    ] {
        assert_eq!(
            authorize(&store, "location", Some(&newer), None).unwrap_err(),
            CONFLICT
        );
        assert_eq!(
            authorize(&store, "location", Some(&newer), Some(&original.token)).unwrap_err(),
            CONFLICT
        );
    }
    let another = observe("other", b"one", &headers("\"v1\""));
    assert_eq!(
        authorize(&store, "other", Some(&another), None).unwrap_err(),
        CONFLICT
    );
    assert_eq!(
        authorize(&store, "other", Some(&another), Some(&original.token)).unwrap_err(),
        CONFLICT
    );
}

#[test]
fn missing_objects_can_be_created_but_deleted_backups_require_review() {
    let store = MemoryStore::default();
    assert!(matches!(
        authorize(&store, "location", None, None).unwrap(),
        WriteCondition::Absent
    ));
    let missing = review(&store, "location", None).unwrap().revision;
    assert!(matches!(
        authorize(&store, "location", None, Some(&missing)).unwrap(),
        WriteCondition::Absent
    ));
    let present = observe("location", b"one", &headers("\"v1\""));
    assert_eq!(
        authorize(&store, "location", Some(&present), Some(&missing)).unwrap_err(),
        CONFLICT
    );
    accept(&store, "location", &present).unwrap();
    assert!(
        review(&store, "location", None)
            .unwrap()
            .requires_confirmation
    );
    assert_eq!(
        authorize(&store, "location", None, None).unwrap_err(),
        CONFLICT
    );
    assert!(authorize(&store, "location", None, Some(&missing)).is_ok());
}

#[test]
fn weak_ambiguous_and_invalid_etags_never_allow_replacement() {
    let store = MemoryStore::default();
    for map in [
        HeaderMap::new(),
        headers("W/\"weak\""),
        headers("bare"),
        headers("\"one\", \"two\""),
        headers("\"a b\""),
    ] {
        let observed = observe("location", b"one", &map);
        assert!(
            !review(&store, "location", Some(&observed))
                .unwrap()
                .conditional_supported
        );
        assert!(authorize(&store, "location", Some(&observed), Some(&observed.token)).is_err());
    }
    let mut duplicate = headers("\"v1\"");
    duplicate.append(ETAG, HeaderValue::from_static("\"v2\""));
    assert!(strong_etag(&duplicate).is_none());
    assert!(valid_etag("\"\""));
    assert!(valid_etag("\"abc-12\""));
    assert!(!valid_etag(&format!("\"{}\"", "a".repeat(1023))));
}

#[test]
fn post_upload_verification_never_accepts_another_writers_version() {
    let correct = observe("location", b"one", &headers("\"v1\""));
    assert!(verify_written(b"one", Some(correct)).is_ok());
    assert!(verify_written(b"one", None).is_err());
    assert!(verify_written(
        b"one",
        Some(observe("location", b"two", &headers("\"v2\"")))
    )
    .is_err());
    assert!(verify_written(b"one", Some(observe("location", b"one", &HeaderMap::new()))).is_err());
}

async fn server(status: u16) -> (RequestBuilder, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buffer = [0; 4096];
        let count = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8(buffer[..count].to_vec()).unwrap();
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nETag: \"new\"\r\nConnection: close\r\n\r\n").unwrap();
        request.to_ascii_lowercase()
    });
    let request = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
        .put(format!("http://{address}/manifest"));
    (request, worker)
}

#[tokio::test]
async fn sends_atomic_creation_and_replacement_conditions() {
    for (condition, expected, absent) in [
        (WriteCondition::Absent, "if-none-match: *", "if-match:"),
        (
            WriteCondition::Matches(HeaderValue::from_static("\"v1\"")),
            "if-match: \"v1\"",
            "if-none-match:",
        ),
    ] {
        let (request, worker) = server(201).await;
        assert_eq!(
            strong_etag(&send(request, &condition).await.unwrap()).as_deref(),
            Some("\"new\"")
        );
        let wire = worker.join().unwrap();
        assert!(wire.contains(expected));
        assert!(!wire.contains(absent));
    }
}

#[tokio::test]
async fn conflicts_and_unsupported_servers_fail_without_unconditional_retry() {
    for status in [409, 412, 428, 400, 405, 501, 403] {
        let (request, worker) = server(status).await;
        let error = send(request, &WriteCondition::Absent).await.unwrap_err();
        if [409, 412, 428].contains(&status) {
            assert_eq!(error, CONFLICT);
        } else {
            assert!(error.contains(&status.to_string()));
        }
        assert!(worker.join().unwrap().contains("if-none-match: *"));
    }
}
