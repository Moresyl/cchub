#![allow(clippy::too_many_arguments)]
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use crate::copilot_auth::{self, CopilotAuthState};
use crate::db::DbState;
use crate::hermes;
use crate::shared::http_client;
use crate::utils::configure_background_command;

use super::super::log_command_timing;
use super::super::proxy_settings::*;
use super::super::statusline::*;
use super::super::types::*;
use super::*;

pub fn bootstrap_tool_environment_from_conn(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<BootstrapToolEnvironmentResult, String> {
    let mut created_dirs = 0usize;
    let mut created_files = 0usize;
    let mut notes = Vec::new();

    let config_dir = resolve_tool_config_dir(conn, tool_id)?;
    ensure_dir_exists(&config_dir, &mut created_dirs)?;

    let skills_dir = resolve_tool_skills_dir(conn, tool_id)?;
    ensure_dir_exists(&skills_dir, &mut created_dirs)?;

    match tool_id {
        "claude" => {
            let (claude_json_path, settings_json_path) = resolve_claude_paths(conn)?;
            if let Some(parent) = claude_json_path.parent() {
                ensure_dir_exists(parent, &mut created_dirs)?;
            }
            if let Some(parent) = settings_json_path.parent() {
                ensure_dir_exists(parent, &mut created_dirs)?;
            }
            write_default_file_if_missing(&claude_json_path, "{}\n", &mut created_files)?;
            write_default_file_if_missing(&settings_json_path, "{}\n", &mut created_files)?;
        }
        "codex" => {
            write_default_file_if_missing(&config_dir.join("config.toml"), "", &mut created_files)?;
            write_default_file_if_missing(
                &config_dir.join("auth.json"),
                "{}\n",
                &mut created_files,
            )?;
            notes.push("Codex CLI 仍需登录后 auth.json 才会真正可用".to_string());
        }
        "gemini" => {
            write_default_file_if_missing(
                &config_dir.join("settings.json"),
                "{}\n",
                &mut created_files,
            )?;
            write_default_file_if_missing(
                &config_dir.join(".env"),
                "# Add GEMINI_API_KEY=...\n",
                &mut created_files,
            )?;
            notes.push("Gemini CLI 仍需在 .env 中填写 GEMINI_API_KEY".to_string());
        }
        "opencode" => {
            write_default_file_if_missing(
                &config_dir.join("opencode.json"),
                "{}\n",
                &mut created_files,
            )?;
        }
        "openclaw" => {
            write_default_file_if_missing(
                &config_dir.join("openclaw.json"),
                "{}\n",
                &mut created_files,
            )?;
        }
        "hermes" => {
            write_default_file_if_missing(
                &config_dir.join("config.yaml"),
                "model:\n  provider: openrouter\n  default: anthropic/claude-sonnet-4.6\n  base_url: https://openrouter.ai/api/v1\n",
                &mut created_files,
            )?;
            write_default_file_if_missing(
                &config_dir.join(".env"),
                "# Add OPENROUTER_API_KEY=...\n",
                &mut created_files,
            )?;
            notes.push("Hermes 仅支持 Linux / macOS / WSL2；Windows 请把根目录覆盖指向 WSL2 内的 ~/.hermes".to_string());
        }
        _ => return Err(format!("Unknown tool: {}", tool_id)),
    }

    Ok(BootstrapToolEnvironmentResult {
        created_dirs,
        created_files,
        notes,
    })
}

pub fn json_file_has_content(path: &std::path::Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };

    match value {
        serde_json::Value::Object(map) => !map.is_empty(),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.trim().is_empty(),
        _ => true,
    }
}

pub fn gemini_env_has_api_key(path: &std::path::Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };

    content.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            return false;
        };

        key.trim() == "GEMINI_API_KEY" && !value.trim().is_empty() && value.trim() != "..."
    })
}

fn is_external_target(target: &str) -> bool {
    let trimmed = target.trim();
    trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with("tel:")
}

fn existing_open_target(target: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(target);
    if path.exists() {
        return Ok(path);
    }

    let mut current = path.parent().map(|parent| parent.to_path_buf());
    while let Some(candidate) = current {
        if candidate.exists() {
            return Ok(candidate);
        }
        current = candidate.parent().map(|parent| parent.to_path_buf());
    }

    Err(format!("Path not found: {}", target))
}

