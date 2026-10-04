//! Checked multi-file saves. Reported failures roll back only our own writes;
//! this is not an atomic transaction across processes or power loss.
use std::path::{Path, PathBuf};

mod plan;
mod revision;
pub(crate) use plan::location as target_key;
pub(crate) use plan::FilePlan;
pub(crate) use revision::FileRevision;

pub(crate) struct FileUpdate {
    pub path: PathBuf,
    pub original: Option<Vec<u8>>,
    pub desired: Vec<u8>,
}

pub(crate) fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => std::fs::read(path)
            .map(Some)
            .map_err(|_| "Cannot read configuration file".into()),
        Ok(_) => Err("Configuration target must be a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Cannot inspect configuration file".into()),
    }
}

struct Saved {
    update: FileUpdate,
    permissions: Option<std::fs::Permissions>,
    attempted: bool,
    binding: PathBuf,
    before: FileRevision,
    written: Option<FileRevision>,
}

fn verify_saved(file: &Saved) -> Result<(), String> {
    if target_key(&file.binding)? != file.update.path {
        return Err("Configuration source location changed while saving".into());
    }
    file.written.as_ref().unwrap_or(&file.before).verify()?;
    if read(&file.update.path)?.as_deref() != Some(&file.update.desired) {
        return Err("Configuration changed while saving; reload it and try again".into());
    }
    Ok(())
}

fn create_owned_parents(
    path: &Path,
    created: &mut Vec<revision::DirectoryRevision>,
) -> Result<(), String> {
    let mut missing = Vec::new();
    let mut parent = path.parent();
    while let Some(directory) = parent {
        if directory.exists() {
            break;
        }
        missing.push(directory.to_path_buf());
        parent = directory.parent();
    }
    for directory in missing.into_iter().rev() {
        match std::fs::create_dir(&directory) {
            Ok(()) => created.push(revision::DirectoryRevision::capture(&directory)?),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err("Cannot create configuration directory".into()),
        }
    }
    Ok(())
}

pub(crate) fn commit(updates: Vec<FileUpdate>) -> Result<(), String> {
    commit_then(updates, || Ok(()))
}

#[cfg(test)]
fn commit_with(
    updates: Vec<FileUpdate>,
    write: impl FnMut(usize, &Path, &[u8]) -> std::io::Result<()>,
) -> Result<(), String> {
    commit_with_finalizer(updates, write, || Ok(()))
}

// Keep recovery files until the associated database transaction also commits.
pub(crate) fn commit_then(
    updates: Vec<FileUpdate>,
    finalize: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    commit_then_with_revisions(updates, Vec::new(), finalize)
}

pub(crate) fn commit_then_with_revisions(
    updates: Vec<FileUpdate>,
    revisions: Vec<FileRevision>,
    finalize: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    commit_owned(
        updates,
        revisions,
        |_, path, bytes| match crate::utils::atomic_write_retained(path, bytes) {
            Ok(handle) => (Some(handle), Ok(())),
            Err(error) => (None, Err(error)),
        },
        finalize,
    )
}

#[cfg(test)]
fn commit_with_finalizer(
    updates: Vec<FileUpdate>,
    mut write: impl FnMut(usize, &Path, &[u8]) -> std::io::Result<()>,
    finalize: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    // Injected test writers can fail after writing. Production ownership comes
    // from the replacement handle itself, not from opening the path afterward.
    commit_owned(
        updates,
        Vec::new(),
        |index, path, bytes| {
            let result = write(index, path, bytes);
            (std::fs::File::open(path).ok(), result)
        },
        finalize,
    )
}

