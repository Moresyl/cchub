use super::transport::*;
use super::S3SyncSettings;
use crate::cloud_credentials::tests::MemoryStore;
use crate::cloud_revision::tests::{fixture, reply};
use crate::cloud_revision::{self, WriteCondition};
use reqwest::header::{HeaderValue, AUTHORIZATION};

fn sign(key: &[u8], bytes: &[u8]) -> Vec<u8> {
    ring::hmac::sign(&ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key), bytes)
        .as_ref()
        .to_vec()
}

#[tokio::test]
async fn s3_snapshot_creation_is_immutable_and_manifest_races_stop_replacement() {
    let manifest = super::S3Manifest {
        format: super::FORMAT.into(),
        protocol_version: super::PROTOCOL_VERSION,
        db_compat_version: super::DB_COMPAT_VERSION,
        snapshot_path: "snapshots/old.sql".into(),
        size_bytes: 3,
        sha256: sha256_hex(b"abc"),
        ..Default::default()
    };
    let body = serde_json::to_vec(&manifest).unwrap();
    let (url, worker) = fixture(vec![
        reply(200, Some("\"v1\""), &body),
        reply(201, Some("\"snapshot\""), b""),
        reply(412, None, b""),
    ]);
    let settings = S3SyncSettings {
        enabled: true,
        endpoint: url,
        bucket: "backup".into(),
        access_key_id: "public-id".into(),
        secret_access_key: "public-secret".into(),
        ..Default::default()
    };
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let (_, revision) = super::fetch_manifest_with_client(&client, &settings)
        .await
        .unwrap()
        .unwrap();
    let store = MemoryStore::default();
    let scope = super::backup_scope(&settings);
    assert!(cloud_revision::authorize(&store, &scope, Some(&revision), None).is_err());
    let reviewed = cloud_revision::review(&store, &scope, Some(&revision)).unwrap();
    let condition =
        cloud_revision::authorize(&store, &scope, Some(&revision), Some(&reviewed.revision))
            .unwrap();
    put_object_using(
        &client,
        &settings,
        &object_key(&settings, "snapshots/new.cchub-backup"),
        b"encrypted-fixture".to_vec(),
        &WriteCondition::Absent,
    )
    .await
    .unwrap();
    assert_eq!(
        put_object_using(
            &client,
            &settings,
            &object_key(&settings, "manifest.json"),
            body,
            &condition
        )
        .await
        .unwrap_err(),
        cloud_revision::CONFLICT
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("GET "));
    for (wire, header) in [(&requests[1], "if-none-match"), (&requests[2], "if-match")] {
        let header_lines = wire.split("\r\n\r\n").next().unwrap().to_ascii_lowercase();
        assert_eq!(
            header_lines
                .lines()
                .filter(|line| line.starts_with(&format!("{header}:")))
                .count(),
            1
        );
        assert!(header_lines.contains(&format!(
            "signedheaders=host;{header};x-amz-content-sha256;x-amz-date"
        )));
    }
}

#[test]
fn conditional_headers_are_signed_once_and_match_an_independent_sigv4_calculation() {
    let settings = S3SyncSettings {
        endpoint: "https://s3.test/storage".into(),
        bucket: "backup".into(),
        access_key_id: "public-test-id".into(),
        secret_access_key: "public-test-secret".into(),
        ..Default::default()
    };
    for condition in [
        WriteCondition::Absent,
        WriteCondition::Matches(HeaderValue::from_static("\"v1\"")),
    ] {
        let request = signed_request(
            &reqwest::Client::new(),
            &settings,
            reqwest::Method::PUT,
            "manifest.json",
            b"payload".to_vec(),
            Some(&condition),
        )
        .unwrap();
        let request = cloud_revision::apply(request, &condition).build().unwrap();
        let headers = request.headers();
        let (name, value) = match condition {
            WriteCondition::Absent => ("if-none-match", "*"),
            WriteCondition::Matches(_) => ("if-match", "\"v1\""),
        };
        assert_eq!(headers.get_all(name).iter().count(), 1);
        assert_eq!(headers[name], value);
        let date = headers["x-amz-date"].to_str().unwrap();
        let day = &date[..8];
        let hash = sha256_hex(b"payload");
        let signed = format!("host;{name};x-amz-content-sha256;x-amz-date");
        let canonical = format!("PUT\n/storage/backup/manifest.json\n\nhost:s3.test\n{name}:{value}\nx-amz-content-sha256:{hash}\nx-amz-date:{date}\n\n{signed}\n{hash}");
        let scope = format!("{day}/us-east-1/s3/aws4_request");
        let to_sign = format!(
            "AWS4-HMAC-SHA256\n{date}\n{scope}\n{}",
            sha256_hex(canonical.as_bytes())
        );
        let key = sign(b"AWS4public-test-secret", day.as_bytes());
        let key = sign(&key, b"us-east-1");
        let key = sign(&key, b"s3");
        let key = sign(&key, b"aws4_request");
        let expected = format!("AWS4-HMAC-SHA256 Credential=public-test-id/{scope}, SignedHeaders={signed}, Signature={}", hex(&sign(&key, to_sign.as_bytes())));
        assert_eq!(headers[AUTHORIZATION], expected);
    }
}
