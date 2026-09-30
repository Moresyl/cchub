use std::path::{Path, PathBuf};

use base64::Engine;

const INVALID_PATH: &str = "备份包含无效或越界的文件路径，恢复已停止";

pub(super) fn relative_path(value: &str, allow_empty: bool) -> Result<PathBuf, String> {
    if value.is_empty() {
        return if allow_empty {
            Ok(PathBuf::new())
        } else {
            Err(INVALID_PATH.into())
        };
    }
    let mut path = PathBuf::new();
    for part in value.split('/') {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|tail| {
                    matches!(
                        tail,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            });
        if part.is_empty()
            || part == "."
            || part == ".."
            || reserved
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || "\\:*?\"<>|".contains(c))
        {
            return Err(INVALID_PATH.into());
        }
        path.push(part);
    }
    Ok(path)
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Includes junctions, which may not be reported as a symbolic link.
        return metadata.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    false
}

pub(super) fn confined_target(
    root: &Path,
    value: &str,
    allow_empty: bool,
) -> Result<PathBuf, String> {
    let relative = relative_path(value, allow_empty)?;
    // The chosen root itself may be a user-configured link. Descendant links
    // must not redirect a restore outside that root (or onto another file).
    let root = if root.exists() {
        root.canonicalize()
            .map_err(|_| "无法读取恢复目标目录".to_string())?
    } else {
        root.to_path_buf()
    };
    let mut target = root;
    for component in relative.components() {
        target.push(component);
        match std::fs::symlink_metadata(&target) {
            Ok(metadata) if is_link(&metadata) => {
                return Err("恢复目标包含符号链接或重解析点，请检查后重试".into())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("无法检查恢复目标路径".into()),
        }
    }
    Ok(target)
}

pub(super) fn decode_file(value: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| "备份文件内容损坏，恢复已停止".into())
}

pub(super) fn validate_artifact_paths(conn: &rusqlite::Connection) -> Result<(), String> {
    let mut skills = conn
        .prepare("SELECT tool_id, name FROM _skill_files")
        .map_err(|e| e.to_string())?;
    let rows = skills
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (tool, name) = row.map_err(|e| e.to_string())?;
        if tool != "claude-settings"
            && !super::backups_restore::TOOL_BACKUP_IDS.contains(&tool.as_str())
        {
            return Err("备份包含不支持的技能工具".into());
        }
        let path = relative_path(&name, false)?;
        if path.components().count() != 1 {
            return Err(INVALID_PATH.into());
        }
    }
    let mut files = conn
        .prepare("SELECT root_key, relative_path, content_base64 FROM _backup_files")
        .map_err(|e| e.to_string())?;
    let rows = files
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (root, path, content) = row.map_err(|e| e.to_string())?;
        let standalone = root == "claude_mcp" || root.starts_with("claude-desktop:");
        if let Some(tool) = root
            .strip_prefix("tooldir:")
            .or_else(|| root.strip_prefix("skillsdir:"))
        {
            if !super::backups_restore::TOOL_BACKUP_IDS.contains(&tool) {
                return Err(INVALID_PATH.into());
            }
        } else if let Some(key) = root.strip_prefix("claude-desktop:") {
            if !super::backups_restore::CLAUDE_DESKTOP_BACKUP_KEYS.contains(&key) {
                return Err(INVALID_PATH.into());
            }
        } else if let Some(project) = root.strip_prefix("project:") {
            if project.trim().is_empty() || project.chars().any(char::is_control) {
                return Err(INVALID_PATH.into());
            }
        } else if root != "claude_mcp" {
            return Err(INVALID_PATH.into());
        }
        relative_path(&path, standalone)?;
        if standalone && !path.is_empty() {
            return Err(INVALID_PATH.into());
        }
        decode_file(&content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
