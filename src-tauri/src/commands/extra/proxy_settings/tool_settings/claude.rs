use super::storage;
use crate::{
    config_write::{self, FileUpdate},
    json_config,
};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct ClaudeSettings {
    pub permission_mode: String,
    pub allow_count: usize,
    pub ask_count: usize,
    pub deny_count: usize,
    pub auto_update: String,
    pub model: String,
    pub tool_search: String,
    pub legacy_tool_search: String,
    pub config_revision: String,
}

struct Snapshot {
    paths: [PathBuf; 2],
    bytes: [Option<Vec<u8>>; 2],
    values: [Value; 2],
}

fn parse(bytes: Option<&[u8]>) -> Result<Value, String> {
    let source = storage::text(bytes, "{}\n")?;
    // Claude's native settings are strict JSON. The field editor also rejects
    // duplicate properties instead of selecting one silently.
    serde_json::from_str::<Value>(source)
        .map_err(|_| "Claude settings must contain valid JSON; repair them before saving")?;
    json_config::parse_json_object(source)
}

fn object<'a>(value: &'a Value, key: &str) -> Result<Option<&'a Map<String, Value>>, String> {
    match value.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_object()
            .map(Some)
            .ok_or_else(|| "Claude settings contain an invalid object field".into()),
    }
}

fn string(value: Option<&Value>) -> Result<String, String> {
    match value {
        None => Ok(String::new()),
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "Claude settings contain an invalid string field".into()),
    }
}

fn env_string(value: &Value, key: &str) -> Result<String, String> {
    string(object(value, "env")?.and_then(|env| env.get(key)))
}

fn count(permissions: Option<&Map<String, Value>>, key: &str) -> Result<usize, String> {
    match permissions.and_then(|permissions| permissions.get(key)) {
        None => Ok(0),
        Some(Value::Array(values)) if values.iter().all(Value::is_string) => Ok(values.len()),
        Some(_) => Err("Claude permission rules must be arrays of strings".into()),
    }
}

impl Snapshot {
    fn load(path: &Path) -> Result<Self, String> {
        let local = path
            .parent()
            .ok_or("Invalid Claude settings location")?
            .join("settings.local.json");
        let bytes = [storage::read(path)?, storage::read(&local)?];
        let values = [parse(bytes[0].as_deref())?, parse(bytes[1].as_deref())?];
        Ok(Self {
            paths: [path.to_path_buf(), local],
            bytes,
            values,
        })
    }

    fn settings(&self) -> Result<ClaudeSettings, String> {
        let permissions = object(&self.values[0], "permissions")?;
        let update_disabled = env_string(&self.values[0], "DISABLE_AUTOUPDATER")?;
        let channel = string(self.values[0].get("autoUpdatesChannel"))?;
        let mut digest = Sha256::new();
        digest.update(b"cchub-claude-settings-v1");
        for index in 0..2 {
            digest.update(storage::revision(
                &self.paths[index],
                self.bytes[index].as_deref(),
            ));
        }
        Ok(ClaudeSettings {
            permission_mode: string(
                permissions.and_then(|permissions| permissions.get("defaultMode")),
            )?,
            allow_count: count(permissions, "allow")?,
            ask_count: count(permissions, "ask")?,
            deny_count: count(permissions, "deny")?,
            auto_update: if update_disabled == "1" || update_disabled.eq_ignore_ascii_case("true") {
                "disabled".into()
            } else {
                channel
            },
            model: string(self.values[0].get("model"))?,
            tool_search: env_string(&self.values[0], "ENABLE_TOOL_SEARCH")?,
            legacy_tool_search: env_string(&self.values[1], "ENABLE_TOOL_SEARCH")?,
            config_revision: format!("{:x}", digest.finalize()),
        })
    }
}

pub(super) fn read(path: &Path) -> Result<ClaudeSettings, String> {
    let _guard = json_config::write_lock()?;
    Snapshot::load(path)?.settings()
}

fn edit_object<'a>(value: &'a mut Value, key: &str) -> Result<&'a mut Map<String, Value>, String> {
    let root = value
        .as_object_mut()
        .ok_or("Claude settings must be an object")?;
    root.entry(key)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "Claude settings contain an invalid object field".into())
}

