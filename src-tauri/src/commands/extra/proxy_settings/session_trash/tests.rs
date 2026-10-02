use super::*;
use std::time::UNIX_EPOCH;

pub(super) fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, SessionDeleteTarget) {
    let area = tempfile::tempdir().unwrap();
    let root = area.path().join("codex");
    let trash = area.path().join("trash");
    fs::create_dir(&root).unwrap();
    let plain = root.join("rollout.jsonl");
    let bytes = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"same\"}}\n";
    fs::write(&plain, bytes).unwrap();
    fs::write(
        archive::twin(&plain).unwrap(),
        zstd::stream::encode_all(&bytes[..], 0).unwrap(),
    )
    .unwrap();
    for path in [&plain, &archive::twin(&plain).unwrap()] {
        OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(1700000000))
            .unwrap();
    }
    let target = SessionDeleteTarget {
        tool_id: "codex".into(),
        session_id: "same".into(),
        source_backend: "jsonl".into(),
        source_path: plain.to_string_lossy().into(),
    };
    (area, root, trash, target)
}

#[test]
fn paired_deletion_retains_bytes_and_restores_both_without_touching_shared_indexes() {
    let (_area, root, trash, target) = fixture();
    let plain = Path::new(&target.source_path);
    let packed = archive::twin(plain).unwrap();
    let old_plain = fs::read(plain).unwrap();
    let old_packed = fs::read(&packed).unwrap();
    fs::write(root.join("session_index.jsonl"), "shared index").unwrap();
    let result = delete(&target, &[root.clone()], &trash).unwrap();
    assert!(!plain.exists() && !packed.exists());
    assert_eq!(list(&trash).unwrap().len(), 1);
    assert_eq!(
        fs::read(root.join("session_index.jsonl")).unwrap(),
        b"shared index"
    );
    restore(&trash, &result.key, &[root.clone()]).unwrap();
    assert_eq!(fs::read(plain).unwrap(), old_plain);
    assert_eq!(fs::read(&packed).unwrap(), old_packed);
    assert!(list(&trash).unwrap().is_empty());
    assert!(
        trash.join(result.key).join("0.blob").is_file(),
        "recovery copies are retained"
    );
}

#[test]
fn failures_after_one_remove_keep_all_recovery_bytes_and_support_retry() {
    let (_area, root, trash, target) = fixture();
    let mut calls = 0;
    let error = delete_with(&target, &[root.clone()], &trash, |path| {
        calls += 1;
        if calls == 2 {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "fixture remove failure",
            ))
        } else {
            fs::remove_file(path)
        }
    })
    .unwrap_err();
    assert!(error.contains("原始文件已保留"));
    let saved = list(&trash).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].state, "recovery");
    restore(&trash, &saved[0].key, &[root.clone()]).unwrap();
    assert!(Path::new(&target.source_path).exists());
    assert!(archive::twin(Path::new(&target.source_path))
        .unwrap()
        .exists());
}

#[test]
fn restore_collision_preserves_new_content_and_all_recovery_copies() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root.clone()], &trash).unwrap();
    fs::write(&target.source_path, "new content").unwrap();
    assert!(restore(&trash, &saved.key, &[root.clone()])
        .unwrap_err()
        .contains("已有不同内容"));
    assert_eq!(fs::read(&target.source_path).unwrap(), b"new content");
    assert!(!archive::twin(Path::new(&target.source_path))
        .unwrap()
        .exists());
    assert_eq!(list(&trash).unwrap().len(), 1);
}

#[test]
fn mismatched_identity_active_files_and_foreign_roots_are_refused_before_deletion() {
    let (_area, root, trash, mut target) = fixture();
    target.session_id = "other".into();
    assert!(delete(&target, &[root.clone()], &trash).is_err());
    target.session_id = "same".into();
    assert!(delete(&target, &[root.join("outside")], &trash).is_err());
    OpenOptions::new()
        .write(true)
        .open(&target.source_path)
        .unwrap()
        .set_modified(std::time::SystemTime::now())
        .unwrap();
    assert!(delete(&target, &[root], &trash)
        .unwrap_err()
        .contains("稍后重试"));
    assert!(Path::new(&target.source_path).is_file());
}

#[test]
fn a_tampered_recovery_manifest_cannot_restore_outside_owned_roots() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root.clone()], &trash).unwrap();
    let (dir, mut manifest) = read_manifest(&trash, &saved.key).unwrap();
    manifest.items[0].path = root.join("../outside.jsonl");
    save(&dir, &manifest).unwrap();
    assert!(restore(&trash, &saved.key, &[root]).is_err());
    assert!(trash.join(saved.key).join("0.blob").is_file());
    assert!(read_manifest(&trash, "../other").is_err());
}

#[test]
fn restore_verifies_snapshot_identity_and_blob_names_before_writing_any_file() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root.clone()], &trash).unwrap();
    let (dir, mut manifest) = read_manifest(&trash, &saved.key).unwrap();
    manifest.session.session_id = "different".into();
    save(&dir, &manifest).unwrap();
    assert!(restore(&trash, &saved.key, &[root.clone()])
        .unwrap_err()
        .contains("identity"));
    assert!(!Path::new(&target.source_path).exists());
    manifest.session.session_id = "same".into();
    manifest.items[0].blob = "C:0.blob".into();
    save(&dir, &manifest).unwrap();
    assert!(restore(&trash, &saved.key, &[root.clone()]).is_err());
    assert!(!Path::new(&target.source_path).exists());
    assert!(!archive::twin(Path::new(&target.source_path))
        .unwrap()
        .exists());
}

#[test]
fn recovery_labels_preserve_readable_titles_and_old_manifests_remain_readable() {
    let (_area, root, trash, target) = fixture();
    let plain = Path::new(&target.source_path);
    let bytes = concat!(
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"same\"}}\n",
        "{\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"恢复 会话\\n内容\"}]}}\n"
    );
    fs::write(plain, bytes).unwrap();
    fs::write(
        archive::twin(plain).unwrap(),
        zstd::stream::encode_all(bytes.as_bytes(), 0).unwrap(),
    )
    .unwrap();
    for path in [plain.to_path_buf(), archive::twin(plain).unwrap()] {
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(1700000000))
            .unwrap();
    }
    let saved = delete(&target, &[root.clone()], &trash).unwrap();
    assert_eq!(saved.title.as_deref(), Some("恢复 会话 内容"));
    let manifest_path = trash.join(&saved.key).join("session.json");
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    legacy["session"].as_object_mut().unwrap().remove("title");
    fs::write(&manifest_path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(list(&trash).unwrap()[0].title, None);
    restore(&trash, &saved.key, &[root]).unwrap();
    assert_eq!(fs::read_to_string(plain).unwrap(), bytes);
}
