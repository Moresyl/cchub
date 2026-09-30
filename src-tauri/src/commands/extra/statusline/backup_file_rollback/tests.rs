use super::*;

#[test]
fn rollback_restores_binary_bytes_and_removes_new_files_and_empty_directories() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original");
    let new = dir.path().join("new/nested/file");
    std::fs::write(&original, [255, 0, 2]).unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    journal.capture(&original).unwrap();
    journal.capture(&new).unwrap();
    journal
        .before_write(&[original.clone(), new.clone()])
        .unwrap();
    crate::utils::atomic_write(&original, b"changed").unwrap();
    crate::utils::atomic_write(&new, b"new file").unwrap();
    assert_eq!(journal.rollback("failed".into()), "failed");
    assert_eq!(std::fs::read(&original).unwrap(), [255, 0, 2]);
    assert!(!dir.path().join("new").exists());
}

#[test]
fn external_changes_before_writing_are_preserved_and_stop_the_group() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::write(&first, "original").unwrap();
    std::fs::write(&second, "original").unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    journal.capture(&first).unwrap();
    journal.capture(&second).unwrap();
    std::fs::write(&second, "external update").unwrap();
    assert!(journal
        .before_write(&[first.clone(), second.clone()])
        .is_err());
    drop(journal);
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "original");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "external update");
}

#[test]
fn dropping_an_uncommitted_journal_rolls_back_but_committing_keeps_changes() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, "original").unwrap();
    for commit in [false, true] {
        let mut journal = FileRollback::new(dir.path()).unwrap();
        let storage = journal.storage.as_ref().unwrap().path().to_path_buf();
        journal.capture(&target).unwrap();
        journal.before_write(std::slice::from_ref(&target)).unwrap();
        crate::utils::atomic_write(&target, b"changed").unwrap();
        if commit {
            journal.commit();
        }
        drop(journal);
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            if commit { "changed" } else { "original" }
        );
        assert!(!storage.exists());
    }
}

#[cfg(windows)]
#[test]
fn rollback_failure_retains_original_files_and_a_recovery_map() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, "original").unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    let storage = journal.storage.as_ref().unwrap().path().to_path_buf();
    journal.capture(&target).unwrap();
    journal.before_write(std::slice::from_ref(&target)).unwrap();
    crate::utils::atomic_write(&target, b"changed").unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&target)
        .unwrap();
    let error = journal.rollback("failed".into());
    assert!(error.contains("未能回滚"));
    drop(journal);
    assert_eq!(
        std::fs::read_to_string(storage.join("0")).unwrap(),
        "original"
    );
    let map: serde_json::Value =
        serde_json::from_slice(&std::fs::read(storage.join("restore-map.json")).unwrap()).unwrap();
    assert_eq!(map[0]["target"], target.to_string_lossy().as_ref());
    assert_eq!(map[0]["original"], "0");
    drop(held);
}

#[cfg(windows)]
#[test]
fn read_only_original_snapshot_can_be_cleaned_up_without_changing_original_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, "original").unwrap();
    let mut permissions = std::fs::metadata(&target).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&target, permissions).unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    let storage = journal.storage.as_ref().unwrap().path().to_path_buf();
    journal.capture(&target).unwrap();
    journal.commit();
    drop(journal);
    assert!(!storage.exists());
    assert!(std::fs::metadata(&target).unwrap().permissions().readonly());
    let mut permissions = std::fs::metadata(&target).unwrap().permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(&target, permissions).unwrap();
}
