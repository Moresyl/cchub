use super::*;
use crate::cloud_credentials::tests::MemoryStore;

const PASS: &str = "public-test-passphrase";

fn phrase(value: &str) -> Zeroizing<String> {
    Zeroizing::new(value.into())
}

#[test]
fn encryption_round_trip_hides_sql_and_uses_independent_random_headers() {
    let sql = b"INSERT INTO profiles VALUES ('private-api-key');";
    let first = seal(Zeroizing::new(sql.to_vec()), phrase(PASS)).unwrap();
    let second = seal(Zeroizing::new(sql.to_vec()), phrase(PASS)).unwrap();
    assert_ne!(&first[16..44], &second[16..44]);
    assert!(!first
        .windows(b"private-api-key".len())
        .any(|part| part == b"private-api-key"));
    assert_eq!(open(first, phrase(PASS)).unwrap().as_slice(), sql);
    assert_eq!(open(second, phrase(PASS)).unwrap().as_slice(), sql);
}

#[test]
fn opens_an_independent_node_crypto_fixture() {
    // Produced using node:crypto PBKDF2 and AES-256-GCM, rather than this encoder.
    let hex = "4343485542454e4301010100000927c0000102030405060708090a0b0c0d0e0f101112131415161718191a1b0000000000000016aef72f89e8c9cf7d4cf1ba7ee3dbb6f59c8000ebb96f48d55bcdbe9d95f116b6e1e6d1d695ac";
    let bytes = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(
        open(bytes, phrase(PASS)).unwrap().as_slice(),
        b"SELECT public_fixture;"
    );
}

#[test]
fn wrong_password_and_modified_ciphertext_never_return_plaintext() {
    let bytes = seal(Zeroizing::new(b"sensitive-sql".to_vec()), phrase(PASS)).unwrap();
    assert_eq!(
        open(bytes.clone(), phrase("wrong-password-long-enough")).unwrap_err(),
        AUTH_FAILED
    );
    for offset in [16, 32, HEADER_LEN, bytes.len() - 1] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        assert_eq!(open(changed, phrase(PASS)).unwrap_err(), AUTH_FAILED);
    }
}

#[test]
fn header_validation_rejects_unsupported_or_excessive_work_before_decryption() {
    let bytes = seal(Zeroizing::new(b"sql".to_vec()), phrase(PASS)).unwrap();
    for offset in [0, 8, 9, 10, 11] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        assert_eq!(open(changed, phrase(PASS)).unwrap_err(), INVALID);
    }
    for iterations in [0u32, 99_999, 1_000_001, u32::MAX] {
        let mut changed = bytes.clone();
        changed[12..16].copy_from_slice(&iterations.to_be_bytes());
        assert_eq!(open(changed, phrase(PASS)).unwrap_err(), INVALID);
    }
    for length in [0u64, (PLAINTEXT_LIMIT + 1) as u64, u64::MAX] {
        let mut changed = bytes.clone();
        changed[44..52].copy_from_slice(&length.to_be_bytes());
        assert_eq!(open(changed, phrase(PASS)).unwrap_err(), INVALID);
    }
    assert!(open(bytes[..bytes.len() - 1].to_vec(), phrase(PASS)).is_err());
    assert!(open(vec![0; HEADER_LEN + TAG_LEN - 1], phrase(PASS)).is_err());
}

#[test]
fn size_and_passphrase_rules_fail_without_encrypting() {
    assert!(seal(Zeroizing::new(Vec::new()), phrase(PASS)).is_err());
    assert!(seal(Zeroizing::new(vec![0; PLAINTEXT_LIMIT + 1]), phrase(PASS)).is_err());
    for pass in ["", "           ", "too-short"] {
        assert!(validate_new_passphrase(pass).is_err());
    }
    assert!(validate_new_passphrase(&"a".repeat(1025)).is_err());
    assert!(validate_new_passphrase("十二个中文字符也可以用作密码").is_ok());
}

#[test]
fn stored_encryption_password_is_masked_and_scoped() {
    let store = MemoryStore::default();
    store.set("server-one", PASS).unwrap();
    let mut settings = BackupEncryption::default();
    settings.load(&store, "server-one").unwrap();
    assert!(settings.has_passphrase);
    assert_eq!(settings.passphrase.as_str(), PASS);
    assert!(!format!("{settings:?}").contains(PASS));
    let json = serde_json::to_string(&settings).unwrap();
    assert!(!json.contains("passphrase\""));
    assert!(!json.contains(PASS));
    assert!(settings.masked().passphrase.is_empty());
    settings.load(&store, "server-two").unwrap();
    assert!(!settings.has_passphrase);
    assert!(settings.prepare_save(&store, "server-two", true).is_err());
    settings.prepare_save(&store, "server-one", true).unwrap();
    assert_eq!(settings.passphrase.as_str(), PASS);
    settings.passphrase_touched = true;
    settings.passphrase = phrase("");
    settings.prepare_save(&store, "server-one", false).unwrap();
    assert!(!settings.has_passphrase);
}

#[tokio::test]
async fn plaintext_restore_requires_explicit_legacy_consent() {
    let bytes = b"SELECT 1;".to_vec();
    assert!(open_for_restore(
        bytes.clone(),
        "".into(),
        "snapshots/old.sql".into(),
        phrase(""),
        false
    )
    .await
    .is_err());
    assert_eq!(
        open_for_restore(
            bytes,
            "".into(),
            "snapshots/old.sql".into(),
            phrase(""),
            true
        )
        .await
        .unwrap()
        .as_slice(),
        b"SELECT 1;"
    );
}

#[tokio::test]
async fn malformed_encrypted_files_cannot_fall_back_to_plaintext() {
    for (format, path, bytes) in [
        (PAYLOAD_FORMAT, "snapshots/new.sql", b"SELECT 1;".to_vec()),
        ("", "snapshots/new.cchub-backup", b"SELECT 1;".to_vec()),
        ("", "snapshots/old.sql", MAGIC.to_vec()),
    ] {
        assert!(
            open_for_restore(bytes, format.into(), path.into(), phrase(PASS), true)
                .await
                .is_err()
        );
    }
    assert!(open_for_restore(
        b"SELECT 1;".to_vec(),
        "future-format".into(),
        "snapshots/old.sql".into(),
        phrase(PASS),
        true
    )
    .await
    .is_err());
}

#[tokio::test]
async fn asynchronous_path_authenticates_before_exposing_restorable_sql() {
    let bytes = seal_async(b"SELECT 1;".to_vec(), phrase(PASS))
        .await
        .unwrap();
    let plain = open_for_restore(
        bytes,
        PAYLOAD_FORMAT.into(),
        "snapshots/new.cchub-backup".into(),
        phrase(PASS),
        false,
    )
    .await
    .unwrap();
    assert_eq!(plain.as_slice(), b"SELECT 1;");
}
