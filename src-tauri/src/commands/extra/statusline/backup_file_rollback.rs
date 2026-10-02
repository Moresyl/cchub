use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

struct SavedFile {
    target: PathBuf,
    saved: Option<PathBuf>,
    permissions: Option<std::fs::Permissions>,
    desired: Option<PathBuf>,
    previous_desired: Option<PathBuf>,
}

/// Retains exact original bytes until the database installation succeeds.
/// This handles reported errors; it is not a cross-filesystem crash transaction.
pub(super) struct FileRollback {
    storage: Option<tempfile::TempDir>,
    files: Vec<SavedFile>,
    locations: HashMap<PathBuf, usize>,
    aliases: HashMap<PathBuf, usize>,
    touched: HashSet<PathBuf>,
    guards: HashMap<PathBuf, Option<Vec<u8>>>,
    new_parents: HashSet<PathBuf>,
    writes: usize,
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
            locations: HashMap::new(),
            aliases: HashMap::new(),
            touched: HashSet::new(),
            guards: HashMap::new(),
            new_parents: HashSet::new(),
            writes: 0,
            finished: false,
        })
    }

    pub(super) fn capture(&mut self, target: &Path) -> Result<(), String> {
        let location = crate::config_write::target_key(target)?;
        if let Some(index) = self.aliases.get(target) {
            if self.locations.get(&location) != Some(index) {
                return Err("恢复目标路径发生变化，恢复已停止".into());
            }
            return Ok(());
        }
        // Full native and skill backups intentionally overlap their snapshots.
        // Different path spellings must share one original and owned version.
        if let Some(index) = self.locations.get(&location) {
            self.aliases.insert(target.to_path_buf(), *index);
            return Ok(());
        }
        let (saved, permissions) = match std::fs::symlink_metadata(target) {
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
            desired: None,
            previous_desired: None,
        });
        self.locations.insert(location, self.files.len() - 1);
        self.aliases
            .insert(target.to_path_buf(), self.files.len() - 1);
        Ok(())
    }

    fn unchanged(file: &SavedFile) -> Result<bool, String> {
        Self::matches(&file.target, file.saved.as_deref())
    }

    fn matches(target: &Path, saved: Option<&Path>) -> Result<bool, String> {
        let current = crate::config_write::read(target)?;
        let expected = saved
            .map(std::fs::read)
            .transpose()
            .map_err(|_| "无法读取文件暂存内容")?;
        Ok(current == expected)
    }

    fn index(&self, target: &Path) -> Result<usize, String> {
        let location = crate::config_write::target_key(target)?;
        let index = self
            .locations
            .get(&location)
            .copied()
            .ok_or("恢复目标未暂存或路径发生变化")?;
        if self.aliases.get(target) != Some(&index) {
            return Err("恢复目标路径发生变化，恢复已停止".into());
        }
        Ok(index)
    }

    fn check_target(&self, target: &Path) -> Result<(), String> {
        let file = &self.files[self.index(target)?];
        if !Self::matches(target, file.desired.as_deref().or(file.saved.as_deref()))? {
            return Err("恢复目标在检查后发生变化，恢复已停止".into());
        }
        Ok(())
    }

    pub(super) fn verify(&self) -> Result<(), String> {
        for target in &self.touched {
            self.check_target(target)?;
        }
        for (target, original) in &self.guards {
            self.index(target)?;
            if crate::config_write::read(target)? != *original {
                return Err("恢复期间其他程序修改了配置，恢复已停止".into());
            }
        }
        Ok(())
    }

    // Intended bytes are staged before mutation. A failed atomic replacement may
    // leave the previous owned version, so both versions remain recoverable.
    pub(super) fn write(&mut self, target: &Path, bytes: &[u8]) -> Result<(), String> {
        self.write_with(target, bytes, crate::utils::atomic_write)
    }

    fn write_with(
        &mut self,
        target: &Path,
        bytes: &[u8],
        write: impl FnOnce(&Path, &[u8]) -> std::io::Result<()>,
    ) -> Result<(), String> {
        self.check_target(target)?;
        let index = self.index(target)?;
        let desired = self
            .storage
            .as_ref()
            .ok_or("恢复暂存目录不可用")?
            .path()
            .join(format!("desired-{index}-{}", self.writes));
        crate::utils::atomic_write(&desired, bytes).map_err(|_| "无法暂存恢复目标内容")?;
        self.writes += 1;
        // Staging a large file takes time; recheck immediately before replacing it.
        self.check_target(target)?;
        let file = &mut self.files[index];
        file.previous_desired = file.desired.replace(desired);
        self.touched.insert(file.target.clone());
        self.guards.remove(&file.target);
        write(target, bytes).map_err(|_| "无法写入恢复目标文件")?;
        file.previous_desired = None;
        if let Some(permissions) = &file.permissions {
            std::fs::set_permissions(target, permissions.clone())
                .map_err(|_| "无法保留恢复目标文件权限")?;
        }
        Ok(())
    }

    pub(super) fn apply_plan(&mut self, plan: crate::config_write::FilePlan) -> Result<(), String> {
        plan.check_targets()?;
        for update in &plan.updates {
            self.capture(&update.path)?;
            let file = &self.files[self.index(&update.path)?];
            let original = file
                .saved
                .as_ref()
                .map(std::fs::read)
                .transpose()
                .map_err(|_| "无法读取原始文件暂存内容")?;
            if original != update.original {
                return Err("恢复目标在准备后发生变化，恢复已停止".into());
            }
            self.check_target(&update.path)?;
        }
        for (target, original) in plan.guards {
            if crate::config_write::read(&target)? != original {
                return Err("恢复目标在准备后发生变化，恢复已停止".into());
            }
            self.capture(&target)?;
            self.guards
                .insert(self.files[self.index(&target)?].target.clone(), original);
        }
        for update in plan.updates {
            self.write(&update.path, &update.desired)?;
        }
        self.verify()
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
            if self.index(&file.target).is_err() {
                failed = true;
                continue;
            }
            let owned = file.desired.as_deref().is_some_and(|desired| {
                matches!(Self::matches(&file.target, Some(desired)), Ok(true))
            }) || file.previous_desired.as_deref().is_some_and(|desired| {
                matches!(Self::matches(&file.target, Some(desired)), Ok(true))
            });
            if !owned {
                // Preserve later external bytes, deletion, directories and links.
                failed = true;
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
            if std::fs::symlink_metadata(parent)
                .is_ok_and(|metadata| metadata.is_dir() && !super::backup_paths::is_link(&metadata))
            {
                let _ = std::fs::remove_dir(parent);
            }
        }
        if failed {
            if let Some(storage) = self.storage.take() {
                let manifest: Vec<_> = self.files.iter().map(|file| serde_json::json!({
                    "target": file.target, "original": file.saved.as_ref().and_then(|p| p.file_name()).map(|name| name.to_string_lossy()),
                    "desired": file.desired.as_ref().and_then(|p| p.file_name()).map(|name| name.to_string_lossy()),
                })).collect();
                if let Ok(bytes) = serde_json::to_vec_pretty(&manifest) {
                    let _ = crate::utils::atomic_write(
                        &storage.path().join("restore-map.json"),
                        &bytes,
                    );
                }
                let path = storage.keep();
                return format!(
                    "{original_error}；部分文件未能回滚，较新的外部修改已保留，原始文件暂存保留在 {}",
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
