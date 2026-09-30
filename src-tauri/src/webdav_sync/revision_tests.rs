use super::*;
use crate::cloud_credentials::tests::MemoryStore;
use crate::cloud_revision::tests::{fixture, reply};

#[tokio::test]
async fn dav_snapshot_creation_is_immutable_and_manifest_races_stop_replacement() {
    let manifest = WebDavManifest {
        format: WEBDAV_FORMAT.into(),
        protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
        db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
        snapshot_path: "snapshots/old.sql".into(),
        size_bytes: 3,
        ..Default::default()
    };
    let body = serde_json::to_vec(&manifest).unwrap();
    let (url, worker) = fixture(vec![
        reply(200, Some("\"v1\""), &body),
        reply(201, Some("\"snapshot\""), b""),
        reply(412, None, b""),
    ]);
    let settings = WebDavSyncSettings {
        enabled: true,
        base_url: url,
        username: "public-user".into(),
        password: "public-password".into(),
        ..Default::default()
    };
    // The wire fixture must not depend on this machine's system proxy settings.
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let (_, revision) = fetch_manifest_for_layout(&client, &settings, WebDavRemoteLayout::Current)
        .await
        .unwrap()
        .unwrap();
    let store = MemoryStore::default();
    let scope = backup_scope(&settings);
    assert!(cloud_revision::authorize(&store, &scope, Some(&revision), None).is_err());
    let reviewed = cloud_revision::review(&store, &scope, Some(&revision)).unwrap();
    let condition =
        cloud_revision::authorize(&store, &scope, Some(&revision), Some(&reviewed.revision))
            .unwrap();
    upload_bytes(
        &client,
        &settings,
        &remote_file_url(
            &settings,
            WebDavRemoteLayout::Current,
            "snapshots/new.cchub-backup",
        )
        .unwrap(),
        "application/octet-stream",
        b"encrypted-fixture".to_vec(),
        &WriteCondition::Absent,
    )
    .await
    .unwrap();
    let error = upload_bytes(
        &client,
        &settings,
        &manifest_url_for_layout(&settings, WebDavRemoteLayout::Current).unwrap(),
        "application/json",
        body,
        &condition,
    )
    .await
    .unwrap_err();
    assert_eq!(error, cloud_revision::CONFLICT);
    assert!(
        cloud_revision::review(&store, &scope, Some(&revision))
            .unwrap()
            .requires_confirmation
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("GET "));
    assert!(requests[1]
        .to_ascii_lowercase()
        .contains("if-none-match: *"));
    assert!(requests[2]
        .to_ascii_lowercase()
        .contains("if-match: \"v1\""));
}