pub fn open_target_in_system(target: &str) -> Result<(), String> {
    if target.trim().is_empty() {
        return Err("Target is empty".to_string());
    }

    if is_external_target(target) {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", target])
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(target)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open")
                .arg(target)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }

    let resolved_target = existing_open_target(target)?;
    let resolved_text = resolved_target.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        if resolved_target.is_file() {
            std::process::Command::new("explorer")
                .args(["/select,", &resolved_text])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            std::process::Command::new("explorer")
                .arg(&resolved_text)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if resolved_target.is_file() {
            std::process::Command::new("open")
                .args(["-R", &resolved_text])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            std::process::Command::new("open")
                .arg(&resolved_text)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&resolved_text)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform".to_string())
}

pub(crate) fn set_json_app_setting<T: Serialize>(
    conn: &rusqlite::Connection,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let payload = serde_json::to_string(value).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, payload],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn get_json_app_setting<T: for<'de> Deserialize<'de>>(
    conn: &rusqlite::Connection,
    key: &str,
) -> Result<Option<T>, String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .ok();

    match raw {
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

pub fn set_text_app_setting(
    conn: &rusqlite::Connection,
    key: &str,
    value: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_text_app_setting(
    conn: &rusqlite::Connection,
    key: &str,
) -> Result<Option<String>, String> {
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .ok())
}

pub const MANAGED_APP_IDS: [&str; 6] = [
    "claude", "codex", "gemini", "opencode", "openclaw", "hermes",
];
pub const VISIBLE_APPS_SETTING_KEY: &str = "visible_apps";
pub const WINDOW_PREFERENCES_SETTING_KEY: &str = "window_preferences";
pub const COMMON_CONFIG_SNIPPETS_SETTING_KEY: &str = "common_config_snippets";
pub const WELCOME_COMPLETED_SETTING_KEY: &str = "welcome_completed";

fn is_common_config_tool(tool_id: &str) -> bool {
    matches!(tool_id, "claude" | "codex" | "gemini")
}

pub fn normalize_integer_like(value: &str) -> Option<i64> {
    let normalized = value.trim().replace(['_', ',', ' '], "");
    if normalized.is_empty() {
        return None;
    }
    normalized.parse::<i64>().ok()
}

pub fn normalized_non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_common_config_snippet(mut snippet: CommonConfigSnippet) -> CommonConfigSnippet {
    let mut normalized = HashMap::new();
    for (key, value) in snippet.custom_values {
        let Some(key) = normalized_non_empty(&key) else {
            continue;
        };
        let Some(value) = normalized_non_empty(&value) else {
            continue;
        };
        normalized.insert(key, value);
    }
    snippet.custom_values = normalized;
    snippet
}

pub fn common_config_snippet_has_payload(snippet: &CommonConfigSnippet) -> bool {
    snippet.hide_attribution
        || snippet.enable_teammates
        || snippet.effort_level_high
        || snippet.enable_tool_search
        || !snippet.custom_values.is_empty()
}

fn load_common_config_snippets_from_conn(
    conn: &rusqlite::Connection,
) -> Result<HashMap<String, CommonConfigSnippet>, String> {
    Ok(get_json_app_setting(conn, COMMON_CONFIG_SNIPPETS_SETTING_KEY)?.unwrap_or_default())
}

pub fn read_common_config_snippet_from_conn(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<CommonConfigSnippet, String> {
    if !is_common_config_tool(tool_id) {
        return Ok(CommonConfigSnippet::default());
    }

    let snippets = load_common_config_snippets_from_conn(conn)?;
    Ok(snippets
        .get(tool_id)
        .cloned()
        .map(normalize_common_config_snippet)
        .unwrap_or_default())
}

pub fn write_common_config_snippet_to_conn(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snippet: CommonConfigSnippet,
) -> Result<CommonConfigSnippet, String> {
    if !is_common_config_tool(tool_id) {
        return Err(format!(
            "Common Config Snippet is not supported for tool: {tool_id}"
        ));
    }

    let mut snippets = load_common_config_snippets_from_conn(conn)?;
    let normalized = normalize_common_config_snippet(snippet);
    if common_config_snippet_has_payload(&normalized) {
        snippets.insert(tool_id.to_string(), normalized.clone());
    } else {
        snippets.remove(tool_id);
    }
    set_json_app_setting(conn, COMMON_CONFIG_SNIPPETS_SETTING_KEY, &snippets)?;
    Ok(normalized)
}

fn resolve_claude_settings_local_path(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    let (_, settings_json_path) = resolve_claude_paths(conn)?;
    let parent = settings_json_path
        .parent()
        .ok_or_else(|| "Invalid Claude settings path".to_string())?;
    Ok(parent.join("settings.local.json"))
}

pub fn read_json_file_or_default(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

pub fn write_json_file_pretty(
    path: &std::path::Path,
    value: &serde_json::Value,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(path, &content).map_err(|e| e.to_string())
}

pub fn read_claude_config_toggles_from_conn(
    conn: &rusqlite::Connection,
) -> Result<ClaudeConfigToggles, String> {
    let path = resolve_claude_settings_local_path(conn)?;
    let settings = read_json_file_or_default(&path)?;
    let env = settings
        .get("env")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();

    let truthy = |key: &str| {
        env.get(key)
            .and_then(|value| value.as_str())
            .map(|value| matches!(value, "1" | "true" | "TRUE" | "True"))
            .unwrap_or(false)
    };

    let max_thinking_tokens_value = env
        .get("CLAUDE_CODE_MAX_THINKING_TOKENS")
        .and_then(|value| value.as_str())
        .unwrap_or("32000")
        .to_string();

    Ok(ClaudeConfigToggles {
        hide_attribution: truthy("ANTHROPIC_HIDE_ATTRIBUTION"),
        enable_teammates: truthy("CLAUDE_CODE_ENABLE_TEAMMATES"),
        max_thinking_tokens: env
            .get("CLAUDE_CODE_MAX_THINKING_TOKENS")
            .and_then(|value| value.as_str())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        max_thinking_tokens_value,
        enable_tool_search: truthy("ENABLE_TOOL_SEARCH"),
    })
}

pub fn write_claude_config_toggle_to_conn(
    conn: &rusqlite::Connection,
    key: &str,
    enabled: bool,
) -> Result<ClaudeConfigToggles, String> {
    let path = resolve_claude_settings_local_path(conn)?;
    let mut settings = read_json_file_or_default(&path)?;

    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    if settings.get("env").is_none() || !settings.get("env").is_some_and(|value| value.is_object())
    {
        settings["env"] = serde_json::json!({});
    }

    let env = settings
        .get_mut("env")
        .and_then(|value| value.as_object_mut())
        .ok_or_else(|| "Claude settings.local env must be an object".to_string())?;

    match key {
        "hideAttribution" => {
            if enabled {
                env.insert(
                    "ANTHROPIC_HIDE_ATTRIBUTION".to_string(),
                    serde_json::json!("true"),
                );
            } else {
                env.remove("ANTHROPIC_HIDE_ATTRIBUTION");
            }
        }
        "enableTeammates" => {
            if enabled {
                env.insert(
                    "CLAUDE_CODE_ENABLE_TEAMMATES".to_string(),
                    serde_json::json!("true"),
                );
            } else {
                env.remove("CLAUDE_CODE_ENABLE_TEAMMATES");
            }
        }
        "maxThinkingTokens" => {
            if enabled {
                env.insert(
                    "CLAUDE_CODE_MAX_THINKING_TOKENS".to_string(),
                    serde_json::json!("32000"),
                );
            } else {
                env.remove("CLAUDE_CODE_MAX_THINKING_TOKENS");
            }
        }
        "enableToolSearch" => {
            if enabled {
                env.insert("ENABLE_TOOL_SEARCH".to_string(), serde_json::json!("true"));
            } else {
                env.remove("ENABLE_TOOL_SEARCH");
            }
        }
        _ => {
            return Err(format!("Unknown Claude config toggle: {key}"));
        }
    }

    if env.is_empty() {
        settings.as_object_mut().map(|value| value.remove("env"));
    }

    write_json_file_pretty(&path, &settings)?;
    read_claude_config_toggles_from_conn(conn)
}

pub fn resolve_codex_structured_paths(
    conn: &rusqlite::Connection,
    path: Option<String>,
) -> Result<(PathBuf, PathBuf), String> {
    let config_path = match path.and_then(|value| normalized_non_empty(&value)) {
        Some(path) => PathBuf::from(path),
        None => resolve_tool_config_dir(conn, "codex")?.join("config.toml"),
    };

    if config_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        != "config.toml"
    {
        return Err(format!(
            "Codex structured editing only supports config.toml: {}",
            config_path.display()
        ));
    }

    let dir = config_path
        .parent()
        .ok_or_else(|| "Invalid Codex config.toml path".to_string())?;
    Ok((config_path.clone(), dir.join("auth.json")))
}
