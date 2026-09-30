use super::*;
use crate::cloud_credentials::tests::MemoryStore;

fn connection() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .unwrap();
    conn
}

fn configured() -> S3SyncSettings {
    S3SyncSettings {
        enabled: true,
        endpoint: "https://s3.test/storage".into(),
        bucket: "backup".into(),
        access_key_id: "alice".into(),
        secret_access_key: " original secret ".into(),
        backup_encryption: serde_json::from_value(
            serde_json::json!({"passphrase": "public-test-passphrase"}),
        )
        .unwrap(),
        ..Default::default()
    }
}

#[test]
fn blank_secret_cannot_follow_changed_endpoint_or_key_id() {
    let conn = connection();
    let store = MemoryStore::default();
    let original = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    for change_account in [false, true] {
        let mut changed = original.clone();
        if change_account {
            changed.access_key_id = "bob".into();
        } else {
            changed.endpoint = "https://other.test".into();
        }
        assert!(write_settings_with_store(&conn, changed.clone(), false, &store).is_err());
        let existing = read_settings_with_store(&conn, &store).unwrap();
        assert!(prepare_connection(&mut changed, Some(&existing), true).is_err());
        assert!(changed.secret_access_key.is_empty());
        assert_eq!(existing.secret_access_key, " original secret ");
    }
    let existing = read_settings_with_store(&conn, &store).unwrap();
    let mut same = original.clone();
    prepare_connection(&mut same, Some(&existing), true).unwrap();
    assert_eq!(same.secret_access_key, " original secret ");
    assert!(prepare_connection(&mut original.clone(), Some(&existing), false).is_err());
}

#[test]
fn scoped_secrets_are_independent_and_unmasked_only_in_backend() {
    let conn = connection();
    let store = MemoryStore::default();
    let first = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    let mut second = first.clone();
    second.access_key_id = "bob".into();
    second.secret_access_key = "second".into();
    let second = write_settings_with_store(&conn, second, true, &store).unwrap();
    assert!(serde_json::to_string(&second)
        .unwrap()
        .find("second")
        .is_none());
    let mut cleared = second.clone();
    cleared.enabled = false;
    write_settings_with_store(&conn, cleared, true, &store).unwrap();
    assert_eq!(store.get(&credential_scope(&second)).unwrap(), None);
    write_settings_with_store(&conn, first, false, &store).unwrap();
    assert_eq!(
        read_settings_with_store(&conn, &store)
            .unwrap()
            .secret_access_key,
        " original secret "
    );
}

#[test]
fn legacy_keyring_and_plaintext_migrate_without_persisting_secrets() {
    for plaintext in [true, false] {
        let conn = connection();
        let store = MemoryStore::default();
        let mut json = serde_json::to_value(configured()).unwrap();
        if plaintext {
            json["secretAccessKey"] = "legacy".into();
        } else {
            store.set(KEYRING_ACCOUNT, "legacy").unwrap();
        }
        set_json_app_setting(&conn, SETTINGS_KEY, &json).unwrap();
        let loaded = read_settings_with_store(&conn, &store).unwrap();
        assert_eq!(loaded.secret_access_key, "legacy");
        assert_eq!(store.get(KEYRING_ACCOUNT).unwrap(), None);
        let raw: serde_json::Value = get_json_app_setting(&conn, SETTINGS_KEY).unwrap().unwrap();
        assert!(raw.get("secretAccessKey").is_none());
        assert_eq!(raw["credentialScope"], credential_scope(&loaded));
        let mut other = loaded.masked_for_frontend();
        other.endpoint = "https://new.test".into();
        other.enabled = false;
        let saved = write_settings_with_store(&conn, other, false, &store).unwrap();
        assert!(!saved.has_secret_access_key);
    }
}

#[test]
fn persistence_failure_rolls_back_secret_and_keeps_original_settings() {
    let conn = connection();
    let store = MemoryStore::default();
    let mut settings = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_settings BEFORE INSERT ON app_settings BEGIN SELECT RAISE(ABORT, 'failed'); END").unwrap();
    settings.secret_access_key = "replacement".into();
    settings.backup_encryption = serde_json::from_value(serde_json::json!({
        "passphrase": "replacement-backup-password", "passphraseTouched": true
    }))
    .unwrap();
    assert!(write_settings_with_store(&conn, settings, true, &store).is_err());
    assert_eq!(
        read_settings_with_store(&conn, &store)
            .unwrap()
            .secret_access_key,
        " original secret "
    );
    assert_eq!(
        read_settings_with_store(&conn, &store)
            .unwrap()
            .backup_encryption
            .passphrase
            .as_str(),
        "public-test-passphrase"
    );
}

