use super::*;

fn manifest() -> WebDavManifest {
    WebDavManifest {
        format: WEBDAV_FORMAT.into(),
        protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
        db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
        snapshot_path: "snapshots/cchub-sync-old.sql".into(),
        size_bytes: 3,
        ..Default::default()
    }
}

#[test]
fn old_webdav_manifests_remain_compatible_and_new_hashes_are_validated() {
    let old = manifest();
    let encoded = serde_json::to_value(&old).unwrap();
    let mut old_json = encoded.as_object().unwrap().clone();
    old_json.remove("sha256");
    let decoded: WebDavManifest = serde_json::from_value(old_json.into()).unwrap();
    validate_manifest_compatibility(&decoded, WebDavRemoteLayout::Current).unwrap();
    validate_manifest_compatibility(&decoded, WebDavRemoteLayout::Legacy).unwrap();
    let mut new = old;
    new.sha256 = crate::cloud_transfer::sha256(b"abc");
    validate_manifest_compatibility(&new, WebDavRemoteLayout::Current).unwrap();
    new.sha256 = "g".repeat(64);
    assert!(validate_manifest_compatibility(&new, WebDavRemoteLayout::Current).is_err());
}

#[test]
fn encrypted_manifest_format_is_supported_but_unknown_formats_are_not() {
    let mut sealed = manifest();
    sealed.snapshot_path = "snapshots/new.cchub-backup".into();
    sealed.payload_format = crate::cloud_backup::PAYLOAD_FORMAT.into();
    validate_manifest_compatibility(&sealed, WebDavRemoteLayout::Current).unwrap();
    assert!(crate::cloud_backup::encrypted(
        &sealed.payload_format,
        &sealed.snapshot_path
    ));
    sealed.payload_format = "future-format".into();
    assert!(validate_manifest_compatibility(&sealed, WebDavRemoteLayout::Current).is_err());
}

#[test]
fn remote_info_marks_unsafe_paths_or_sizes_incompatible() {
    for path in [
        "../db.sql",
        "snapshots/../db.sql",
        "snapshots/%2e%2e/db.sql",
    ] {
        let mut invalid = manifest();
        invalid.snapshot_path = path.into();
        assert!(validate_manifest_compatibility(&invalid, WebDavRemoteLayout::Current).is_err());
        assert!(validate_manifest_compatibility(&invalid, WebDavRemoteLayout::Legacy).is_err());
    }
    let mut invalid = manifest();
    invalid.size_bytes = MAX_WEBDAV_SYNC_BYTES as u64 + 1;
    assert!(validate_manifest_compatibility(&invalid, WebDavRemoteLayout::Current).is_err());
}
