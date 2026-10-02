use super::*;

#[test]
fn aliases_and_guards_preflight_before_any_write() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file");
    std::fs::write(&target, b"old").unwrap();
    let mut plan = FilePlan::default();
    plan.replace(target.clone(), b"first".to_vec()).unwrap();
    plan.replace(dir.path().join("./file"), b"second".to_vec())
        .unwrap();
    assert!(plan.commit().unwrap_err().contains("same location"));
    assert_eq!(std::fs::read(&target).unwrap(), b"old");
    let mut plan = FilePlan::default();
    plan.replace(target.clone(), b"new".to_vec()).unwrap();
    let untouched = dir.path().join("untouched");
    plan.guards.push((untouched.clone(), None));
    std::fs::write(&untouched, b"external").unwrap();
    assert!(plan.commit().is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"old");
}

#[cfg(windows)]
#[test]
fn windows_case_aliases_are_rejected_even_for_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("config.json");
    let second = dir.path().join("CONFIG.JSON");
    let mut plan = FilePlan::default();
    plan.replace(first.clone(), b"a".to_vec()).unwrap();
    plan.replace(second, b"b".to_vec()).unwrap();
    assert!(plan.commit().is_err());
    assert!(!first.exists());
}

#[test]
fn a_guard_changed_by_the_finalizer_rolls_back_owned_files_and_preserves_the_guard() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("file");
    let guarded = dir.path().join("guard");
    std::fs::write(&target, b"old").unwrap();
    let mut plan = FilePlan::default();
    plan.replace(target.clone(), b"new".to_vec()).unwrap();
    plan.guards.push((guarded.clone(), None));
    assert!(plan
        .commit_then(|| {
            assert_eq!(std::fs::read(&target).unwrap(), b"new");
            std::fs::write(&guarded, b"external").unwrap();
            Err("finalizer fixture".into())
        })
        .is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"old");
    assert_eq!(std::fs::read(guarded).unwrap(), b"external");
}
