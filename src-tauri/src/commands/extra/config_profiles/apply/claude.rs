use crate::{config_write, config_write::FileUpdate, json_config};
use serde_json::{Map, Value};
use std::{collections::HashSet, path::Path};

const SOURCE_KEYS: [&str; 2] = ["__claude_json_keys__", "__settings_json_keys__"];
const PROTECTED_SETTINGS: [&str; 3] = ["statusLine", "enabledPlugins", "mcpServers"];
const SETTINGS_KEYS: [&str; 9] = [
    "permissions",
    "skipDangerousModePermissionPrompt",
    "alwaysThinkingEnabled",
    "attribution",
    "autoUpdatesChannel",
    "statusLine",
    "enabledPlugins",
    "mcpServers",
    "env",
];

struct Document {
    original: Option<Vec<u8>>,
    value: Map<String, Value>,
}

fn text(bytes: Option<&[u8]>) -> Result<&str, String> {
    bytes
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| "Claude configuration must be UTF-8; repair it before switching".to_string())
        .map(|value| value.unwrap_or("{}\n"))
}

fn load(path: &Path) -> Result<Document, String> {
    let original = config_write::read(path)?;
    let value = parse(text(original.as_deref())?)?;
    Ok(Document {
        original,
        value: value.as_object().unwrap().clone(),
    })
}

fn parse(source: &str) -> Result<Value, String> {
    // Match the existing Claude settings adapter's strict native JSON contract.
    serde_json::from_str::<Value>(source)
        .map_err(|_| "Claude configuration must contain valid JSON; repair it before switching")?;
    json_config::parse_json_object(source)
}

pub(super) fn read_at(paths: [&Path; 2]) -> Result<String, String> {
    let _guard = json_config::write_lock()?;
    check_locations(paths)?;
    let first = load(paths[0])?;
    let second = load(paths[1])?;
    if first.value.is_empty() && second.value.is_empty() {
        return Err("No Claude config found".into());
    }
    let sources = [
        first.value.keys().cloned().collect::<Vec<_>>(),
        second.value.keys().cloned().collect::<Vec<_>>(),
    ];
    let mut combined = first.value;
    for (key, value) in second.value {
        combined.entry(key).or_insert(value);
    }
    for (key, source) in SOURCE_KEYS.into_iter().zip(sources) {
        combined.insert(key.into(), serde_json::json!(source));
    }
    serde_json::to_string_pretty(&combined).map_err(|_| "Cannot encode Claude snapshot".into())
}

fn source_keys(snapshot: &Map<String, Value>, key: &str) -> Result<HashSet<String>, String> {
    let Some(value) = snapshot.get(key) else {
        return Ok(HashSet::new());
    };
    let keys = value
        .as_array()
        .ok_or("Claude snapshot source metadata must contain arrays of field names")?;
    let mut result = HashSet::new();
    for key in keys {
        let key = key
            .as_str()
            .ok_or("Claude snapshot source metadata must contain arrays of field names")?;
        if SOURCE_KEYS.contains(&key) || !result.insert(key.to_owned()) {
            return Err(
                "Claude snapshot source metadata contains invalid or duplicate fields".into(),
            );
        }
    }
    Ok(result)
}

pub(super) struct Prepared {
    updates: Vec<FileUpdate>,
    absent: Vec<std::path::PathBuf>,
}

impl Prepared {
    pub(super) fn commit(self) -> Result<(), String> {
        check_absent(&self.absent)?;
        config_write::commit_then(self.updates, || check_absent(&self.absent))
    }
}

fn check_absent(paths: &[std::path::PathBuf]) -> Result<(), String> {
    for path in paths {
        if config_write::read(path)?.is_some() {
            return Err("Configuration changed externally; reload it and try again".into());
        }
    }
    Ok(())
}

fn check_locations(paths: [&Path; 2]) -> Result<(), String> {
    if paths[0] == paths[1] {
        return Err("Claude configuration files must have distinct locations".into());
    }
    Ok(())
}

// Caller owns the shared application write lock through prepare and commit.
pub(super) fn prepare_at(paths: [&Path; 2], snapshot: &str) -> Result<Prepared, String> {
    check_locations(paths)?;
    let snapshot = parse(snapshot)?;
    let snapshot = snapshot.as_object().unwrap();
    let sources = [
        source_keys(snapshot, SOURCE_KEYS[0])?,
        source_keys(snapshot, SOURCE_KEYS[1])?,
    ];
    let tagged = sources.iter().any(|source| !source.is_empty());
    let mut fields = [Map::new(), Map::new()];
    for (key, value) in snapshot {
        if SOURCE_KEYS.contains(&key.as_str()) {
            continue;
        }
        if tagged {
            for index in 0..2 {
                if sources[index].contains(key) {
                    fields[index].insert(key.clone(), value.clone());
                }
            }
            if !sources.iter().any(|source| source.contains(key)) {
                fields[1].insert(key.clone(), value.clone());
            }
        } else {
            let index = usize::from(SETTINGS_KEYS.contains(&key.as_str()));
            fields[index].insert(key.clone(), value.clone());
        }
    }
    // Validate both documents before preparing any write. An unreadable or
    // malformed file never becomes a fabricated empty object.
    let documents = [load(paths[0])?, load(paths[1])?];
    let mut updates = Vec::new();
    let mut absent = Vec::new();
    for (index, (document, fields)) in documents.into_iter().zip(fields).enumerate() {
        if index == 0 && fields.is_empty() && document.original.is_none() {
            absent.push(paths[index].to_path_buf());
            continue;
        }
        let desired = json_config::edit_json_text(text(document.original.as_deref())?, |value| {
            let target = value.as_object_mut().unwrap();
            for (key, value) in fields {
                if index == 1 && PROTECTED_SETTINGS.contains(&key.as_str()) {
                    continue;
                }
                target.insert(key, value);
            }
            Ok(())
        })?;
        updates.push(FileUpdate {
            path: paths[index].into(),
            original: document.original,
            desired: desired.into_bytes(),
        });
    }
    Ok(Prepared { updates, absent })
}

#[cfg(test)]
mod tests;
