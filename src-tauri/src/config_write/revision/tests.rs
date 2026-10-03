use super::*;

#[test]
fn capture_uses_the_retained_native_file_and_exact_bytes() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("native.conf");
    std::fs::write(&file, b"\xef\xbb\xbfexact\r\n").unwrap();
    let (revision, bytes) = FileRevision::capture(&file).unwrap();
    assert_eq!(bytes.unwrap(), b"\xef\xbb\xbfexact\r\n");
    revision.verify().unwrap();
    let clone = revision.clone();
    drop(revision);
    clone.verify().unwrap();
}

#[test]
fn same_byte_atomic_file_replacement_is_detected() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("native.conf");
    std::fs::write(&file, "same bytes").unwrap();
    let (revision, _) = FileRevision::capture(&file).unwrap();
    crate::utils::atomic_write(&file, b"same bytes").unwrap();
    assert!(revision.verify().is_err());
    revision.verify_parents().unwrap();
    assert_eq!(std::fs::read_to_string(file).unwrap(), "same bytes");
}

#[test]
fn same_byte_parent_directory_replacement_is_detected() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("folder");
    std::fs::create_dir(&parent).unwrap();
    let file = parent.join("native.conf");
    std::fs::write(&file, "same bytes").unwrap();
    let (revision, _) = FileRevision::capture(&file).unwrap();
    let renamed = std::fs::rename(&parent, root.path().join("retained-folder"));
    if cfg!(windows)
        && renamed
            .as_ref()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied)
    {
        // Windows may prevent directory renaming while the retained child
        // handle is open. That also preserves the original physical source.
        revision.verify().unwrap();
        assert_eq!(std::fs::read_to_string(file).unwrap(), "same bytes");
        assert!(!root.path().join("retained-folder").exists());
        return;
    }
    renamed.unwrap();
    std::fs::create_dir(&parent).unwrap();
    std::fs::write(&file, "same bytes").unwrap();
    assert!(revision.verify().is_err());
    assert!(revision.verify_parents().is_err());
}

#[test]
fn missing_scope_capture_creates_nothing_and_tracks_creation() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("new/nested/native.conf");
    let (revision, bytes) = FileRevision::capture(&file).unwrap();
    assert!(bytes.is_none());
    assert!(!root.path().join("new").exists());
    revision.verify().unwrap();
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    revision.verify().unwrap();
    std::fs::write(&file, "created").unwrap();
    assert!(revision.verify().is_err());
}

#[test]
fn directory_targets_are_refused_without_reading_them_as_files() {
    let root = tempfile::tempdir().unwrap();
    assert!(FileRevision::capture(root.path()).is_err());
}

#[cfg(windows)]
#[test]
fn actual_windows_junction_retarget_preserves_both_equal_byte_targets() {
    fn junction(path: &Path, target: &Path) {
        let mut process = std::process::Command::new("powershell");
        crate::utils::configure_background_command(&mut process);
        let output = process
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:CCHUB_TEST_LINK -Target $env:CCHUB_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("CCHUB_TEST_LINK", path).env("CCHUB_TEST_TARGET", target)
            .output().unwrap();
        assert!(
            output.status.success(),
            "owned fixture junction creation failed"
        );
    }
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    let link = root.path().join("alias");
    for folder in [&first, &second] {
        std::fs::create_dir(folder).unwrap();
        std::fs::write(folder.join("native.conf"), "same bytes").unwrap();
    }
    junction(&link, &first);
    let target = link.join("native.conf");
    let canonical = crate::config_write::target_key(&target).unwrap();
    let (revision, _) = FileRevision::capture(&target).unwrap();
    std::fs::remove_dir(&link).unwrap();
    junction(&link, &second);
    assert_ne!(crate::config_write::target_key(&target).unwrap(), canonical);
    // The pinned physical source is still valid; configured aliases must also
    // check their canonical location before using this retained revision.
    revision.verify().unwrap();
    for folder in [&first, &second] {
        assert_eq!(
            std::fs::read_to_string(folder.join("native.conf")).unwrap(),
            "same bytes"
        );
    }
    std::fs::remove_dir(link).unwrap();
}
