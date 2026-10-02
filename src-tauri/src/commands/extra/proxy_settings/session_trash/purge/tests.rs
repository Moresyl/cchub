use super::super::{delete, list, read_manifest, restore, save, tests::fixture};
use super::*;

fn selection(trash: &Path, key: &str) -> SessionPurgeTarget {
    SessionPurgeTarget {
        key: key.into(),
        revision: revision(trash, key).unwrap(),
    }
}

#[test]
fn purge_one_removes_paired_recovery_copies_and_preserves_live_files_and_other_entries() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root.clone()], &trash).unwrap();
    let chosen = selection(&trash, &saved.key);
    let bytes = b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"same\"}}\n";
    fs::write(&target.source_path, bytes).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&target.source_path)
        .unwrap()
        .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1700000000))
        .unwrap();
    let newer = delete(&target, &[root.clone()], &trash).unwrap();
    fs::write(&target.source_path, "new live content").unwrap();
    fs::write(root.join("session_index.jsonl"), "shared index").unwrap();
    let result = purge(&trash, vec![chosen.clone(), chosen]).unwrap();
    assert_eq!(result.purged, [saved.key.clone()]);
    assert!(result.failed.is_empty());
    assert!(!trash.join(&saved.key).exists());
    assert_eq!(list(&trash).unwrap()[0].key, newer.key);
    assert_eq!(fs::read(&target.source_path).unwrap(), b"new live content");
    assert_eq!(
        fs::read(root.join("session_index.jsonl")).unwrap(),
        b"shared index"
    );
    assert!(restore(&trash, &saved.key, &[root]).is_err());
    assert!(trash.is_dir());
}

#[test]
fn batch_uses_only_confirmed_entries_and_continues_after_a_changed_one() {
    let (_area, root, trash, target) = fixture();
    let first = delete(&target, &[root.clone()], &trash).unwrap();
    let chosen = selection(&trash, &first.key);
    let later_key = uuid::Uuid::new_v4().to_string();
    let (dir, mut manifest) = read_manifest(&trash, &first.key).unwrap();
    let later = trash.join(&later_key);
    fs::create_dir(&later).unwrap();
    for item in &manifest.items {
        fs::copy(dir.join(&item.blob), later.join(&item.blob)).unwrap();
    }
    manifest.session.key = later_key.clone();
    save(&later, &manifest).unwrap();
    let later_selection = selection(&trash, &later_key);
    fs::write(later.join("0.blob"), "changed snapshot").unwrap();
    let result = purge(&trash, vec![later_selection, chosen]).unwrap();
    assert_eq!(result.purged, [first.key]);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].reason, PurgeFailureReason::Changed);
    assert!(later.join("session.json").is_file());
    assert!(later.join("1.blob").is_file());
}

#[test]
fn stale_manifest_or_restored_entry_is_not_purged() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root.clone()], &trash).unwrap();
    let chosen = selection(&trash, &saved.key);
    let (dir, mut manifest) = read_manifest(&trash, &saved.key).unwrap();
    manifest.session.title = Some("edited title".into());
    save(&dir, &manifest).unwrap();
    let result = purge(&trash, vec![chosen]).unwrap();
    assert_eq!(result.failed[0].reason, PurgeFailureReason::Changed);
    assert!(dir.join("0.blob").is_file());
    let chosen = selection(&trash, &saved.key);
    restore(&trash, &saved.key, &[root]).unwrap();
    let result = purge(&trash, vec![chosen]).unwrap();
    assert_eq!(result.failed[0].reason, PurgeFailureReason::Unsafe);
    assert!(Path::new(&target.source_path).is_file());
    assert!(dir.join("0.blob").is_file());
}

#[test]
fn unknown_files_nested_folders_and_malformed_blob_names_are_preserved() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root], &trash).unwrap();
    let chosen = selection(&trash, &saved.key);
    let dir = trash.join(&saved.key);
    for name in ["notes.txt", "2.blob"] {
        fs::write(dir.join(name), "unowned content").unwrap();
        assert!(revision(&trash, &saved.key).is_err());
        let result = purge(&trash, vec![chosen.clone()]).unwrap();
        assert_eq!(result.failed[0].reason, PurgeFailureReason::Unsafe);
        assert_eq!(fs::read(dir.join(name)).unwrap(), b"unowned content");
        assert!(dir.join("0.blob").is_file());
        fs::remove_file(dir.join(name)).unwrap();
    }
    fs::create_dir(dir.join("nested")).unwrap();
    assert!(revision(&trash, &saved.key).is_err());
    fs::remove_dir(dir.join("nested")).unwrap();
    let (_, mut manifest) = read_manifest(&trash, &saved.key).unwrap();
    manifest.items[0].blob = "../outside".into();
    save(&dir, &manifest).unwrap();
    assert!(revision(&trash, &saved.key).is_err());
    assert!(dir.join("0.blob").is_file());
    assert!(list(&trash).unwrap()[0].purge_revision.is_none());
}

