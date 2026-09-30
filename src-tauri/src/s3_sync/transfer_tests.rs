use super::*;

#[test]
fn manifest_rejects_unsafe_paths_sizes_and_non_hex_hashes() {
    let valid = S3Manifest {
        format: FORMAT.into(),
        protocol_version: PROTOCOL_VERSION,
        db_compat_version: DB_COMPAT_VERSION,
        snapshot_path: "snapshots/db.sql".into(),
        size_bytes: 3,
        sha256: sha256_hex(b"abc"),
        ..Default::default()
    };
    validate_manifest(&valid).unwrap();
    for path in [
        "../db.sql",
        "snapshots/../db.sql",
        "snapshots/%2e%2e/db.sql",
    ] {
        let mut invalid = valid.clone();
        invalid.snapshot_path = path.into();
        assert!(validate_manifest(&invalid).is_err());
    }
    let mut invalid = valid.clone();
    invalid.sha256 = "g".repeat(64);
    assert!(validate_manifest(&invalid).is_err());
    invalid = valid;
    invalid.size_bytes = MAX_SYNC_BYTES as u64 + 1;
    assert!(validate_manifest(&invalid).is_err());
}
