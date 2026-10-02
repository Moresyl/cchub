use crate::config_write::{self, FileUpdate};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAX_SETTINGS_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn config_path(conn: &Connection, tool: &str) -> Result<PathBuf, String> {
    if !matches!(tool, "claude" | "codex") {
        return Err("Unsupported settings tool".into());
    }
    crate::commands::extra_commands::resolve_tool_config_path(conn, tool)
}

pub(super) fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.len() > MAX_SETTINGS_BYTES as u64 => {
            return Err("Configuration file exceeds the settings size limit".into());
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err("Cannot inspect configuration file".into());
        }
        _ => {}
    }
    let bytes = config_write::read(path)?;
    if bytes
        .as_ref()
        .is_some_and(|bytes| bytes.len() > MAX_SETTINGS_BYTES)
    {
        return Err("Configuration file exceeds the settings size limit".into());
    }
    Ok(bytes)
}

pub(super) fn text<'a>(bytes: Option<&'a [u8]>, default: &'a str) -> Result<&'a str, String> {
    match bytes {
        Some(bytes) => {
            std::str::from_utf8(bytes).map_err(|_| "Configuration must contain valid UTF-8".into())
        }
        None => Ok(default),
    }
}

pub(super) fn revision(path: &Path, bytes: Option<&[u8]>) -> String {
    let mut digest = Sha256::new();
    digest.update(b"cchub-tool-settings-v1");
    let location = path.to_string_lossy();
    digest.update((location.len() as u64).to_le_bytes());
    digest.update(location.as_bytes());
    digest.update([u8::from(bytes.is_some())]);
    if let Some(bytes) = bytes {
        digest.update(bytes);
    }
    format!("{:x}", digest.finalize())
}

pub(super) fn check_revision(
    path: &Path,
    bytes: Option<&[u8]>,
    expected: Option<&str>,
) -> Result<(), String> {
    if expected.is_some_and(|expected| expected != revision(path, bytes)) {
        return Err("Configuration or its location changed; reload before saving".into());
    }
    Ok(())
}

pub(super) fn save(path: &Path, original: Option<Vec<u8>>, desired: String) -> Result<(), String> {
    config_write::commit(vec![FileUpdate {
        path: path.to_path_buf(),
        original,
        desired: desired.into_bytes(),
    }])
}
