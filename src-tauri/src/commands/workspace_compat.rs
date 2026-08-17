use std::path::{Path, PathBuf};

fn workspace_root() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".openclaw").join("workspace"))
        .ok_or_else(|| "Cannot find home directory".to_string())
}

fn validate_filename(filename: &str) -> Result<&Path, String> {
    let path = Path::new(filename);
    if filename.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return Err("Workspace filename must be a relative safe path".to_string());
    }
    Ok(path)
}

#[tauri::command]
pub fn read_workspace_file(filename: String) -> Result<Option<String>, String> {
    let relative = validate_filename(&filename)?;
    let path = workspace_root()?.join(relative);
    if !path.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(&path)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn write_workspace_file(filename: String, content: String) -> Result<(), String> {
    let relative = validate_filename(&filename)?;
    let root = workspace_root()?;
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &content).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_workspace_directory(subdir: Option<String>) -> Result<bool, String> {
    let root = workspace_root()?;
    let path = match subdir.as_deref() {
        Some("memory") => root.join("memory"),
        Some(value) if !value.trim().is_empty() => {
            let relative = validate_filename(value)?;
            root.join(relative)
        }
        _ => root,
    };
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    crate::commands::extra_commands::open_in_system(path.to_string_lossy().into_owned())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::validate_filename;

    #[test]
    fn rejects_workspace_traversal() {
        assert!(validate_filename("../outside.md").is_err());
        assert!(validate_filename("notes/today.md").is_ok());
    }
}
