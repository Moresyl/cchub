use crate::config_write::{self, FileUpdate};
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAX_SETTINGS_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn config_path(conn: &Connection, tool: &str) -> Result<PathBuf, String> {
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT config_dir, mcp_config_path FROM custom_paths WHERE tool_id = ?1",
            [tool],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| "Cannot read configured settings location")?;
    let (directory, mcp) = row.unwrap_or_default();
    let file = match tool {
        "claude" => "settings.json",
        "codex" => "config.toml",
        _ => return Err("Unsupported settings tool".into()),
    };
    if let Some(directory) = directory.filter(|value| !value.trim().is_empty()) {
        return Ok(PathBuf::from(directory).join(file));
    }
    // Claude's MCP file is separate from its settings directory.
    if tool == "codex" {
        if let Some(mcp) = mcp.filter(|value| !value.trim().is_empty()) {
            return PathBuf::from(mcp)
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(|parent| parent.join(file))
                .ok_or_else(|| "Invalid configured settings location".into());
        }
    }
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(home.join(format!(".{tool}")).join(file))
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
