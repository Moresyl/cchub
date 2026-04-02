use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use zip::read::ZipArchive;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBackup {
    pub id: String,
    pub skill_name: String,
    pub original_path: String,
    pub backup_path: String,
    pub created_at: String,
    pub size_bytes: u64,
}

fn backup_root_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(home.join(".cchub").join("skill-backups"))
}

fn sanitize_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        "skill".to_string()
    } else {
        sanitized.to_string()
    }
}

fn backup_meta_path_for_id(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() && path.symlink_metadata().is_err() {
        return Ok(());
    }

    let metadata = path.symlink_metadata().map_err(|e| e.to_string())?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    } else {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn load_skill_backup_from_meta(path: &Path) -> Result<SkillBackup, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn prune_skill_backups(dir: &Path) -> Result<(), String> {
    let mut backups = list_skill_backups()?;
    if backups.len() <= 20 {
        return Ok(());
    }

    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    for backup in backups.into_iter().skip(20) {
        let backup_path = PathBuf::from(&backup.backup_path);
        let meta_path = backup_meta_path_for_id(dir, &backup.id);
        let _ = remove_path_if_exists(&backup_path);
        let _ = remove_path_if_exists(&meta_path);
    }
    Ok(())
}

pub fn list_skill_backups() -> Result<Vec<SkillBackup>, String> {
    let dir = backup_root_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        if let Ok(backup) = load_skill_backup_from_meta(&path) {
            backups.push(backup);
        }
    }

    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(backups)
}

pub fn create_skill_backup(path: &Path) -> Result<SkillBackup, String> {
    if !path.exists() {
        return Err(format!("File does not exist: {}", path.display()));
    }

    let dir = backup_root_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "skill.md".to_string());
    let skill_name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().replace(".disabled", ""))
        .unwrap_or_else(|| "skill".to_string());
    let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    let id = format!("{}-{}", stamp, sanitize_name(&skill_name));
    let backup_path = dir.join(format!("{}--{}", id, file_name));

    std::fs::copy(path, &backup_path)
        .map_err(|e| format!("Failed to back up {}: {}", path.display(), e))?;

    let size_bytes = std::fs::metadata(&backup_path)
        .map_err(|e| e.to_string())?
        .len();

    let backup = SkillBackup {
        id: id.clone(),
        skill_name,
        original_path: path.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        size_bytes,
    };

    let meta_path = backup_meta_path_for_id(&dir, &id);
    crate::utils::atomic_write_string(
        &meta_path,
        &serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    prune_skill_backups(&dir)?;
    Ok(backup)
}

pub fn restore_skill_backup(id: &str, target_path: Option<&str>) -> Result<String, String> {
    let dir = backup_root_dir()?;
    let meta_path = backup_meta_path_for_id(&dir, id);
    let backup = load_skill_backup_from_meta(&meta_path)?;
    let backup_path = PathBuf::from(&backup.backup_path);
    if !backup_path.exists() {
        return Err(format!("Backup file not found: {}", backup.backup_path));
    }

    let target = target_path
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&backup.original_path));

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::copy(&backup_path, &target).map_err(|e| format!("Failed to restore backup: {}", e))?;
    Ok(target.to_string_lossy().to_string())
}

pub fn delete_skill_backup(id: &str) -> Result<(), String> {
    let dir = backup_root_dir()?;
    let meta_path = backup_meta_path_for_id(&dir, id);
    let backup = load_skill_backup_from_meta(&meta_path)?;
    remove_path_if_exists(Path::new(&backup.backup_path))?;
    remove_path_if_exists(&meta_path)?;
    Ok(())
}

fn normalize_skill_target_name(source_path: &Path) -> String {
    if source_path.extension().and_then(|ext| ext.to_str()) == Some("skill") {
        format!(
            "{}.md",
            source_path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| "skill".to_string())
        )
    } else {
        source_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "skill.md".to_string())
    }
}

fn extract_skill_archive(source_path: &Path, target_skills_dir: &Path) -> Result<String, String> {
    let file = std::fs::File::open(source_path)
        .map_err(|e| format!("Failed to open archive {}: {}", source_path.display(), e))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Failed to read archive {}: {}", source_path.display(), e))?;
    let mut primary_installed_path: Option<String> = None;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let Some(entry_path) = entry.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };
        if entry_path.as_os_str().is_empty() {
            continue;
        }

        let destination = target_skills_dir.join(&entry_path);
        if entry.is_dir() {
            std::fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        remove_path_if_exists(&destination)?;
        let mut output = std::fs::File::create(&destination).map_err(|e| e.to_string())?;
        io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;

        if primary_installed_path.is_none()
            && destination.extension().and_then(|ext| ext.to_str()) == Some("md")
        {
            primary_installed_path = Some(destination.to_string_lossy().to_string());
        }
    }

    primary_installed_path.ok_or_else(|| "Archive did not contain any .md skill files".to_string())
}

/// Create a symbolic link (platform-specific)
#[cfg(windows)]
fn create_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(src, dst)
}

#[cfg(unix)]
fn create_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

/// Install a skill file to a target tool's skills directory
/// method: "symlink" or "copy" (default)
pub fn install_skill_file(
    source: &str,
    target_skills_dir: &str,
    method: &str,
) -> Result<String, String> {
    let source_path = PathBuf::from(source);
    if !source_path.exists() {
        return Err(format!("Source file does not exist: {}", source));
    }

    let target_dir = PathBuf::from(target_skills_dir);
    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create directory {}: {}", target_skills_dir, e))?;
    }

    let ext = source_path
        .extension()
        .and_then(|item| item.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "zip" {
        return extract_skill_archive(&source_path, &target_dir);
    }

    if ext == "skill" {
        match extract_skill_archive(&source_path, &target_dir) {
            Ok(installed) => return Ok(installed),
            Err(_) => {
                // Fall through and treat `.skill` as a markdown-like file.
            }
        }
    }

    let target_path = target_dir.join(normalize_skill_target_name(&source_path));

    // Remove existing file/symlink at target
    remove_path_if_exists(&target_path)?;

    if method == "symlink" {
        // Canonicalize source to absolute path for symlink
        let abs_source = std::fs::canonicalize(&source_path)
            .map_err(|e| format!("Failed to resolve source path: {}", e))?;
        match create_symlink(&abs_source, &target_path) {
            Ok(()) => return Ok(target_path.to_string_lossy().to_string()),
            Err(_e) => {
                // Fallback to copy if symlink fails (e.g. Windows without admin/dev mode)
                std::fs::copy(&source_path, &target_path)
                    .map_err(|e| format!("Failed to copy file: {}", e))?;
            }
        }
    } else {
        std::fs::copy(&source_path, &target_path)
            .map_err(|e| format!("Failed to copy file: {}", e))?;
    }

    Ok(target_path.to_string_lossy().to_string())
}

/// Uninstall (delete) a skill file
pub fn uninstall_skill_file(path: &str) -> Result<(), String> {
    let file_path = PathBuf::from(path);
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    create_skill_backup(&file_path)?;
    std::fs::remove_file(&file_path).map_err(|e| format!("Failed to delete {}: {}", path, e))
}

/// Copy a skill file to another tool's skills directory
pub fn copy_skill_between_tools(
    source_path: &str,
    target_skills_dir: &str,
    method: &str,
) -> Result<String, String> {
    install_skill_file(source_path, target_skills_dir, method)
}
