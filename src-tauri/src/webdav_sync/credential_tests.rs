use super::*;
use crate::cloud_credentials::tests::MemoryStore;

fn connection() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        .unwrap();
    conn
}

fn configured() -> WebDavSyncSettings {
    WebDavSyncSettings {
        enabled: true,
        base_url: "https://dav.test/root".into(),
        username: "alice".into(),
        password: " original secret ".into(),
        backup_encryption: serde_json::from_value(
            serde_json::json!({"passphrase": "public-test-passphrase"}),
        )
        .unwrap(),
        ..Default::default()
    }
}

#[test]
fn blank_password_cannot_follow_changed_server_or_account() {
    let conn = connection();
    let store = MemoryStore::default();
    let original = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    for change_account in [false, true] {
        let mut changed = original.clone();
        if change_account {
            changed.username = "bob".into();
        } else {
            changed.base_url = "https://other.test".into();
        }
        assert!(write_settings_with_store(&conn, changed.clone(), false, &store).is_err());
        let existing = read_settings_with_store(&conn, &store).unwrap();
        assert!(prepare_connection(&mut changed, Some(&existing), true).is_err());
        assert!(changed.password.is_empty());
        assert_eq!(existing.password, " original secret ");
    }
    let mut test = original.clone();
    let existing = read_settings_with_store(&conn, &store).unwrap();
    prepare_connection(&mut test, Some(&existing), true).unwrap();
    assert_eq!(test.password, " original secret ");
    assert!(prepare_connection(&mut original.clone(), Some(&existing), false).is_err());
}

#[test]
fn separate_entries_survive_switching_and_clearing_one_account() {
    let conn = connection();
    let store = MemoryStore::default();
    let first = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    let mut second = first.clone();
    second.username = "bob".into();
    second.password = "second".into();
    let second = write_settings_with_store(&conn, second, true, &store).unwrap();
    write_settings_with_store(&conn, first.clone(), false, &store).unwrap();
    assert_eq!(
        read_settings_with_store(&conn, &store).unwrap().password,
        " original secret "
    );
    let mut cleared = first.clone();
    cleared.enabled = false;
    let cleared = write_settings_with_store(&conn, cleared, true, &store).unwrap();
    assert!(!cleared.has_password);
    assert_eq!(
        store.get(&credential_scope(&second)).unwrap().as_deref(),
        Some("second")
    );
    write_settings_with_store(&conn, second, false, &store).unwrap();
    assert_eq!(
        read_settings_with_store(&conn, &store).unwrap().password,
        "second"
    );
}

#[test]
fn legacy_entry_migrates_only_to_the_stored_target() {
    let conn = connection();
    let store = MemoryStore::default();
    let mut old = configured();
    old.password.clear();
    old.has_password = true;
    set_json_app_setting(&conn, WEBDAV_SYNC_SETTINGS_KEY, &old).unwrap();
    store.set(WEBDAV_KEYRING_ACCOUNT, "legacy").unwrap();
    let loaded = read_settings_with_store(&conn, &store).unwrap();
    assert_eq!(loaded.password, "legacy");
    assert_eq!(store.get(WEBDAV_KEYRING_ACCOUNT).unwrap(), None);
    let mut changed = loaded.masked_for_frontend();
    changed.base_url = "https://new.test".into();
    changed.enabled = false;
    let saved = write_settings_with_store(&conn, changed, false, &store).unwrap();
    assert!(!saved.has_password);
    assert!(read_settings_with_store(&conn, &store)
        .unwrap()
        .password
        .is_empty());
}

#[test]
fn failed_settings_persistence_does_not_replace_credentials() {
    let conn = connection();
    let store = MemoryStore::default();
    let mut settings = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_settings BEFORE INSERT ON app_settings BEGIN SELECT RAISE(ABORT, 'failed'); END").unwrap();
    settings.password = "replacement".into();
    settings.backup_encryption = serde_json::from_value(serde_json::json!({
        "passphrase": "replacement-backup-password", "passphraseTouched": true
    }))
    .unwrap();
    assert!(write_settings_with_store(&conn, settings, true, &store).is_err());
    assert_eq!(
        read_settings_with_store(&conn, &store).unwrap().password,
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
fn backup_password_is_scoped_to_server_account_root_and_profile() {
    let conn = connection();
    let store = MemoryStore::default();
    let original = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    assert!(original.backup_encryption.has_passphrase);
    assert!(original.backup_encryption.passphrase.is_empty());
    let raw: serde_json::Value = get_json_app_setting(&conn, WEBDAV_SYNC_SETTINGS_KEY)
        .unwrap()
        .unwrap();
    assert!(raw["backup_encryption"].get("passphrase").is_none());
    assert!(raw["backup_encryption"].get("passphraseTouched").is_none());
    for field in ["server", "account", "root", "profile"] {
        let mut other = original.clone();
        other.enabled = false;
        match field {
            "server" => other.base_url = "https://other.test".into(),
            "account" => other.username = "bob".into(),
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
    assert_eq!(loaded.password, " original secret ");
}

#[test]
fn remote_path_changes_reset_sync_history_without_losing_same_account() {
    let conn = connection();
    let store = MemoryStore::default();
    let saved = write_settings_with_store(&conn, configured(), true, &store).unwrap();
    let mut stored = saved.clone();
    stored.last_sync_at = Some("old".into());
    stored.last_error = Some("old error".into());
    set_json_app_setting(&conn, WEBDAV_SYNC_SETTINGS_KEY, &stored).unwrap();
    let mut changed = saved;
    changed.profile = "other".into();
    let saved = write_settings_with_store(&conn, changed, false, &store).unwrap();
    assert!(saved.last_sync_at.is_none() && saved.last_error.is_none());
    assert!(saved.has_password);
}

#[test]
fn upload_status_keeps_concurrent_edits_and_does_not_mark_another_target_synced() {
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
    other.profile = "other".into();
    write_settings_with_store(&conn, other, false, &store).unwrap();
    update_upload_status(&conn, &original, "late-completion".into()).unwrap();
    let loaded = read_settings_with_store(&conn, &store).unwrap();
    assert_eq!(loaded.profile, "other");
    assert!(loaded.last_sync_at.is_none());
}
