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

fn status_for_tool(conn: &rusqlite::Connection, tool_id: &str) -> LocalAuthStatus {
    let config_dir = resolve_tool_config_dir(conn, tool_id).ok();
    let candidates: Vec<std::path::PathBuf> = match (tool_id, config_dir.as_ref()) {
        ("claude", Some(dir)) => vec![dir.join(".credentials.json"), dir.join("credentials.json")],
        ("codex", Some(dir)) => vec![dir.join("auth.json")],
        ("gemini", Some(dir)) => vec![dir.join("oauth_creds.json"), dir.join("settings.json")],
        ("openclaw", Some(dir)) => vec![dir.join("auth-profiles.json"), dir.join("openclaw.json")],
        ("opencode", Some(dir)) => vec![dir.join("auth.json"), dir.join("opencode.json")],
        ("hermes", Some(dir)) => vec![dir.join("config.yaml"), dir.join("config.yml")],
        ("pi", Some(dir)) => vec![dir.join("models.json"), dir.join("settings.json")],
        ("grokbuild", Some(dir)) => vec![dir.join("auth.json"), dir.join("config.toml")],
        _ => Vec::new(),
    };
    let credential_path = candidates.iter().find(|path| non_empty_file(path)).cloned();
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
    use super::env_present;

    #[test]
    fn env_present_returns_false_for_empty_input() {
        assert!(!env_present(&[]));
    }
}
