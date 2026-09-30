use std::collections::HashSet;
use std::path::{Path, PathBuf};

struct SavedFile {
    target: PathBuf,
    saved: Option<PathBuf>,
    permissions: Option<std::fs::Permissions>,
}

/// Retains exact original bytes until the database installation succeeds.
/// This handles reported errors; it is not a cross-filesystem crash transaction.
pub(super) struct FileRollback {
    storage: Option<tempfile::TempDir>,
    files: Vec<SavedFile>,
    captured: HashSet<PathBuf>,
    touched: HashSet<PathBuf>,
    new_parents: HashSet<PathBuf>,
    finished: bool,
}

impl FileRollback {
    pub(super) fn new(directory: &Path) -> Result<Self, String> {
        Ok(Self {
            storage: Some(
                tempfile::Builder::new()
                    .prefix("cchub-file-rollback-")
                    .tempdir_in(directory)
                    .map_err(|_| "无法创建文件恢复暂存目录".to_string())?,
            ),
            files: Vec::new(),
            captured: HashSet::new(),
            touched: HashSet::new(),
            new_parents: HashSet::new(),
            finished: false,
        })
    }

    pub(super) fn capture(&mut self, target: &Path) -> Result<(), String> {
        if self.captured.contains(target) {
            return Ok(());
        }
        let (saved, permissions) = match std::fs::metadata(target) {
            Ok(metadata) if metadata.is_file() => {
                let saved = self
                    .storage
                    .as_ref()
                    .ok_or("恢复暂存目录不可用")?
                    .path()
                    .join(self.files.len().to_string());
                std::fs::copy(target, &saved)
                    .map_err(|_| "无法暂存恢复目标的原始文件".to_string())?;
                #[cfg(windows)]
                if metadata.permissions().readonly() {
                    let mut permissions = metadata.permissions();
                    permissions.set_readonly(false);
                    std::fs::set_permissions(&saved, permissions)
                        .map_err(|_| "无法准备原始文件暂存内容")?;
                }
                Some((saved, metadata.permissions()))
            }
            Ok(_) => return Err("备份恢复目标不是普通文件".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err("无法读取恢复目标的原始文件".into()),
        }
        .map_or((None, None), |(saved, permissions)| {
            (Some(saved), Some(permissions))
        });
        let mut parent = target.parent();
        while let Some(path) = parent {
            if path.exists() {
                break;
            }
            self.new_parents.insert(path.to_path_buf());
            parent = path.parent();
        }
        self.files.push(SavedFile {
            target: target.to_path_buf(),
            saved,
            permissions,
        });
        self.captured.insert(target.to_path_buf());
        Ok(())
    }

    fn unchanged(file: &SavedFile) -> Result<bool, String> {
        match (&file.saved, std::fs::read(&file.target)) {
            (Some(saved), Ok(bytes)) => {
                Ok(bytes == std::fs::read(saved).map_err(|_| "无法读取原始文件暂存内容")?)
            }
            (None, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
            (_, Ok(_)) => Ok(false),
            (Some(_), Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            _ => Err("无法检查恢复目标是否已变化".into()),
        }
    }

    pub(super) fn before_write(&mut self, targets: &[PathBuf]) -> Result<(), String> {
        // Check the complete tool group before allowing its first write.
        for target in targets {
            let file = self
                .files
                .iter()
                .find(|file| &file.target == target)
                .ok_or("恢复目标未暂存")?;
            if !self.touched.contains(target) && !Self::unchanged(file)? {
                return Err("恢复目标在检查后发生变化，恢复已停止".into());
            }
        }
        self.touched.extend(targets.iter().cloned());
        Ok(())
    }

    pub(super) fn commit(&mut self) {
        self.finished = true;
    }

    pub(super) fn rollback(&mut self, original_error: String) -> String {
        self.finished = true;
        let mut failed = false;
        for file in self
            .files
            .iter()
            .rev()
            .filter(|file| self.touched.contains(&file.target))
        {
            if matches!(Self::unchanged(file), Ok(true)) {
                continue;
            }
            let result = if let Some(saved) = &file.saved {
                std::fs::read(saved)
                    .and_then(|bytes| crate::utils::atomic_write(&file.target, &bytes))
                    .and_then(|()| {
                        if let Some(permissions) = &file.permissions {
                            std::fs::set_permissions(&file.target, permissions.clone())?;
                        }
                        Ok(())
                    })
            } else {
                match std::fs::remove_file(&file.target) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    result => result,
                }
            };
            failed |= result.is_err();
        }
        let mut parents: Vec<_> = self.new_parents.iter().collect();
        parents.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for parent in parents {
            let _ = std::fs::remove_dir(parent);
        }
        if failed {
            if let Some(storage) = self.storage.take() {
                let manifest: Vec<_> = self.files.iter().map(|file| serde_json::json!({
                    "target": file.target, "original": file.saved.as_ref().and_then(|p| p.file_name()).map(|name| name.to_string_lossy()),
                })).collect();
                if let Ok(bytes) = serde_json::to_vec_pretty(&manifest) {
                    let _ = crate::utils::atomic_write(
                        &storage.path().join("restore-map.json"),
                        &bytes,
                    );
                }
                let path = storage.keep();
                return format!(
                    "{original_error}；部分文件未能回滚，原始文件暂存保留在 {}",
                    path.display()
                );
            }
        }
        original_error
    }
}

impl Drop for FileRollback {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.rollback("恢复被中断".into());
        }
    }
}

#[cfg(test)]
mod tests;
