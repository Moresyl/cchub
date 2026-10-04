use super::*;

#[test]
fn equal_byte_external_replacement_is_not_overwritten_during_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    std::fs::write(&path, b"original").unwrap();
    let mut external = None;
    let error = commit_then(vec![update(&path, b"desired")], || {
        crate::utils::atomic_write(&path, b"desired").unwrap();
        external = Some(FileRevision::capture(&path).unwrap().0);
        Err("database fixture refused".into())
    })
    .unwrap_err();
    assert!(error.contains("recovery is incomplete"));
    external.unwrap().verify().unwrap();
    assert_eq!(read(&path).unwrap().unwrap(), b"desired");
    let recovery = PathBuf::from(error.split("Original files: ").nth(1).unwrap());
    assert_eq!(std::fs::read(recovery.join("0")).unwrap(), b"original");
    std::fs::remove_dir_all(recovery).unwrap();
}

#[test]
fn equal_byte_replacement_between_members_stops_before_the_second_write() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::write(&first, b"original first").unwrap();
    std::fs::write(&second, b"original second").unwrap();
    let mut external = None;
    let mut writes = 0;
    let mut finalized = false;
    let error = commit_owned(
        vec![
            update(&first, b"desired first"),
            update(&second, b"desired second"),
        ],
        Vec::new(),
        |index, path, bytes| {
            writes += 1;
            assert_eq!(index, 0);
            let handle = crate::utils::atomic_write_retained(path, bytes).unwrap();
            crate::utils::atomic_write(&second, b"original second").unwrap();
            external = Some(FileRevision::capture(&second).unwrap().0);
            (Some(handle), Ok(()))
        },
        || {
            finalized = true;
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("identity changed"));
    assert!(!error.contains("recovery is incomplete"));
    assert_eq!(writes, 1);
    assert!(!finalized);
    assert_eq!(read(&first).unwrap().unwrap(), b"original first");
    assert_eq!(read(&second).unwrap().unwrap(), b"original second");
    external.unwrap().verify().unwrap();
}

#[test]
fn a_supplied_old_identity_rejects_equal_bytes_before_writes_or_finalization() {
    for desired in [b"original".as_slice(), b"changed".as_slice()] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"original").unwrap();
        let (revision, _) = FileRevision::capture(&path).unwrap();
        crate::utils::atomic_write(&path, b"original").unwrap();
        let (external, _) = FileRevision::capture(&path).unwrap();
        let mut finalized = false;
        assert!(
            commit_then_with_revisions(vec![update(&path, desired)], vec![revision], || {
                finalized = true;
                Ok(())
            })
            .unwrap_err()
            .contains("identity changed")
        );
        assert!(!finalized);
        external.verify().unwrap();
        assert_eq!(read(&path).unwrap().unwrap(), b"original");
    }
}

#[test]
fn recovery_keeps_a_directory_created_by_another_writer_between_members() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("external/second");
    std::fs::write(&first, b"original").unwrap();
    let error = commit_with(
        vec![update(&first, b"desired"), update(&second, b"created")],
        |index, path, bytes| {
            if index == 1 {
                return Err(std::io::Error::other("fixture failure"));
            }
            crate::utils::atomic_write(path, bytes)?;
            std::fs::create_dir(second.parent().unwrap())
        },
    )
    .unwrap_err();
    assert!(!error.contains("recovery is incomplete"));
    assert_eq!(read(&first).unwrap().unwrap(), b"original");
    assert!(!second.exists());
    assert!(second.parent().unwrap().is_dir());
}

#[cfg(windows)]
#[test]
fn junction_retarget_after_write_recovers_only_the_original_physical_source() {
    fn junction(path: &Path, target: &Path) {
        let mut command = std::process::Command::new("powershell");
        crate::utils::configure_background_command(&mut command);
        let output = command
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:CCHUB_TEST_LINK -Target $env:CCHUB_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("CCHUB_TEST_LINK", path)
            .env("CCHUB_TEST_TARGET", target)
            .output().unwrap();
        assert!(
            output.status.success(),
            "owned fixture junction creation failed"
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    let alias = dir.path().join("alias");
    for folder in [&first, &second] {
        std::fs::create_dir(folder).unwrap();
        std::fs::write(folder.join("config"), b"original").unwrap();
    }
    junction(&alias, &first);
    let mut finalized = false;
    let error = commit_owned(
        vec![update(&alias.join("config"), b"desired")],
        Vec::new(),
        |_, path, bytes| {
            let handle = crate::utils::atomic_write_retained(path, bytes).unwrap();
            std::fs::remove_dir(&alias).unwrap();
            junction(&alias, &second);
            (Some(handle), Ok(()))
        },
        || {
            finalized = true;
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("location changed"));
    assert!(!error.contains("recovery is incomplete"));
    assert!(!finalized);
    for folder in [&first, &second] {
        assert_eq!(std::fs::read(folder.join("config")).unwrap(), b"original");
    }
    std::fs::remove_dir(alias).unwrap();
}

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
    assert_eq!(
        map[0]["target"],
        target_key(&first).unwrap().to_string_lossy().as_ref()
    );
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
