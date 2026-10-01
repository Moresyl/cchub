//! Recoverable Codex file deletion. Shared client indexes/databases stay owned.
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::super::config_profiles::session_roots_for_tool;
use super::super::types::SessionDeleteTarget;
use crate::shared::session_archive as archive;

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashedSession {
    pub key: String,
    pub session_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub source_path: String,
    pub deleted_at: String,
    pub file_count: usize,
    pub state: String,
}

#[derive(Serialize, Deserialize)]
struct Item {
    path: PathBuf,
    blob: String,
    hash: String,
    size: u64,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    session: TrashedSession,
    items: Vec<Item>,
}

pub(super) fn directory() -> Result<PathBuf, String> {
    Ok(super::super::statusline::managed_backups_dir()?.join("session-trash"))
}

fn digest(path: &Path) -> Result<(String, u64), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || archive::is_link(&metadata) {
        return Err("Session data must be a regular file".into());
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut reader = file.take(archive::MAX_SESSION_BYTES + 1);
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let size = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if size == 0 {
            break;
        }
        count += size as u64;
        if count > archive::MAX_SESSION_BYTES {
            return Err("Session exceeds the recovery size limit".into());
        }
        hash.update(&chunk[..size]);
    }
    Ok((format!("{:x}", hash.finalize()), count))
}

fn session_id(path: &Path) -> Result<String, String> {
    session_id_exact(path, archive::compressed(path))
}

fn session_id_exact(path: &Path, packed: bool) -> Result<String, String> {
    for line in archive::reader_lines(
        archive::open_exact(path, packed, archive::MAX_SESSION_BYTES).map_err(|e| e.to_string())?,
    )
    .take(120)
    {
        let line = line.map_err(|e| e.to_string())?;
        let Ok(row) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if row.get("type").and_then(|v| v.as_str()) == Some("session_meta") {
            if let Some(id) = row
                .pointer("/payload/id")
                .and_then(|v| v.as_str())
                .filter(|id| !id.is_empty())
            {
                return Ok(id.to_owned());
            }
        }
    }
    Err("Cannot verify the session identity".into())
}

