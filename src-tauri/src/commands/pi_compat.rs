//! Pi integration commands.
//!
//! Pi keeps its provider catalog and session settings in files under the
//! agent directory, while CCHub stores managed provider snapshots in SQLite.
//! These commands bridge both stores without copying transcript contents.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

use crate::db::DbState;

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_SCRIPT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiCurrentState {
    pub enabled_provider_ids: Vec<String>,
    pub default_provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PiSessionDiscovery {
    Available,
    RequiresProjectContext {
        #[serde(rename = "configuredPath")]
        configured_path: String,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageScript {
    pub enabled: bool,
    pub language: String,
    pub code: String,
    pub timeout: Option<u64>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub access_token: Option<String>,
    pub user_id: Option<String>,
    pub template_type: Option<String>,
    pub auto_query_interval: Option<u64>,
    pub coding_plan_provider: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub team_organization_id: Option<String>,
    pub team_project_id: Option<String>,
}

fn pi_agent_dir() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("PI_CODING_AGENT_DIR") {
        if !value.is_empty() {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err("PI_CODING_AGENT_DIR must be an absolute directory".to_string());
            }
            return Ok(path);
        }
    }
    dirs::home_dir()
        .map(|home| home.join(".pi").join("agent"))
        .ok_or_else(|| "Cannot determine the home directory".to_string())
}

fn read_limited(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(format!(
            "Pi file exceeds the 1 MiB limit: {}",
            path.display()
        ));
    }
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(format!(
            "Pi file exceeds the 1 MiB limit: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

fn read_json_file(path: &Path) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = read_limited(path)?;
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Invalid Pi JSON ({}): {error}", path.display()))?;
    Ok(Some(value))
}

fn read_json_object(path: &Path) -> Result<Option<Map<String, Value>>, String> {
    Ok(read_json_file(path)?
        .map(|value| {
            value
                .as_object()
                .cloned()
                .ok_or_else(|| format!("Pi JSON root must be an object: {}", path.display()))
        })
        .transpose()?)
}

fn expand_home(value: &str, home: &Path) -> Option<PathBuf> {
    if value == "~" {
        return Some(home.to_path_buf());
    }
    value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
        .map(|suffix| home.join(suffix))
        .or_else(|| {
            let path = PathBuf::from(value);
            path.is_absolute().then_some(path)
        })
}

fn session_root() -> Result<Result<PathBuf, String>, String> {
    let home = dirs::home_dir().ok_or_else(|| "Cannot determine the home directory".to_string())?;
    let configured = match std::env::var("PI_CODING_AGENT_SESSION_DIR")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        Some(value) => Some(value),
        None => read_json_object(&pi_agent_dir()?.join("settings.json"))?.and_then(|object| {
            object
                .get("sessionDir")
                .and_then(Value::as_str)
                .map(str::to_string)
                .filter(|v| !v.trim().is_empty())
        }),
    };

    if let Some(value) = configured {
        let Some(path) = expand_home(&value, &home) else {
            return Ok(Err(format!(
                "Pi sessionDir '{value}' requires a project cwd and cannot be globally enumerated"
            )));
        };
        match fs::metadata(&path) {
            Ok(metadata) if !metadata.is_dir() => Ok(Err(format!(
                "Configured Pi session directory is not a directory: {}",
                path.display()
            ))),
            Ok(_) => fs::read_dir(&path)
                .map(|_| Ok(path.clone()))
                .map_err(|error| {
                    format!(
                        "Configured Pi session directory is not readable ({}): {error}",
                        path.display()
                    )
                }),
            Err(error) => Ok(Err(format!(
                "Configured Pi session directory is unavailable ({}): {error}",
                path.display()
            ))),
        }
    } else {
        Ok(Ok(pi_agent_dir()?.join("sessions")))
    }
}

fn validate_usage_script(script: &UsageScript) -> Result<(), String> {
    if script.code.len() > MAX_SCRIPT_BYTES {
        return Err("Usage script exceeds the 256 KiB limit".to_string());
    }
    if script.language.trim().is_empty() {
        return Err("Usage script language is required".to_string());
    }
    if script.auto_query_interval.is_some_and(|value| value > 1440) {
        return Err("Auto query interval cannot exceed 1440 minutes".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn get_pi_current_state() -> Result<PiCurrentState, String> {
    let agent_dir = pi_agent_dir()?;
    let providers = read_json_object(&agent_dir.join("models.json"))?
        .and_then(|object| object.get("providers").and_then(Value::as_object).cloned())
        .unwrap_or_default();
    let default_provider_id =
        read_json_object(&agent_dir.join("settings.json"))?.and_then(|object| {
            object
                .get("defaultProvider")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        });
    Ok(PiCurrentState {
        enabled_provider_ids: providers.keys().cloned().collect(),
        default_provider_id,
    })
}

#[tauri::command]
pub fn get_pi_session_discovery() -> PiSessionDiscovery {
    match session_root() {
        Ok(Ok(_)) => PiSessionDiscovery::Available,
        Ok(Err(reason)) if reason.contains("requires a project cwd") => {
            let configured_path = std::env::var("PI_CODING_AGENT_SESSION_DIR")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    pi_agent_dir().ok().and_then(|dir| {
                        read_json_object(&dir.join("settings.json"))
                            .ok()
                            .flatten()
                            .and_then(|object| {
                                object
                                    .get("sessionDir")
                                    .and_then(Value::as_str)
                                    .map(ToString::to_string)
                            })
                    })
                })
                .unwrap_or_default();
            PiSessionDiscovery::RequiresProjectContext { configured_path }
        }
        Ok(Err(reason)) | Err(reason) => PiSessionDiscovery::Unavailable { reason },
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn update_pi_provider_usage_script(
    id: String,
    usage_script: UsageScript,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Pi provider id is required".to_string());
    }
    validate_usage_script(&usage_script)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let (tool_id, snapshot): (String, String) = conn
        .query_row(
            "SELECT tool_id, config_snapshot FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| format!("Pi provider '{id}' not found"))?;
    if tool_id != "pi" {
        return Err(format!("Provider does not belong to Pi: {id}"));
    }
    let mut document: Value = serde_json::from_str(&snapshot).map_err(|error| error.to_string())?;
    let object = document
        .as_object_mut()
        .ok_or_else(|| "Pi provider snapshot must be a JSON object".to_string())?;
    object.insert(
        "usageScript".to_string(),
        serde_json::to_value(usage_script).map_err(|error| error.to_string())?,
    );
    let next = serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE config_profiles SET config_snapshot = ?1, updated_at = ?2 WHERE id = ?3 AND tool_id = 'pi'",
        rusqlite::params![next, chrono::Utc::now().to_rfc3339(), id],
    )
    .map_err(|error| error.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{expand_home, validate_usage_script, UsageScript};
    use std::path::Path;

    fn script() -> UsageScript {
        UsageScript {
            enabled: true,
            language: "javascript".to_string(),
            code: "return {};".to_string(),
            timeout: None,
            api_key: None,
            base_url: None,
            access_token: None,
            user_id: None,
            template_type: None,
            auto_query_interval: None,
            coding_plan_provider: None,
            access_key_id: None,
            secret_access_key: None,
            team_organization_id: None,
            team_project_id: None,
        }
    }

    #[test]
    fn validates_usage_script_bounds() {
        let mut value = script();
        assert!(validate_usage_script(&value).is_ok());
        value.auto_query_interval = Some(1441);
        assert!(validate_usage_script(&value).is_err());
    }

    #[test]
    fn expands_only_absolute_or_home_paths() {
        let home = Path::new("C:\\Users\\tester");
        assert_eq!(expand_home("~/sessions", home), Some(home.join("sessions")));
        assert!(expand_home("relative/sessions", home).is_none());
    }
}