#[test]
fn backup_password_is_scoped_to_endpoint_account_bucket_root_and_profile() {
    let conn = connection();
    let store = MemoryStore::default();
    let original = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    assert!(original.backup_encryption.has_passphrase);
    assert!(original.backup_encryption.passphrase.is_empty());
    let raw: serde_json::Value = get_json_app_setting(&conn, SETTINGS_KEY).unwrap().unwrap();
    assert!(raw["backupEncryption"].get("passphrase").is_none());
    assert!(raw["backupEncryption"].get("passphraseTouched").is_none());
    for field in ["endpoint", "account", "bucket", "root", "profile"] {
        let mut other = original.clone();
        other.enabled = false;
        match field {
            "endpoint" => other.endpoint = "https://other.test".into(),
            "account" => other.access_key_id = "bob".into(),
            "bucket" => other.bucket = "other".into(),
            "root" => other.remote_root = "other".into(),
            _ => other.profile = "other".into(),
        }
        let saved = write_settings_with_store(&conn, other, false, &store).unwrap();
        assert!(!saved.backup_encryption.has_passphrase, "{field}");
        assert!(read_settings_with_store(&conn, &store)
            .unwrap()
            .backup_encryption
            .passphrase
            .is_empty());
        write_settings_with_store(&conn, original.clone(), false, &store).unwrap();
        assert_eq!(
            read_settings_with_store(&conn, &store)
                .unwrap()
                .backup_encryption
                .passphrase
                .as_str(),
            "public-test-passphrase"
        );
    }
}

#[test]
fn clearing_backup_password_requires_disabling_auto_upload_and_keeps_login() {
    let conn = connection();
    let store = MemoryStore::default();
    let mut initial = configured();
    initial.auto_sync = true;
    let saved = write_settings_with_store(&conn, initial, true, &store).unwrap();
    let mut cleared = saved.clone();
    cleared.backup_encryption.passphrase_touched = true;
    assert!(write_settings_with_store(&conn, cleared.clone(), false, &store).is_err());
    assert_eq!(
        read_settings_with_store(&conn, &store)
            .unwrap()
            .backup_encryption
            .passphrase
            .as_str(),
        "public-test-passphrase"
    );
    cleared.auto_sync = false;
    let saved = write_settings_with_store(&conn, cleared, false, &store).unwrap();
    assert!(!saved.backup_encryption.has_passphrase);
    let loaded = read_settings_with_store(&conn, &store).unwrap();
    assert!(loaded.backup_encryption.passphrase.is_empty());
    assert_eq!(loaded.secret_access_key, " original secret ");
}

#[test]
fn bucket_changes_clear_history_but_preserve_account_and_endpoint_base_path() {
    let conn = connection();
    let store = MemoryStore::default();
    let mut stored = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    stored.last_sync_at = Some("old".into());
    stored.last_error = Some("old error".into());
    set_json_app_setting(&conn, SETTINGS_KEY, &stored).unwrap();
    stored.bucket = "new-bucket".into();
    let saved = write_settings_with_store(&conn, stored, false, &store).unwrap();
    assert!(saved.last_sync_at.is_none() && saved.last_error.is_none());
    assert!(saved.has_secret_access_key);
    assert_eq!(
        object_url(&saved, "manifest.json").unwrap().as_str(),
        "https://s3.test/storage/new-bucket/manifest.json"
    );
}

#[test]
fn completed_upload_never_replaces_concurrently_edited_settings() {
    let conn = connection();
    let store = MemoryStore::default();
    let original = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    let mut edited = original.clone();
    edited.auto_sync = true;
    edited.enabled = false;
    write_settings_with_store(&conn, edited, false, &store).unwrap();
    update_upload_status(&conn, &original, "completed".into()).unwrap();
    let loaded = read_settings_with_store(&conn, &store).unwrap();
    assert!(!loaded.enabled && loaded.auto_sync);
    assert_eq!(loaded.last_sync_at.as_deref(), Some("completed"));
    let mut other = loaded;
    other.bucket = "new-bucket".into();
    write_settings_with_store(&conn, other, false, &store).unwrap();
    update_upload_status(&conn, &original, "late-completion".into()).unwrap();
    let loaded = read_settings_with_store(&conn, &store).unwrap();
    assert_eq!(loaded.bucket, "new-bucket");
    assert!(loaded.last_sync_at.is_none());
}