fn session_title(path: &Path) -> Option<String> {
    let lines = archive::lines(path, 16 * 1024 * 1024).ok()?;
    for line in lines.take(120) {
        let line = line.ok()?;
        let Ok(row) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let explicit = row
            .pointer("/payload/title")
            .or_else(|| row.get("title"))
            .and_then(|value| value.as_str());
        let user = if row.get("type").and_then(|value| value.as_str()) == Some("response_item")
            && row
                .pointer("/payload/role")
                .and_then(|value| value.as_str())
                == Some("user")
        {
            Some(super::codex_message_content(
                row.pointer("/payload/content"),
            ))
        } else if row
            .pointer("/payload/type")
            .and_then(|value| value.as_str())
            == Some("user_message")
        {
            row.pointer("/payload/message")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        } else {
            None
        };
        if let Some(text) = explicit.or(user.as_deref()) {
            let title = text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .filter(|character| !character.is_control())
                .take(160)
                .collect::<String>();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    None
}

fn save(dir: &Path, manifest: &Manifest) -> Result<(), String> {
    let bytes = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
    crate::utils::atomic_write(&dir.join("session.json"), &bytes).map_err(|e| e.to_string())
}

pub(super) fn delete(
    target: &SessionDeleteTarget,
    roots: &[PathBuf],
    trash: &Path,
) -> Result<TrashedSession, String> {
    delete_with(target, roots, trash, |path| fs::remove_file(path))
}

fn delete_with(
    target: &SessionDeleteTarget,
    roots: &[PathBuf],
    trash: &Path,
    mut remove: impl FnMut(&Path) -> std::io::Result<()>,
) -> Result<TrashedSession, String> {
    if target.tool_id != "codex" || target.source_backend != "jsonl" {
        return Err("Unsupported recoverable session source".into());
    }
    let source = archive::resolve(Path::new(&target.source_path)).map_err(|e| e.to_string())?;
    let logical = archive::logical_path(&source);
    let mut paths = Vec::new();
    for path in [
        logical.clone(),
        archive::twin(&logical).ok_or("Unsupported session file")?,
    ] {
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.to_string()),
            Ok(meta) => {
                if !meta.is_file()
                    || archive::is_link(&meta)
                    || !roots.iter().any(|root| archive::confined(&path, root))
                {
                    return Err("Invalid session source path".into());
                }
                // Read the specific form, rather than resolve()'s preferred twin.
                // Plain data and packed data are validated after making snapshots.
                if meta
                    .modified()
                    .map_err(|e| e.to_string())?
                    .elapsed()
                    .unwrap_or(Duration::ZERO)
                    < Duration::from_secs(60)
                {
                    return Err("会话文件仍在使用或刚刚更新，请稍后重试".into());
                }
                paths.push((path, meta.modified().map_err(|e| e.to_string())?));
            }
        }
    }
    if paths.is_empty() {
        return Err("Session file is no longer present".into());
    }
    if session_id(&source)? != target.session_id {
        return Err("Session identity does not match the selected source".into());
    }
    fs::create_dir_all(trash).map_err(|e| e.to_string())?;
    let canonical_trash = trash.canonicalize().map_err(|e| e.to_string())?;
    if roots.iter().any(|root| {
        canonical_trash.starts_with(root.canonicalize().unwrap_or_else(|_| root.clone()))
    }) {
        return Err("Recovery directory must be outside session roots".into());
    }
    let key = uuid::Uuid::new_v4().to_string();
    let dir = canonical_trash.join(&key);
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    let mut manifest = Manifest {
        session: TrashedSession {
            key,
            session_id: target.session_id.clone(),
            title: session_title(&source),
            source_path: source.to_string_lossy().into_owned(),
            deleted_at: chrono::Utc::now().to_rfc3339(),
            file_count: paths.len(),
            state: "prepared".into(),
        },
        items: Vec::new(),
    };
    for (index, (path, _)) in paths.iter().enumerate() {
        let blob = format!("{index}.blob");
        let (hash, size) = digest(path)?;
        let destination = dir.join(&blob);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|e| e.to_string())?;
        let mut input = File::open(path)
            .map_err(|e| e.to_string())?
            .take(archive::MAX_SESSION_BYTES + 1);
        let copied = std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        if copied != size || digest(&destination)? != (hash.clone(), size) {
            return Err("Session changed while preparing recovery".into());
        }
        // Validate each twin's own metadata, including a mismatched packed file.
        if session_id_exact(&destination, archive::compressed(path))? != target.session_id {
            return Err("Session twin identity does not match".into());
        }
        manifest.items.push(Item {
            path: path.clone(),
            blob,
            hash,
            size,
        });
    }
    save(&dir, &manifest)?;
    for (item, (_, modified)) in manifest.items.iter().zip(&paths) {
        let still_owned = roots.iter().any(|root| archive::confined(&item.path, root));
        let unchanged =
            fs::metadata(&item.path).is_ok_and(|m| m.modified().ok() == Some(*modified));
        let result = (|| {
            if !still_owned || !unchanged || digest(&item.path)? != (item.hash.clone(), item.size) {
                return Err("Session changed before deletion".to_string());
            }
            remove(&item.path).map_err(|e| e.to_string())
        })();
        if let Err(error) = result {
            manifest.session.state = "recovery".into();
            let _ = save(&dir, &manifest);
            return Err(format!(
                "{error}; 原始文件已保留，可在最近删除中恢复（{}）",
                manifest.session.key
            ));
        }
    }
    manifest.session.state = "deleted".into();
    save(&dir, &manifest)
        .map_err(|e| format!("{e}; Recovery files retained at {}", dir.display()))?;
    Ok(manifest.session)
}

