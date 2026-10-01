use super::*;

fn pack(path: &Path, bytes: &[u8]) {
    fs::write(path, zstd::stream::encode_all(bytes, 0).unwrap()).unwrap();
}

#[test]
fn compressed_sessions_keep_logical_paths_and_prefer_the_plain_twin() {
    let root = tempfile::tempdir().unwrap();
    let plain = root.path().join("rollout.jsonl");
    let packed = twin(&plain).unwrap();
    fs::write(&plain, "plain\n").unwrap();
    pack(&packed, b"packed\n");
    assert_eq!(resolve(&packed).unwrap(), plain);
    assert_eq!(
        preferred_files(vec![packed.clone(), plain.clone()]),
        vec![plain.clone()]
    );
    fs::remove_file(&plain).unwrap();
    assert_eq!(resolve(&plain).unwrap(), packed);
    assert_eq!(logical_path(&packed), plain);
    assert_eq!(
        lines(&plain, 100)
            .unwrap()
            .collect::<io::Result<Vec<_>>>()
            .unwrap(),
        vec!["packed"]
    );
    assert!(!compressed(Path::new("config.zst")));
    assert!(compressed(Path::new("rollout.JSONL.ZST")));
}

#[test]
fn bounded_decoder_retains_exact_lines_and_accepts_concatenated_frames() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("rollout.jsonl.zst");
    let mut frames = zstd::stream::encode_all("第一行\r\n".as_bytes(), 0).unwrap();
    frames.extend(zstd::stream::encode_all(&b"last"[..], 0).unwrap());
    fs::write(&path, frames).unwrap();
    let bytes = "第一行\r\nlast".len() as u64;
    assert_eq!(
        lines(&path, bytes)
            .unwrap()
            .collect::<io::Result<Vec<_>>>()
            .unwrap(),
        vec!["第一行", "last"]
    );
    assert_eq!(tail(&path, bytes, 4).unwrap(), b"last");
    assert!(tail(&path, bytes - 1, 4).is_err());
}

#[test]
fn malformed_archives_invalid_text_and_large_lines_report_errors() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("rollout.jsonl.zst");
    for bytes in [
        b"not an archive".to_vec(),
        vec![0x28, 0xb5, 0x2f, 0xfd],
        zstd::stream::encode_all(&b"hello\n"[..], 0).unwrap()[..6].to_vec(),
    ] {
        fs::write(&path, bytes).unwrap();
        assert!(lines(&path, 100)
            .unwrap()
            .collect::<io::Result<Vec<_>>>()
            .is_err());
    }
    pack(&path, &[0xff, b'\n']);
    assert!(lines(&path, 100)
        .unwrap()
        .collect::<io::Result<Vec<_>>>()
        .is_err());
    pack(&path, &vec![b'x'; MAX_LINE_BYTES as usize + 1]);
    assert!(lines(&path, MAX_SESSION_BYTES)
        .unwrap()
        .collect::<io::Result<Vec<_>>>()
        .is_err());
    assert!(tail(&path, 128, 8).is_err());
}

#[test]
fn owned_paths_reject_traversal_missing_roots_and_directory_twins() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("rollout.jsonl");
    fs::write(&path, "hello").unwrap();
    assert!(confined(&path, root.path()));
    assert!(!confined(&root.path().join("../other.jsonl"), root.path()));
    assert!(!confined(&path, &root.path().join("missing")));
    assert!(owned_destination(
        &root.path().join("new/rollout.jsonl"),
        root.path(),
        true
    ));
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    pack(&twin(&path).unwrap(), b"packed");
    assert!(
        resolve(&path).is_err(),
        "a directory is not a missing plain file"
    );
}

#[cfg(windows)]
#[test]
fn descendant_links_are_not_owned_session_sources() {
    use std::os::windows::fs::symlink_file;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    let path = root.path().join("rollout.jsonl");
    // Windows permits file symlinks only with Developer Mode/elevation.
    match symlink_file(outside.path(), &path) {
        Ok(()) => {
            assert!(!confined(&path, root.path()));
            assert!(resolve(&path).is_err());
        }
        Err(error) if error.raw_os_error() == Some(1314) => {}
        Err(error) => panic!("{error}"),
    }
}
