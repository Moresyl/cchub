use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::DbState;

const INTEGRATION_SETTING_KEY: &str = "claude_extension_integration";
const CONFIG_FILE_NAME: &str = "config.json";
const MAX_CONFIG_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeExtensionStatus {
    pub path: String,
    pub exists: bool,
    pub enabled: bool,
    pub valid_json: bool,
}

fn config_path(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    Ok(
        crate::commands::extra_commands::resolve_tool_config_dir(conn, "claude")?
            .join(CONFIG_FILE_NAME),
    )
}

fn read_config_object(
    path: &Path,
) -> Result<(serde_json::Map<String, serde_json::Value>, bool), String> {
    if !path.exists() {
        return Ok((serde_json::Map::new(), false));
    }

    let bytes =
        std::fs::read(path).map_err(|error| format!("读取 Claude 扩展配置失败: {error}"))?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err("Claude 扩展配置超过 2 MB 限制".to_string());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Claude 扩展配置不是有效 JSON: {error}"))?;
    let object = value
        .as_object()
        .cloned()
        .ok_or_else(|| "Claude 扩展配置必须是 JSON 对象".to_string())?;
    Ok((object, true))
}

fn managed(object: &serde_json::Map<String, serde_json::Value>) -> bool {
    object
        .get("primaryApiKey")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value == "any")
}

fn updated_object(
    mut object: serde_json::Map<String, serde_json::Value>,
    official: bool,
) -> serde_json::Map<String, serde_json::Value> {
    if official {
        object.remove("primaryApiKey");
    } else {
        object.insert(
            "primaryApiKey".to_string(),
            serde_json::Value::String("any".to_string()),
        );
    }
    object
}

fn write_config(
    conn: &rusqlite::Connection,
    official: bool,
) -> Result<ClaudeExtensionStatus, String> {
    let path = config_path(conn)?;
    let (object, existed) = read_config_object(&path)?;
    let next = updated_object(object, official);
    let value = serde_json::Value::Object(next);
    let content = format!(
        "{}\n",
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建 Claude 扩展配置目录失败: {error}"))?;
    }
    crate::utils::atomic_write_string(&path, &content).map_err(|error| error.to_string())?;

    Ok(ClaudeExtensionStatus {
        path: path.display().to_string(),
        exists: existed || path.exists(),
        enabled: !official,
        valid_json: true,
    })
}

fn integration_enabled(conn: &rusqlite::Connection) -> Result<bool, String> {
    Ok(
        crate::commands::extra_commands::get_text_app_setting(conn, INTEGRATION_SETTING_KEY)?
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("true")),
    )
}

pub fn sync_for_profile(conn: &rusqlite::Connection, snapshot: &str) -> Result<(), String> {
    if !integration_enabled(conn)? {
        return Ok(());
    }
    let parsed: serde_json::Value = serde_json::from_str(snapshot)
        .map_err(|error| format!("读取 Claude profile 元数据失败: {error}"))?;
    let category = parsed
        .get("metadata")
        .and_then(|metadata| metadata.get("category"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let official = category == "official";
    write_config(conn, official).map(|_| ())
}

#[tauri::command]
pub fn get_claude_extension_integration(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    integration_enabled(&conn)
}

#[tauri::command]
pub fn set_claude_extension_integration(
    enabled: bool,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    crate::commands::extra_commands::set_text_app_setting(
        &conn,
        INTEGRATION_SETTING_KEY,
        if enabled { "true" } else { "false" },
    )?;
    Ok(enabled)
}

#[tauri::command]
pub fn get_claude_extension_status(
    db: State<'_, DbState>,
) -> Result<ClaudeExtensionStatus, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let path = config_path(&conn)?;
    if !path.exists() {
        return Ok(ClaudeExtensionStatus {
            path: path.display().to_string(),
            exists: false,
            enabled: false,
            valid_json: true,
        });
    }
    match read_config_object(&path) {
        Ok((object, _)) => Ok(ClaudeExtensionStatus {
            path: path.display().to_string(),
            exists: true,
            enabled: managed(&object),
            valid_json: true,
        }),
        Err(_) => Ok(ClaudeExtensionStatus {
            path: path.display().to_string(),
            exists: true,
            enabled: false,
            valid_json: false,
        }),
    }
}

#[tauri::command]
pub fn apply_claude_extension_config(
    official: bool,
    db: State<'_, DbState>,
) -> Result<ClaudeExtensionStatus, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    write_config(&conn, official)
}

#[cfg(test)]
mod tests {
    use super::{managed, updated_object};

    #[test]
    fn applies_managed_key_without_touching_other_fields() {
        let mut object = serde_json::Map::new();
        object.insert("theme".to_string(), serde_json::json!("dark"));
        let next = updated_object(object, false);
        assert_eq!(next.get("theme"), Some(&serde_json::json!("dark")));
        assert!(managed(&next));
    }

    #[test]
    fn official_mode_removes_only_managed_key() {
        let mut object = serde_json::Map::new();
        object.insert("primaryApiKey".to_string(), serde_json::json!("any"));
        object.insert("theme".to_string(), serde_json::json!("dark"));
        let next = updated_object(object, true);
        assert!(!managed(&next));
        assert_eq!(next.get("theme"), Some(&serde_json::json!("dark")));
    }
}
