use super::*;

#[cfg(windows)]
#[test]
fn an_external_parent_junction_cannot_redirect_restore_targets_or_guards() {
    for written in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let archived = dir.path().join("archived");
        let external = dir.path().join("external");
        std::fs::create_dir(&external).unwrap();
        let target = root.join("file");
        std::fs::write(external.join("file"), "restored").unwrap();
        let mut journal = FileRollback::new(dir.path()).unwrap();
        if written {
            journal.capture(&target).unwrap();
            journal.write(&target, b"restored").unwrap();
        } else {
            std::fs::create_dir(&root).unwrap();
            std::fs::write(&target, b"restored").unwrap();
            let mut plan = crate::config_write::FilePlan::default();
            plan.guards
                .push((target.clone(), Some(b"restored".to_vec())));
            journal.apply_plan(plan).unwrap();
        }
        std::fs::rename(&root, &archived).unwrap();
        let mut command = std::process::Command::new("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:CCHUB_TEST_LINK -Target $env:CCHUB_TEST_TARGET -ErrorAction Stop | Out-Null"])
        .env("CCHUB_TEST_LINK", &root).env("CCHUB_TEST_TARGET", &external);
        crate::utils::configure_background_command(&mut command);
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(journal.verify().is_err());
        assert!(journal.write(&target, b"another write").is_err());
        let error = journal.rollback("failed".into());
        if written {
            assert!(error.contains("外部修改已保留"));
        } else {
            assert_eq!(error, "failed");
        }
        assert_eq!(std::fs::read(external.join("file")).unwrap(), b"restored");
        assert_eq!(std::fs::read(archived.join("file")).unwrap(), b"restored");
        assert!(
            root.exists(),
            "rollback must not remove an externally created junction"
        );
        std::fs::remove_dir(&root).unwrap();
    }
}

#[test]
fn external_edits_deletions_and_directory_replacements_are_preserved_on_rollback() {
    for change in ["edit", "delete", "directory"] {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let external = dir.path().join("external");
        std::fs::write(&first, [255, 0, 2]).unwrap();
        std::fs::write(&external, "original").unwrap();
        let mut journal = FileRollback::new(dir.path()).unwrap();
        let storage = journal.storage.as_ref().unwrap().path().to_path_buf();
        journal.capture(&first).unwrap();
        journal.capture(&external).unwrap();
        journal.write(&first, b"restored").unwrap();
        journal.write(&external, b"restored").unwrap();
        match change {
            "edit" => std::fs::write(&external, "new external bytes").unwrap(),
            "delete" => std::fs::remove_file(&external).unwrap(),
            _ => {
                std::fs::remove_file(&external).unwrap();
                std::fs::create_dir(&external).unwrap();
                std::fs::write(external.join("user-file"), "keep").unwrap();
            }
        }
        assert!(journal.verify().is_err());
        let error = journal.rollback("failed".into());
        assert!(error.contains("较新的外部修改已保留"));
        assert_eq!(std::fs::read(&first).unwrap(), [255, 0, 2]);
        assert_eq!(std::fs::read(storage.join("1")).unwrap(), b"original");
        match change {
            "edit" => assert_eq!(std::fs::read(&external).unwrap(), b"new external bytes"),
            "delete" => assert!(!external.exists()),
            _ => assert_eq!(std::fs::read(external.join("user-file")).unwrap(), b"keep"),
        }
        let map: serde_json::Value =
            serde_json::from_slice(&std::fs::read(storage.join("restore-map.json")).unwrap())
                .unwrap();
        assert_eq!(map[1]["original"], "1");
        assert!(map[1]["desired"].as_str().unwrap().starts_with("desired-"));
    }
}

#[test]
fn a_newly_created_file_with_a_later_external_edit_is_not_removed() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("new/nested/file");
    let mut journal = FileRollback::new(dir.path()).unwrap();
    journal.capture(&target).unwrap();
    journal.write(&target, b"restored").unwrap();
    std::fs::write(&target, "external").unwrap();
    assert!(journal.rollback("failed".into()).contains("外部修改已保留"));
    assert_eq!(std::fs::read(&target).unwrap(), b"external");
}

