use rusqlite::Connection;
use serde_yaml::{Mapping, Value};
use std::path::{Path, PathBuf};

use super::config_path;

fn ensure_mapping(value: &mut Value) -> &mut Mapping {
    if !matches!(value, Value::Mapping(_)) {
        *value = Value::Mapping(Mapping::new());
    }
    match value {
        Value::Mapping(mapping) => mapping,
        _ => unreachable!(),
    }
}

fn backup_path_for(path: &Path) -> PathBuf {
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    path.with_file_name(format!("config.yaml.cchub-backup.{timestamp}"))
}

fn has_existing_backup(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .map(|name| name.starts_with("config.yaml.cchub-backup."))
            .unwrap_or(false)
    })
}

pub fn read_value_if_exists(conn: &Connection) -> Result<Option<Value>, String> {
    let path = config_path(conn)?;
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_yaml::from_str::<Value>(&content)
        .map(Some)
        .map_err(|e| format!("Failed to parse Hermes config.yaml: {e}"))
}

pub fn read_value(conn: &Connection) -> Result<Value, String> {
    let mut value = read_value_if_exists(conn)?.unwrap_or(Value::Mapping(Mapping::new()));
    ensure_mapping(&mut value);
    Ok(value)
}

pub fn write_value(conn: &Connection, value: &Value) -> Result<Option<PathBuf>, String> {
    let path = config_path(conn)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut created_backup = None;
    if path.exists() && !has_existing_backup(&path) {
        let backup_path = backup_path_for(&path);
        std::fs::copy(&path, &backup_path).map_err(|e| {
            format!(
                "Failed to back up Hermes config {} to {}: {e}",
                path.display(),
                backup_path.display()
            )
        })?;
        created_backup = Some(backup_path);
    }

    let mut content = serde_yaml::to_string(value)
        .map_err(|e| format!("Failed to serialize Hermes YAML: {e}"))?;
    if !content.ends_with('\n') {
        content.push('\n');
    }
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(created_backup)
}

pub fn top_level_mapping_mut(value: &mut Value) -> &mut Mapping {
    ensure_mapping(value)
}