fn set_optional(value: &mut Value, key: &str, next: &str) {
    if next.is_empty() {
        value.as_object_mut().unwrap().remove(key);
    } else {
        value[key] = json!(next);
    }
}

fn set_env(value: &mut Value, key: &str, next: &str) -> Result<(), String> {
    if next.is_empty() && value.get("env").is_none() {
        return Ok(());
    }
    let env = edit_object(value, "env")?;
    if next.is_empty() {
        env.remove(key);
    } else {
        env.insert(key.into(), json!(next));
    }
    // Keep an originally present env object, even if it becomes empty.
    Ok(())
}

fn edit(value: &mut Value, key: &str, next: &str) -> Result<(), String> {
    match key {
        "permission_mode" => {
            if !matches!(
                next,
                "" | "default"
                    | "manual"
                    | "acceptEdits"
                    | "plan"
                    | "auto"
                    | "dontAsk"
                    | "bypassPermissions"
            ) {
                return Err("Unsupported Claude permission mode".into());
            }
            if next.is_empty() && value.get("permissions").is_none() {
                return Ok(());
            }
            let permissions = edit_object(value, "permissions")?;
            if next.is_empty() {
                permissions.remove("defaultMode");
            } else {
                permissions.insert("defaultMode".into(), json!(next));
            }
        }
        "auto_update" => {
            if !matches!(next, "" | "latest" | "stable" | "disabled") {
                return Err("Unsupported Claude update channel".into());
            }
            if next == "disabled" {
                set_env(value, "DISABLE_AUTOUPDATER", "1")?;
            } else {
                set_env(value, "DISABLE_AUTOUPDATER", "")?;
                set_optional(value, "autoUpdatesChannel", next);
            }
        }
        "model" => {
            if next.len() > 512 || next.chars().any(char::is_control) || next.trim() != next {
                return Err("Invalid Claude model identifier".into());
            }
            set_optional(value, "model", next);
        }
        "tool_search" => {
            let threshold = next.strip_prefix("auto:").is_some_and(|number| {
                !number.is_empty()
                    && number.bytes().all(|byte| byte.is_ascii_digit())
                    && number.parse::<u32>().is_ok_and(|number| number <= 100)
            });
            if !matches!(next, "" | "true" | "false" | "auto") && !threshold {
                return Err("Unsupported Claude tool search value".into());
            }
            set_env(value, "ENABLE_TOOL_SEARCH", next)?;
        }
        _ => return Err("Unsupported Claude setting".into()),
    }
    Ok(())
}

pub(super) fn write(
    path: &Path,
    key: &str,
    next: &str,
    expected_revision: &str,
) -> Result<ClaudeSettings, String> {
    let _guard = json_config::write_lock()?;
    let mut snapshot = Snapshot::load(path)?;
    if snapshot.settings()?.config_revision != expected_revision {
        return Err("Configuration or its location changed; reload before saving".into());
    }
    let mut updates = Vec::new();
    let mut unchanged = Vec::new();
    for index in 0..2 {
        let source = storage::text(snapshot.bytes[index].as_deref(), "{}\n")?;
        let desired = json_config::edit_json_text(source, |value| {
            if index == 0 {
                edit(value, key, next)
            } else if key == "tool_search" {
                set_env(value, "ENABLE_TOOL_SEARCH", "")
            } else {
                Ok(())
            }
        })?;
        if desired == source {
            unchanged.push((snapshot.paths[index].clone(), snapshot.bytes[index].clone()));
        } else {
            updates.push(FileUpdate {
                path: snapshot.paths[index].clone(),
                original: snapshot.bytes[index].clone(),
                desired: desired.as_bytes().to_vec(),
            });
            snapshot.bytes[index] = Some(desired.as_bytes().to_vec());
            snapshot.values[index] = parse(Some(desired.as_bytes()))?;
        }
    }
    let acknowledged = snapshot.settings()?;
    config_write::commit_then(updates, || {
        for (path, bytes) in unchanged {
            if storage::read(&path)? != bytes {
                return Err("Configuration changed while saving; reload before editing".into());
            }
        }
        Ok(())
    })?;
    Ok(acknowledged)
}
