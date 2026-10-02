//! Purge only the exact recovery snapshots shown in the confirmation preview.
//! Never recursively erase unknown files or expand a batch to newer entries.
use super::{archive, read_manifest_bytes, MAX_MANIFEST_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_TARGETS: usize = 20_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionPurgeTarget {
    pub key: String,
    pub revision: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PurgeFailureReason {
    Changed,
    Unsafe,
    RemoveFailed,
}

#[derive(Debug, Serialize)]
pub struct SessionPurgeFailure {
    pub key: String,
    pub reason: PurgeFailureReason,
}

#[derive(Debug, Default, Serialize)]
pub struct SessionPurgeResult {
    pub purged: Vec<String>,
    pub failed: Vec<SessionPurgeFailure>,
}

struct FileStamp {
    name: String,
    path: PathBuf,
    size: u64,
    modified: u128,
}

struct Snapshot {
    dir: PathBuf,
    files: Vec<FileStamp>,
    note: Vec<u8>,
}

impl Snapshot {
    fn revision(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(&self.note);
        for file in &self.files {
            hash.update(file.name.as_bytes());
            hash.update([0]);
            hash.update(file.size.to_le_bytes());
            hash.update(file.modified.to_le_bytes());
        }
        format!("{:x}", hash.finalize())
    }
}

fn inspect(trash: &Path, key: &str) -> Result<Snapshot, PurgeFailureReason> {
    use PurgeFailureReason::{Changed, Unsafe};
    if uuid::Uuid::parse_str(key)
        .map(|id| id.to_string())
        .as_deref()
        != Ok(key)
    {
        return Err(Unsafe);
    }
    for path in [trash.to_path_buf(), trash.join(key)] {
        let meta = fs::symlink_metadata(&path).map_err(|_| Changed)?;
        if !meta.is_dir() || archive::is_link(&meta) {
            return Err(Unsafe);
        }
    }
    let canonical_trash = trash.canonicalize().map_err(|_| Unsafe)?;
    let (source_dir, manifest, bytes) = read_manifest_bytes(trash, key).map_err(|_| Unsafe)?;
    let dir = canonical_trash.join(key);
    if source_dir.canonicalize().map_err(|_| Unsafe)? != dir
        || !matches!(
            manifest.session.state.as_str(),
            "prepared" | "recovery" | "deleted"
        )
        || manifest
            .items
            .iter()
            .enumerate()
            .any(|(index, item)| item.blob != format!("{index}.blob"))
    {
        return Err(Unsafe);
    }
    let mut expected = HashSet::from(["session.json".to_string()]);
    expected.extend(manifest.items.iter().map(|item| item.blob.clone()));
    let mut files = Vec::new();
    // Missing blobs after an interrupted/partly failed purge remain removable.
    // Unknown entries are refused, even when named like another owned blob.
    for entry in fs::read_dir(&dir).map_err(|_| Unsafe)? {
        let entry = entry.map_err(|_| Unsafe)?;
        let name = entry.file_name().to_str().ok_or(Unsafe)?.to_owned();
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| Changed)?;
        if !expected.contains(&name)
            || !meta.is_file()
            || archive::is_link(&meta)
            || !archive::confined(&path, &canonical_trash)
            || (name == "session.json" && meta.len() > MAX_MANIFEST_BYTES)
        {
            return Err(Unsafe);
        }
        files.push(FileStamp {
            name,
            path,
            size: meta.len(),
            modified: meta
                .modified()
                .map_err(|_| Unsafe)?
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Unsafe)?
                .as_nanos(),
        });
    }
    if !files.iter().any(|file| file.name == "session.json") {
        return Err(Changed);
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Snapshot {
        dir,
        files,
        note: bytes,
    })
}

pub(super) fn revision(trash: &Path, key: &str) -> Result<String, PurgeFailureReason> {
    inspect(trash, key).map(|snapshot| snapshot.revision())
}

pub(in super::super) fn purge(
    trash: &Path,
    targets: Vec<SessionPurgeTarget>,
) -> Result<SessionPurgeResult, String> {
    purge_with(trash, targets, |path| fs::remove_file(path))
}

fn purge_with(
    trash: &Path,
    targets: Vec<SessionPurgeTarget>,
    mut remove: impl FnMut(&Path) -> std::io::Result<()>,
) -> Result<SessionPurgeResult, String> {
    if targets.is_empty() || targets.len() > MAX_TARGETS {
        return Err("Invalid session trash selection".into());
    }
    let mut result = SessionPurgeResult::default();
    let mut seen = HashSet::new();
    for target in targets {
        if !seen.insert(target.key.clone()) {
            continue;
        }
        let outcome = (|| {
            if target.revision.len() != 64
                || !target.revision.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(PurgeFailureReason::Changed);
            }
            let mut snapshot = inspect(trash, &target.key)?;
            if snapshot.revision() != target.revision {
                return Err(PurgeFailureReason::Changed);
            }
            // Delete the note last so an interrupted purge remains listed and retryable.
            let mut files = snapshot
                .files
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>();
            files.sort_by_key(|path| path.file_name().is_some_and(|name| name == "session.json"));
            for path in files {
                // Recheck before each unlink, accounting only for our own earlier removals.
                if inspect(trash, &target.key)?.revision() != snapshot.revision() {
                    return Err(PurgeFailureReason::Changed);
                }
                if !archive::confined(&path, trash) {
                    return Err(PurgeFailureReason::Unsafe);
                }
                remove(&path).map_err(|_| PurgeFailureReason::RemoveFailed)?;
                snapshot.files.retain(|file| file.path != path);
            }
            if fs::remove_dir(&snapshot.dir).is_err() {
                // Keep a failed cleanup discoverable, even when every blob has
                // already gone. Never overwrite a newer note or follow a link.
                let note = snapshot.dir.join("session.json");
                if archive::owned_destination(&note, trash, true) {
                    if let Ok(mut output) = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&note)
                    {
                        let _ = output
                            .write_all(&snapshot.note)
                            .and_then(|_| output.sync_all());
                    }
                }
                return Err(PurgeFailureReason::RemoveFailed);
            }
            Ok(())
        })();
        match outcome {
            Ok(()) => result.purged.push(target.key),
            Err(reason) => result.failed.push(SessionPurgeFailure {
                key: target.key,
                reason,
            }),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
