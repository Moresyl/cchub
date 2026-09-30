use serde::Serialize;
use tauri::State;

use crate::db::DbState;

use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAuthStatus {
    pub tool_id: String,
    pub authenticated: bool,
    pub source: String,
    pub credential_path: Option<String>,
    pub detail: String,
}

fn non_empty_file(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|item| item.is_file() && item.len() > 2)
        .unwrap_or(false)
}

fn env_present(keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        std::env::var(key)
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn credential_value(value: &serde_json::Value) -> bool {
    let Some(value) = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    if let Some(variable) = value
        .strip_prefix("{env:")
        .and_then(|value| value.strip_suffix('}'))
    {
        return env_present(&[variable]);
    }
    true
}

fn opencode_file_has_credentials(path: &std::path::Path) -> bool {
    let Ok(source) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = crate::json_config::parse_json_object(&source) else {
        return false;
    };
    if value
        .pointer("/options/apiKey")
        .is_some_and(credential_value)
    {
        return true;
    }
    if value
        .get("provider")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|providers| {
            providers.values().any(|provider| {
                provider
                    .pointer("/options/apiKey")
                    .is_some_and(credential_value)
            })
        })
    {
        return true;
    }
    value.as_object().is_some_and(|providers| {
        providers.values().any(|provider| {
            matches!(
                provider.get("type").and_then(serde_json::Value::as_str),
                Some("api" | "oauth")
            ) && ["key", "access", "refresh"]
                .iter()
                .any(|key| provider.get(*key).is_some_and(credential_value))
        })
    })
}

fn status_for_tool(conn: &rusqlite::Connection, tool_id: &str) -> LocalAuthStatus {
    let config_dir = resolve_tool_config_dir(conn, tool_id).ok();
    let candidates: Vec<std::path::PathBuf> = match (tool_id, config_dir.as_ref()) {
        ("claude", Some(dir)) => vec![dir.join(".credentials.json"), dir.join("credentials.json")],
        ("codex", Some(dir)) => vec![dir.join("auth.json")],
        ("gemini", Some(dir)) => vec![dir.join("oauth_creds.json"), dir.join("settings.json")],
        ("openclaw", Some(dir)) => vec![dir.join("auth-profiles.json"), dir.join("openclaw.json")],
        ("opencode", Some(dir)) => {
            let mut paths = dirs::home_dir()
                .map(|home| vec![crate::opencode_paths::data_dir(&home).join("auth.json")])
                .unwrap_or_default();
            paths.push(dir.join("auth.json"));
            if let Ok(path) = resolve_tool_config_path(conn, tool_id) {
                paths.push(path);
            }
            paths
        }
        ("hermes", Some(dir)) => vec![dir.join("config.yaml"), dir.join("config.yml")],
        ("pi", Some(dir)) => vec![dir.join("models.json"), dir.join("settings.json")],
        ("grokbuild", Some(dir)) => vec![dir.join("auth.json"), dir.join("config.toml")],
        _ => Vec::new(),
    };
    let credential_path = candidates
        .iter()
        .find(|path| {
            if tool_id == "opencode" {
                opencode_file_has_credentials(path)
            } else {
                non_empty_file(path)
            }
        })
        .cloned();
    let environment = match tool_id {
        "claude" => env_present(&["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"]),
        "codex" => env_present(&["OPENAI_API_KEY"]),
        "gemini" => env_present(&["GEMINI_API_KEY", "GOOGLE_API_KEY"]),
        "hermes" => env_present(&["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GEMINI_API_KEY"]),
        _ => false,
    };
    let authenticated = credential_path.is_some() || environment;
    let source = if credential_path.is_some() && environment {
        "file+environment"
    } else if credential_path.is_some() {
        "file"
    } else if environment {
        "environment"
    } else {
        "none"
    };
    LocalAuthStatus {
        tool_id: tool_id.to_string(),
        authenticated,
        source: source.to_string(),
        credential_path: credential_path.map(|path| path.to_string_lossy().to_string()),
        detail: if authenticated {
            "Credential detected"
        } else {
            "No local credential detected"
        }
        .to_string(),
    }
}

#[tauri::command]
pub fn get_local_auth_status(db: State<'_, DbState>) -> Result<Vec<LocalAuthStatus>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok([
        "claude",
        "codex",
        "gemini",
        "grokbuild",
        "opencode",
        "openclaw",
        "hermes",
        "pi",
    ]
    .into_iter()
    .map(|tool_id| status_for_tool(&conn, tool_id))
    .collect())
}

#[cfg(test)]
mod tests {
    use super::{env_present, opencode_file_has_credentials};

    #[test]
    fn env_present_returns_false_for_empty_input() {
        assert!(!env_present(&[]));
    }

    #[test]
    fn opencode_auth_detection_requires_an_actual_credential() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("auth.json");
        for source in [
            "{}\n",
            "{\"theme\":\"dark\"}",
            "{\"provider\":{\"custom\":{\"options\":{\"apiKey\":\" \"}}}}",
            "{\"openai\":{\"type\":\"oauth\",\"expires\":123}}",
            "broken",
        ] {
            std::fs::write(&path, source).unwrap();
            assert!(!opencode_file_has_credentials(&path));
        }
        for source in [
            "{\"openai\":{\"type\":\"api\",\"key\":\"fixture\"}}",
            "{\"openai\":{\"type\":\"oauth\",\"refresh\":\"fixture\"}}",
            "{ // config\n\"provider\":{\"custom\":{\"options\":{\"apiKey\":\"fixture\"}}}}",
        ] {
            std::fs::write(&path, source).unwrap();
            assert!(opencode_file_has_credentials(&path));
        }
    }
}