fn read_manifest(trash: &Path, key: &str) -> Result<(PathBuf, Manifest), String> {
    if uuid::Uuid::parse_str(key).is_err() || key.contains(['/', '\\']) {
        return Err("Invalid recovery key".into());
    }
    let dir = trash.join(key);
    let path = dir.join("session.json");
    if !archive::confined(&path, trash) {
        return Err("Invalid recovery manifest path".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("Recovery manifest is too large".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| "Recovery manifest is damaged")?;
    if manifest.session.key != key
        || manifest.items.is_empty()
        || manifest.items.len() > 2
        || manifest.session.file_count != manifest.items.len()
    {
        return Err("Recovery manifest is damaged".into());
    }
    Ok((dir, manifest))
}

pub(super) fn list(trash: &Path) -> Result<Vec<TrashedSession>, String> {
    let entries = match fs::read_dir(trash) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut sessions = Vec::new();
    for entry in entries.flatten().take(20_000) {
        let Some(key) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if let Ok((_, manifest)) = read_manifest(trash, &key) {
            if manifest.session.state != "restored" {
                sessions.push(manifest.session);
            }
        }
    }
    sessions.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(sessions)
}

pub(super) fn restore(trash: &Path, key: &str, roots: &[PathBuf]) -> Result<(), String> {
    let (dir, mut manifest) = read_manifest(trash, key)?;
    let mut destinations = std::collections::HashSet::new();
    let logical = archive::logical_path(Path::new(&manifest.session.source_path));
    for (index, item) in manifest.items.iter().enumerate() {
        if !archive::jsonl(&item.path)
            || archive::logical_path(&item.path) != logical
            || item.blob != format!("{index}.blob")
            || item.size > archive::MAX_SESSION_BYTES
            || !destinations.insert(item.path.clone())
            || !roots
                .iter()
                .any(|root| archive::owned_destination(&item.path, root, true))
        {
            return Err("Invalid recovery target".into());
        }
        let blob = dir.join(&item.blob);
        if !archive::confined(&blob, trash) || digest(&blob)? != (item.hash.clone(), item.size) {
            return Err("Recovery file is damaged".into());
        }
        if session_id_exact(&blob, archive::compressed(&item.path))? != manifest.session.session_id
        {
            return Err("Recovery session identity does not match".into());
        }
        match fs::symlink_metadata(&item.path) {
            Ok(_) if digest(&item.path)? != (item.hash.clone(), item.size) => {
                return Err("恢复位置已有不同内容，已保留现有文件".into())
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    for item in &manifest.items {
        if item.path.exists() {
            if digest(&item.path)? != (item.hash.clone(), item.size) {
                return Err("恢复位置已有不同内容，已保留现有文件".into());
            }
            continue;
        }
        if !roots
            .iter()
            .any(|root| archive::owned_destination(&item.path, root, true))
        {
            return Err("Recovery target changed".into());
        }
        let parent = item.path.parent().ok_or("Invalid recovery target")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let blob = dir.join(&item.blob);
        if !archive::confined(&blob, trash) {
            return Err("Recovery file changed".into());
        }
        let copied = std::io::copy(
            &mut File::open(&blob)
                .map_err(|e| e.to_string())?
                .take(archive::MAX_SESSION_BYTES + 1),
            temp.as_file_mut(),
        )
        .map_err(|e| e.to_string())?;
        temp.as_file_mut().flush().map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        if copied != item.size || digest(temp.path())? != (item.hash.clone(), item.size) {
            return Err("Recovery file changed; 原始文件仍保留，可重试恢复".into());
        }
        if !roots
            .iter()
            .any(|root| archive::owned_destination(&item.path, root, true))
        {
            return Err("Recovery target changed".into());
        }
        temp.persist_noclobber(&item.path)
            .map_err(|e| format!("{}; 原始文件仍保留，可重试恢复", e.error))?;
    }
    manifest.session.state = "restored".into();
    save(&dir, &manifest)
}

pub(super) fn delete_from_conn(
    conn: &rusqlite::Connection,
    target: &SessionDeleteTarget,
) -> Result<TrashedSession, String> {
    delete(
        target,
        &session_roots_for_tool(conn, "codex")?,
        &directory()?,
    )
}

#[cfg(test)]
mod tests;