#[test]
fn rejects_traversal_missing_manifests_invalid_revisions_and_oversized_batches() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root], &trash).unwrap();
    let chosen = selection(&trash, &saved.key);
    for key in ["", ".", "..", "../outside", "/", "C:\\", "a/b", "a\\b"] {
        assert!(revision(&trash, key).is_err());
    }
    let empty_key = uuid::Uuid::new_v4().to_string();
    fs::create_dir(trash.join(&empty_key)).unwrap();
    assert!(revision(&trash, &empty_key).is_err());
    for invalid in ["", "f", &"x".repeat(64)] {
        let result = purge(
            &trash,
            vec![SessionPurgeTarget {
                key: saved.key.clone(),
                revision: invalid.into(),
            }],
        )
        .unwrap();
        assert_eq!(result.failed[0].reason, PurgeFailureReason::Changed);
    }
    assert!(purge(&trash, vec![]).is_err());
    assert!(purge(&trash, vec![chosen; MAX_TARGETS + 1]).is_err());
    assert!(trash.join(saved.key).join("0.blob").is_file());
    assert!(serde_json::from_value::<SessionPurgeTarget>(
        serde_json::json!({"key":"x","revision":"y","all":true})
    )
    .is_err());
}

#[test]
fn partial_remove_failure_retains_note_updates_revision_and_can_be_retried() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root], &trash).unwrap();
    let chosen = selection(&trash, &saved.key);
    let old_revision = chosen.revision.clone();
    let mut calls = 0;
    let result = purge_with(&trash, vec![chosen], |path| {
        calls += 1;
        if calls == 2 {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "masked-fixture-secret",
            ))
        } else {
            fs::remove_file(path)
        }
    })
    .unwrap();
    assert_eq!(result.failed[0].reason, PurgeFailureReason::RemoveFailed);
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("masked-fixture-secret"));
    assert!(trash.join(&saved.key).join("session.json").is_file());
    let listed = list(&trash).unwrap();
    assert_ne!(listed[0].purge_revision.as_ref().unwrap(), &old_revision);
    let result = purge(&trash, vec![selection(&trash, &saved.key)]).unwrap();
    assert_eq!(result.purged, [saved.key]);
    assert!(list(&trash).unwrap().is_empty());
}

#[cfg(any(unix, windows))]
#[test]
fn refuses_linked_recovery_directories_and_blob_files() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root], &trash).unwrap();
    let dir = trash.join(&saved.key);
    let outside = trash.with_file_name("outside");
    fs::rename(&dir, &outside).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &dir).unwrap();
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_dir(&outside, &dir) {
        // Windows may require Developer Mode or the symbolic-link privilege.
        assert_eq!(error.raw_os_error(), Some(1314));
        eprintln!("Symbolic-link fixture unavailable: Windows privilege 1314");
        return;
    }
    assert!(revision(&trash, &saved.key).is_err());
    assert!(outside.join("0.blob").is_file());
    #[cfg(unix)]
    fs::remove_file(&dir).unwrap();
    #[cfg(windows)]
    fs::remove_dir(&dir).unwrap();
    fs::rename(&outside, &dir).unwrap();
    let blob = dir.join("0.blob");
    let outside_blob = trash.with_file_name("outside.blob");
    fs::rename(&blob, &outside_blob).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_blob, &blob).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&outside_blob, &blob).unwrap();
    assert!(revision(&trash, &saved.key).is_err());
    assert!(outside_blob.is_file());
}

#[test]
fn files_added_during_purge_stop_cleanup_before_the_recovery_note_is_removed() {
    let (_area, root, trash, target) = fixture();
    let saved = delete(&target, &[root], &trash).unwrap();
    let chosen = selection(&trash, &saved.key);
    let dir = trash.join(&saved.key);
    let mut calls = 0;
    let result = purge_with(&trash, vec![chosen], |path| {
        calls += 1;
        fs::remove_file(path)?;
        fs::write(dir.join("unexpected.txt"), "new external content")
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(result.failed[0].reason, PurgeFailureReason::Unsafe);
    assert!(dir.join("session.json").is_file());
    assert!(dir.join("1.blob").is_file());
    assert_eq!(
        fs::read(dir.join("unexpected.txt")).unwrap(),
        b"new external content"
    );
}

#[test]
fn directory_remove_failure_keeps_a_note_and_never_overwrites_newer_external_content() {
    for newer_note in [false, true] {
        let (_area, root, trash, target) = fixture();
        let saved = delete(&target, &[root], &trash).unwrap();
        let dir = trash.join(&saved.key);
        let result = purge_with(&trash, vec![selection(&trash, &saved.key)], |path| {
            fs::remove_file(path)?;
            if path.file_name().is_some_and(|name| name == "session.json") {
                fs::write(dir.join("new-external.txt"), "retained external data")?;
                if newer_note {
                    fs::write(dir.join("session.json"), "new external note")?;
                }
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(result.failed[0].reason, PurgeFailureReason::RemoveFailed);
        assert!(dir.join("session.json").is_file());
        assert_eq!(
            fs::read(dir.join("new-external.txt")).unwrap(),
            b"retained external data"
        );
        if newer_note {
            assert_eq!(
                fs::read(dir.join("session.json")).unwrap(),
                b"new external note"
            );
        } else {
            assert_eq!(list(&trash).unwrap()[0].key, saved.key);
            assert!(list(&trash).unwrap()[0].purge_revision.is_none());
            fs::remove_file(dir.join("new-external.txt")).unwrap();
            assert_eq!(
                purge(&trash, vec![selection(&trash, &saved.key)])
                    .unwrap()
                    .purged,
                [saved.key]
            );
        }
    }
}
