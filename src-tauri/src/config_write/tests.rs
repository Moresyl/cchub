use super::*;

#[test]
fn a_failed_database_finalizer_restores_each_original_and_runs_after_verified_writes() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("new/nested/second");
    std::fs::write(&first, b"original").unwrap();
    let result = commit_then(
        vec![update(&first, b"new"), update(&second, b"created")],
        || {
            assert_eq!(read(&first).unwrap().unwrap(), b"new");
            assert_eq!(read(&second).unwrap().unwrap(), b"created");
            Err("database commit fixture failed".into())
        },
    );
    assert!(result.unwrap_err().contains("database commit"));
    assert_eq!(read(&first).unwrap().unwrap(), b"original");
    assert!(!second.exists());
    assert!(!dir.path().join("new").exists());
}

#[test]
fn no_op_files_still_run_the_database_finalizer_once_without_changing_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    std::fs::write(&path, b"original").unwrap();
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let mut called = 0;
    assert!(commit_then(vec![update(&path, b"original")], || {
        called += 1;
        Err("fixture".into())
    })
    .is_err());
    assert_eq!(called, 1);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
    assert_eq!(read(&path).unwrap().unwrap(), b"original");
}

#[test]
fn database_failure_with_a_newer_external_edit_keeps_the_edit_and_recovery_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    std::fs::write(&path, b"original").unwrap();
    let error = commit_then(vec![update(&path, b"ours")], || {
        std::fs::write(&path, b"external").unwrap();
        Err("database commit fixture failed".into())
    })
    .unwrap_err();
    assert_eq!(read(&path).unwrap().unwrap(), b"external");
    let recovery = PathBuf::from(error.split("Original files: ").nth(1).unwrap());
    assert!(recovery
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("cchub-config-recovery-"));
    assert_eq!(std::fs::read(recovery.join("0")).unwrap(), b"original");
    std::fs::remove_dir_all(recovery).unwrap();
}

#[test]
fn a_file_save_failure_does_not_attempt_the_database_finalizer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    let mut called = false;
    assert!(commit_with_finalizer(
        vec![update(&path, b"new")],
        |_, _, _| { Err(std::io::Error::other("fixture")) },
        || {
            called = true;
            Ok(())
        }
    )
    .is_err());
    assert!(!called);
    assert!(!path.exists());
}

fn update(path: &Path, desired: &[u8]) -> FileUpdate {
    FileUpdate {
        path: path.into(),
        original: read(path).unwrap(),
        desired: desired.to_vec(),
    }
}

#[test]
fn complete_preflight_stops_before_any_write_and_noops_retain_timestamps() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::write(&first, b"old first").unwrap();
    std::fs::write(&second, b"old second").unwrap();
    let updates = vec![update(&first, b"new first"), update(&second, b"new second")];
    std::fs::write(&second, b"external").unwrap();
    assert!(commit(updates).unwrap_err().contains("externally"));
    assert_eq!(read(&first).unwrap().unwrap(), b"old first");
    assert_eq!(read(&second).unwrap().unwrap(), b"external");
    let modified = std::fs::metadata(&first).unwrap().modified().unwrap();
    commit_with(vec![update(&first, b"old first")], |_, _, _| {
        panic!("no-op wrote a file")
    })
    .unwrap();
    assert_eq!(
        std::fs::metadata(first).unwrap().modified().unwrap(),
        modified
    );
}

#[test]
fn second_write_failure_restores_exact_bytes_and_removes_new_files_and_directories() {
    for first_exists in [false, true] {
        for failure_after_write in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let first = dir.path().join("new/nested/first");
            let second = dir.path().join("second");
            if first_exists {
                crate::utils::atomic_write(&first, &[255, 0, 2]).unwrap();
            }
            std::fs::write(&second, b"original second").unwrap();
            let before = read(&first).unwrap();
            let error = commit_with(
                vec![update(&first, b"new first"), update(&second, b"new second")],
                |index, path, bytes| {
                    if index == 1 && !failure_after_write {
                        return Err(std::io::Error::other("synthetic-private-secret"));
                    }
                    crate::utils::atomic_write(path, bytes)?;
                    if index == 1 {
                        return Err(std::io::Error::other("synthetic-private-secret"));
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(!error.contains("synthetic-private-secret"));
            assert_eq!(read(&first).unwrap(), before);
            assert_eq!(read(&second).unwrap().unwrap(), b"original second");
            if !first_exists {
                assert!(!dir.path().join("new").exists());
            }
        }
    }
}

#[test]
fn changes_between_member_writes_stop_the_group_and_preserve_the_external_edit() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::write(&first, b"old first").unwrap();
    std::fs::write(&second, b"old second").unwrap();
    let updates = vec![update(&first, b"new first"), update(&second, b"new second")];
    let error = commit_with(updates, |index, path, bytes| {
        assert_eq!(index, 0);
        crate::utils::atomic_write(path, bytes)?;
        std::fs::write(&second, b"external")
    })
    .unwrap_err();
    assert!(error.contains("externally"));
    assert_eq!(read(&first).unwrap().unwrap(), b"old first");
    assert_eq!(read(&second).unwrap().unwrap(), b"external");
}

#[test]
fn lost_write_ownership_retains_external_data_and_recoverable_originals() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::write(&first, b"private original").unwrap();
    std::fs::write(&second, b"old second").unwrap();
    let error = commit_with(
        vec![update(&first, b"new first"), update(&second, b"new second")],
        |index, path, bytes| {
            if index == 1 {
                std::fs::write(&first, b"external newer edit")?;
                return Err(std::io::Error::other("failure"));
            }
            crate::utils::atomic_write(path, bytes)
        },
    )
    .unwrap_err();
    assert!(!error.contains("private original"));
    assert!(error.contains("recovery is incomplete"));
    assert_eq!(read(&first).unwrap().unwrap(), b"external newer edit");
    assert_eq!(read(&second).unwrap().unwrap(), b"old second");
    let recovery = PathBuf::from(error.split("Original files: ").nth(1).unwrap());
    assert_eq!(
        std::fs::read(recovery.join("0")).unwrap(),
        b"private original"
    );
    let map: serde_json::Value =
        serde_json::from_slice(&std::fs::read(recovery.join("restore-map.json")).unwrap()).unwrap();
    assert_eq!(map[0]["target"], first.to_string_lossy().as_ref());
    assert_eq!(map[0]["original"], "0");
    std::fs::remove_dir_all(recovery).unwrap();
}

#[test]
fn non_file_targets_and_duplicate_targets_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read(dir.path()).is_err());
    let target = dir.path().join("target");
    assert!(commit(vec![update(&target, b"one"), update(&target, b"two")]).is_err());
    assert!(!target.exists());
}

#[cfg(windows)]
#[test]
fn real_second_file_replace_failure_rolls_back_the_first_file() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("config.toml");
    let second = dir.path().join("auth.json");
    std::fs::write(&first, b"old config").unwrap();
    std::fs::write(&second, b"old auth").unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&second)
        .unwrap();
    assert!(commit(vec![
        update(&first, b"new config"),
        update(&second, b"new auth")
    ])
    .is_err());
    assert_eq!(read(&first).unwrap().unwrap(), b"old config");
    assert_eq!(read(&second).unwrap().unwrap(), b"old auth");
    drop(held);
}