#[test]
fn successive_owned_writes_through_aliases_restore_one_exact_original() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file");
    let alias = dir.path().join("./file");
    std::fs::write(&target, [255, 1, 0]).unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    journal.capture(&target).unwrap();
    journal.capture(&alias).unwrap();
    assert_eq!(journal.files.len(), 1);
    journal.write(&target, b"snapshot").unwrap();
    journal.write(&alias, b"full backup").unwrap();
    journal.verify().unwrap();
    assert_eq!(journal.rollback("failed".into()), "failed");
    assert_eq!(std::fs::read(&target).unwrap(), [255, 1, 0]);
}

#[test]
fn a_failed_second_owned_write_recovers_the_previous_owned_version() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file");
    std::fs::write(&target, "original").unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    journal.capture(&target).unwrap();
    journal.write(&target, b"snapshot").unwrap();
    let error = journal
        .write_with(&target, b"full backup", |_, _| {
            Err(std::io::Error::other("private-credential-error"))
        })
        .unwrap_err();
    assert!(!error.contains("private-credential-error"));
    assert_eq!(std::fs::read(&target).unwrap(), b"snapshot");
    assert_eq!(journal.rollback(error), "无法写入恢复目标文件");
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
}

#[test]
fn a_reported_error_after_replacement_still_recovers_our_written_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file");
    std::fs::write(&target, "original").unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    journal.capture(&target).unwrap();
    assert!(journal
        .write_with(&target, b"restored", |path, bytes| {
            crate::utils::atomic_write(path, bytes)?;
            Err(std::io::Error::other("reported failure"))
        })
        .is_err());
    assert_eq!(journal.rollback("failed".into()), "failed");
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
}

#[test]
fn an_absent_guard_changed_after_applying_a_plan_stops_finalization() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("written");
    let guard = dir.path().join("absent");
    std::fs::write(&target, "original").unwrap();
    let mut plan = crate::config_write::FilePlan::default();
    plan.replace(target.clone(), b"restored".to_vec()).unwrap();
    plan.guards.push((guard.clone(), None));
    let mut journal = FileRollback::new(dir.path()).unwrap();
    journal.apply_plan(plan).unwrap();
    std::fs::write(&guard, "external").unwrap();
    assert!(journal.verify().is_err());
    assert_eq!(journal.rollback("failed".into()), "failed");
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
    assert_eq!(std::fs::read(&guard).unwrap(), b"external");
}

#[test]
fn aliased_targets_inside_one_native_plan_fail_before_any_write() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file");
    std::fs::write(&target, "original").unwrap();
    let mut plan = crate::config_write::FilePlan::default();
    plan.replace(target.clone(), b"first".to_vec()).unwrap();
    plan.replace(dir.path().join("./file"), b"second".to_vec())
        .unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    assert!(journal.apply_plan(plan).is_err());
    assert!(journal.touched.is_empty());
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
}

#[test]
fn rollback_restores_binary_bytes_and_removes_new_files_and_empty_directories() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original");
    let new = dir.path().join("new/nested/file");
    std::fs::write(&original, [255, 0, 2]).unwrap();
    let mut journal = FileRollback::new(dir.path()).unwrap();
    journal.capture(&original).unwrap();
    journal.capture(&new).unwrap();
    journal.write(&original, b"changed").unwrap();
    journal.write(&new, b"new file").unwrap();
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
    let mut plan = crate::config_write::FilePlan::default();
    plan.updates = vec![first.clone(), second.clone()]
        .into_iter()
        .map(|path| crate::config_write::FileUpdate {
            path,
            original: Some(b"original".to_vec()),
            desired: b"restored".to_vec(),
        })
        .collect();
    assert!(journal.apply_plan(plan).is_err());
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
        journal.write(&target, b"changed").unwrap();
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
    journal.write(&target, b"changed").unwrap();
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
