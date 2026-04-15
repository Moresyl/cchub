use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};

use super::env_path;

pub fn read_env_map(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let path = env_path(conn)?;
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut env_map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        env_map.insert(key.to_string(), value.trim().to_string());
    }
    Ok(env_map)
}

pub fn write_env_map(conn: &Connection, env_map: &HashMap<String, String>) -> Result<(), String> {
    let path = env_path(conn)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let ordered = env_map
        .iter()
        .filter_map(|(key, value)| {
            let trimmed_key = key.trim();
            if trimmed_key.is_empty() {
                None
            } else {
                Some((trimmed_key.to_string(), value.trim().to_string()))
            }
        })
        .collect::<BTreeMap<_, _>>();

    let mut content = ordered
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n");
    if !content.is_empty() {
        content.push('\n');
    }

    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())
}