fn commit_owned(
    updates: Vec<FileUpdate>,
    revisions: Vec<FileRevision>,
    mut write: impl FnMut(usize, &Path, &[u8]) -> (Option<std::fs::File>, std::io::Result<()>),
    finalize: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    plan::check_distinct(updates.iter().map(|update| update.path.as_path()))?;
    let mut saved = Vec::new();
    let mut paths = std::collections::HashSet::new();
    for mut update in updates {
        if !paths.insert(update.path.clone()) {
            return Err("Duplicate configuration save target".into());
        }
        if read(&update.path)? != update.original {
            return Err("Configuration changed externally; reload it and try again".into());
        }
        let binding = update.path.clone();
        let (captured, bytes) = FileRevision::capture(&binding)?;
        let before = if let Some(revision) = revisions
            .iter()
            .find(|revision| revision.path() == captured.path())
        {
            revision.verify()?;
            revision.clone()
        } else {
            captured
        };
        if bytes != update.original {
            return Err("Configuration changed externally; reload it and try again".into());
        }
        update.path = before.path().into();
        let permissions = if update.original.is_some() {
            Some(
                std::fs::metadata(&update.path)
                    .map_err(|_| "Cannot inspect configuration permissions")?
                    .permissions(),
            )
        } else {
            None
        };
        saved.push(Saved {
            update,
            permissions,
            attempted: false,
            binding,
            before,
            written: None,
        });
    }
    if saved
        .iter()
        .all(|file| file.update.original.as_deref() == Some(&file.update.desired))
    {
        for file in &saved {
            verify_saved(file)?;
        }
        return finalize();
    }
    let storage = tempfile::Builder::new()
        .prefix("cchub-config-recovery-")
        .tempdir()
        .map_err(|_| "Cannot prepare configuration recovery files")?;
    // Retain original bytes before the first write, including opaque credentials.
    for (index, file) in saved.iter().enumerate() {
        if let Some(bytes) = &file.update.original {
            crate::utils::atomic_write(&storage.path().join(index.to_string()), bytes)
                .map_err(|_| "Cannot stage original configuration")?;
        }
    }
    let mut new_parents = Vec::new();
    let result = (|| {
        for (index, file) in saved.iter_mut().enumerate() {
            file.before.verify()?;
            if target_key(&file.binding)? != file.update.path {
                return Err(
                    "Configuration source location changed; reload before continuing".into(),
                );
            }
            if read(&file.update.path)? != file.update.original {
                return Err("Configuration changed externally; reload it and try again".to_string());
            }
            if file.update.original.as_deref() == Some(&file.update.desired) {
                continue;
            }
            create_owned_parents(&file.update.path, &mut new_parents)?;
            file.before.verify()?;
            file.attempted = true;
            let (handle, result) = write(index, &file.update.path, &file.update.desired);
            if let Some(handle) = handle {
                file.written = Some(file.before.written(handle)?);
            }
            result.map_err(|_| "Cannot save configuration; the file group will be rolled back")?;
            file.written
                .as_ref()
                .ok_or("Cannot verify configuration write ownership")?
                .verify()?;
            if let Some(permissions) = &file.permissions {
                std::fs::set_permissions(&file.update.path, permissions.clone())
                    .map_err(|_| "Cannot preserve configuration permissions")?;
            }
        }
        for file in &saved {
            verify_saved(file)?;
        }
        finalize()
    })();
    let Err(error) = result else {
        return Ok(());
    };
    let mut failed = false;
    for file in saved.iter().rev().filter(|file| file.attempted) {
        let Ok(current) = read(&file.update.path) else {
            failed = true;
            continue;
        };
        if current == file.update.original {
            continue;
        }
        if file
            .written
            .as_ref()
            .is_none_or(|owned| owned.verify().is_err())
        {
            failed = true;
            continue;
        }
        if current.as_deref() != Some(&file.update.desired) {
            // A newer edit belongs to another writer. Never restore over it.
            failed = true;
            continue;
        }
        let restored = if let Some(original) = &file.update.original {
            crate::utils::atomic_write(&file.update.path, original).and_then(|()| {
                if let Some(permissions) = &file.permissions {
                    std::fs::set_permissions(&file.update.path, permissions.clone())?;
                }
                Ok(())
            })
        } else {
            std::fs::remove_file(&file.update.path)
        };
        failed |= restored.is_err();
    }
    // Release replacement handles before deleting newly created empty parents.
    for file in &mut saved {
        file.written = None;
    }
    for parent in new_parents.into_iter().rev() {
        parent.remove_if_unchanged();
    }
    if failed {
        let map: Vec<_> = saved
            .iter()
            .enumerate()
            .map(|(index, file)| {
                serde_json::json!({
                    "target": file.update.path,
                    "original": file.update.original.as_ref().map(|_| index.to_string()),
                })
            })
            .collect();
        if let Ok(bytes) = serde_json::to_vec_pretty(&map) {
            let _ = crate::utils::atomic_write(&storage.path().join("restore-map.json"), &bytes);
        }
        let path = storage.keep();
        return Err(format!(
            "{error}; recovery is incomplete, newer edits were preserved. Original files: {}",
            path.display()
        ));
    }
    Err(error)
}

#[cfg(test)]
mod tests;
