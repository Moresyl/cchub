use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};

use super::env_path;

pub fn read_env_map(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let path = env_path(conn)?;
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(parse_env_text(&content))
}

pub(super) fn parse_env_text(content: &str) -> HashMap<String, String> {
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
    env_map
}

pub(crate) fn render_env_map(env_map: &HashMap<String, String>) -> Result<String, String> {
    for (key, value) in env_map {
        let mut bytes = key.bytes();
        if !bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || value
                .chars()
                .any(|character| matches!(character, '\r' | '\n' | '\0'))
        {
            return Err("Invalid environment variable name or multiline value".into());
        }
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

    Ok(content)
}
