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

use super::log_command_timing;
use super::proxy_settings::*;
use super::statusline::*;
use super::types::*;

// ── Config Profiles ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigProfile {
    pub id: String,
    pub name: String,
    pub tool_id: String,
    pub config_snapshot: String,
    pub sort_order: i64,
    pub source_type: Option<String>,
    pub source_key: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedConfigProfileInput {
    pub tool_id: String,
    pub config_snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigFragment {
    pub id: String,
    pub name: String,
    pub target_tools: Vec<String>,
    pub fields: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

pub(super) fn tool_config_file_name(tool_id: &str) -> Result<&'static str, String> {
    match tool_id {
        "claude" => Ok("settings.json"),
        "codex" => Ok("config.toml"),
        "gemini" => Ok("settings.json"),
        "opencode" => Ok("opencode.json"),
        "openclaw" => Ok("openclaw.json"),
        "hermes" => Ok("config.yaml"),
        _ => Err(format!("Unknown tool: {}", tool_id)),
    }
}

pub(super) fn default_tool_config_dir(
    home: &std::path::Path,
    tool_id: &str,
) -> Result<PathBuf, String> {
    let dir = match tool_id {
        "claude" => ".claude",
        "codex" => ".codex",
        "gemini" => ".gemini",
        "opencode" => ".opencode",
        "openclaw" => ".openclaw",
        "hermes" => ".hermes",
        _ => return Err(format!("Unknown tool: {}", tool_id)),
    };
    Ok(home.join(dir))
}

pub(super) fn resolve_tool_config_dir(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<PathBuf, String> {
    if tool_id == "hermes" {
        return hermes::hermes_root(conn);
    }

    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let custom_dir: Option<String> = conn
        .query_row(
            "SELECT config_dir FROM custom_paths WHERE tool_id = ?1",
            rusqlite::params![tool_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(dir) = custom_dir.filter(|dir| !dir.trim().is_empty()) {
        return Ok(PathBuf::from(dir));
    }

    let custom_config_path: Option<String> = conn
        .query_row(
            "SELECT mcp_config_path FROM custom_paths WHERE tool_id = ?1",
            rusqlite::params![tool_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(path) = custom_config_path.filter(|path| !path.trim().is_empty()) {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            return Ok(parent.to_path_buf());
        }
    }

    default_tool_config_dir(&home, tool_id)
}

pub(super) fn resolve_tool_config_path(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<PathBuf, String> {
    if tool_id == "hermes" {
        return hermes::config_path(conn);
    }
    Ok(resolve_tool_config_dir(conn, tool_id)?.join(tool_config_file_name(tool_id)?))
}

pub(super) fn resolve_claude_paths(
    conn: &rusqlite::Connection,
) -> Result<(PathBuf, PathBuf), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let custom_dir: Option<String> = conn
        .query_row(
            "SELECT config_dir FROM custom_paths WHERE tool_id = 'claude'",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let settings_json = if let Some(dir) = custom_dir.filter(|dir| !dir.trim().is_empty()) {
        PathBuf::from(dir).join("settings.json")
    } else {
        home.join(".claude").join("settings.json")
    };

    let custom_mcp_path: Option<String> = conn
        .query_row(
            "SELECT mcp_config_path FROM custom_paths WHERE tool_id = 'claude'",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let claude_json = if let Some(path) = custom_mcp_path.filter(|path| !path.trim().is_empty()) {
        PathBuf::from(path)
    } else {
        home.join(".claude.json")
    };

    Ok((claude_json, settings_json))
}

pub(super) fn resolve_tool_skills_dir(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<PathBuf, String> {
    if tool_id == "hermes" {
        return hermes::skills_dir(conn);
    }

    let custom_skills_dir: Option<String> = conn
        .query_row(
            "SELECT skills_dir FROM custom_paths WHERE tool_id = ?1",
            rusqlite::params![tool_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(dir) = custom_skills_dir.filter(|dir| !dir.trim().is_empty()) {
        return Ok(PathBuf::from(dir));
    }

    Ok(resolve_tool_config_dir(conn, tool_id)?.join("skills"))
}

pub(super) fn tool_cli_command(tool_id: &str) -> &'static str {
    match tool_id {
        "claude" => "claude",
        "codex" => "codex",
        "gemini" => "gemini",
        "opencode" => "opencode",
        "openclaw" => "openclaw",
        "hermes" => "hermes",
        _ => "",
    }
}

pub(super) fn is_executable_file(path: &std::path::Path) -> bool {
    path.is_file()
}

pub(super) fn cli_exists_in_path(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }

    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };

    let path_exts: Vec<String> = if cfg!(windows) {
        std::env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| {
                vec![
                    ".EXE".to_string(),
                    ".CMD".to_string(),
                    ".BAT".to_string(),
                    ".COM".to_string(),
                ]
            })
    } else {
        Vec::new()
    };

    for dir in std::env::split_paths(&path_var) {
        let direct = dir.join(command);
        if is_executable_file(&direct) {
            return true;
        }

        if cfg!(windows) {
            for ext in &path_exts {
                let ext = ext.trim();
                if ext.is_empty() {
                    continue;
                }
                let normalized_ext = if ext.starts_with('.') {
                    ext.to_string()
                } else {
                    format!(".{}", ext)
                };
                let candidate = dir.join(format!("{command}{normalized_ext}"));
                if is_executable_file(&candidate) {
                    return true;
                }
            }
        }
    }

    false
}

pub(super) fn tool_label(tool_id: &str) -> &'static str {
    match tool_id {
        "claude" => "Claude",
        "codex" => "Codex",
        "gemini" => "Gemini",
        "opencode" => "OpenCode",
        "openclaw" => "OpenClaw",
        "hermes" => "Hermes",
        _ => "Session",
    }
}

pub(super) fn tool_hidden_dir(tool_id: &str) -> Option<&'static str> {
    match tool_id {
        "claude" => Some(".claude"),
        "codex" => Some(".codex"),
        "gemini" => Some(".gemini"),
        "opencode" => Some(".opencode"),
        "openclaw" => Some(".openclaw"),
        "hermes" => Some(".hermes"),
        _ => None,
    }
}

pub(super) fn format_unix_timestamp(value: i64) -> Option<String> {
    if value <= 0 {
        return None;
    }

    let (seconds, nanos) = if value > 10_000_000_000 {
        let seconds = value / 1000;
        let remainder = (value % 1000).unsigned_abs() as u32;
        (seconds, remainder.saturating_mul(1_000_000))
    } else {
        (value, 0)
    };

    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, nanos).map(|datetime| {
        datetime
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string()
    })
}

pub(super) fn format_timestamp_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(parsed) = trimmed.parse::<i64>() {
        return format_unix_timestamp(parsed);
    }

    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(
            parsed
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string(),
        );
    }

    Some(trimmed.chars().take(19).collect())
}

pub(super) fn truncate_session_text(text: &str, max_chars: usize) -> String {
    let condensed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if condensed.chars().count() <= max_chars {
        condensed
    } else {
        let mut result = condensed.chars().take(max_chars).collect::<String>();
        result.push_str("...");
        result
    }
}

pub(super) fn count_query_hits(query: &str, values: &[String]) -> usize {
    if query.is_empty() {
        return 0;
    }

    values
        .iter()
        .filter(|value| value.to_lowercase().contains(query))
        .count()
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct SessionTokenTotals {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    has_usage: bool,
}

impl SessionTokenTotals {
    fn record(
        &mut self,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) {
        let resolved_input = input_tokens.unwrap_or(0);
        let resolved_output = output_tokens.unwrap_or(0);
        let resolved_total =
            total_tokens.unwrap_or_else(|| resolved_input.saturating_add(resolved_output));

        if resolved_input == 0 && resolved_output == 0 && resolved_total == 0 {
            return;
        }

        self.input_tokens = self.input_tokens.saturating_add(resolved_input);
        self.output_tokens = self.output_tokens.saturating_add(resolved_output);
        self.total_tokens = self.total_tokens.saturating_add(resolved_total);
        self.has_usage = true;
    }

    pub(super) fn input_option(self) -> Option<u64> {
        self.has_usage.then_some(self.input_tokens)
    }

    pub(super) fn output_option(self) -> Option<u64> {
        self.has_usage.then_some(self.output_tokens)
    }

    pub(super) fn total_option(self) -> Option<u64> {
        self.has_usage.then_some(self.total_tokens)
    }
}

pub(super) fn read_token_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

pub(super) fn object_usage_totals(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<(Option<u64>, Option<u64>, Option<u64>)> {
    let input_tokens = [
        "input_tokens",
        "prompt_tokens",
        "inputTokenCount",
        "inputTokens",
    ]
    .iter()
    .find_map(|key| map.get(*key).and_then(read_token_u64));
    let output_tokens = [
        "output_tokens",
        "completion_tokens",
        "candidatesTokenCount",
        "outputTokenCount",
        "outputTokens",
    ]
    .iter()
    .find_map(|key| map.get(*key).and_then(read_token_u64));
    let total_tokens = ["total_tokens", "totalTokenCount", "totalTokens"]
        .iter()
        .find_map(|key| map.get(*key).and_then(read_token_u64));

    (input_tokens.is_some() || output_tokens.is_some() || total_tokens.is_some()).then_some((
        input_tokens,
        output_tokens,
        total_tokens,
    ))
}

pub(super) fn accumulate_token_usage_from_value(
    value: &serde_json::Value,
    totals: &mut SessionTokenTotals,
    depth: usize,
) {
    if depth > 8 {
        return;
    }

    match value {
        serde_json::Value::Object(map) => {
            if let Some((input_tokens, output_tokens, total_tokens)) = object_usage_totals(map) {
                totals.record(input_tokens, output_tokens, total_tokens);
                return;
            }

            for child in map.values() {
                accumulate_token_usage_from_value(child, totals, depth + 1);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                accumulate_token_usage_from_value(item, totals, depth + 1);
            }
        }
        _ => {}
    }
}

pub(super) fn normalize_session_query(query: Option<String>) -> String {
    query.unwrap_or_default().trim().to_lowercase()
}

pub(super) fn session_roots_for_tool(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(global_root) = resolve_tool_config_dir(conn, tool_id) {
        if global_root.exists() {
            let key = global_root.to_string_lossy().to_string();
            if seen.insert(key) {
                roots.push(global_root);
            }
        }
    }

    if let Some(hidden_dir) = tool_hidden_dir(tool_id) {
        for project_root in discover_project_roots(conn) {
            let session_root = project_root.join(hidden_dir);
            if !session_root.exists() {
                continue;
            }
            let key = session_root.to_string_lossy().to_string();
            if seen.insert(key) {
                roots.push(session_root);
            }
        }
    }

    Ok(roots)
}

pub(super) fn is_session_candidate_path(
    tool_id: &str,
    path: &std::path::Path,
    base_dir: &std::path::Path,
) -> bool {
    if !path.is_file() {
        return false;
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let relative = match path.strip_prefix(base_dir) {
        Ok(relative) => relative.to_string_lossy().to_ascii_lowercase(),
        Err(_) => path.to_string_lossy().to_ascii_lowercase(),
    };

    let has_keyword = [
        "session",
        "sessions",
        "history",
        "conversation",
        "conversations",
        "thread",
        "threads",
        "chat",
        "rollout",
        "transcript",
        "project",
    ]
    .iter()
    .any(|keyword| relative.contains(keyword));

    match extension.as_deref() {
        Some("jsonl") => {
            if !has_keyword {
                return false;
            }
            // Skip Claude agent sub-sessions (e.g. agent-a54b9a9c979dbd77c.jsonl)
            if tool_id == "claude" {
                if let Some(stem) = path.file_stem().and_then(|v| v.to_str()) {
                    if stem.starts_with("agent-") {
                        return false;
                    }
                }
            }
            true
        }
        Some("sqlite" | "db") => has_keyword || tool_id == "opencode",
        _ => false,
    }
}

pub(super) fn collect_session_candidate_files(
    tool_id: &str,
    current_dir: &std::path::Path,
    base_dir: &std::path::Path,
    jsonl_files: &mut Vec<PathBuf>,
    sqlite_files: &mut Vec<PathBuf>,
    depth: usize,
) {
    if depth > 5 {
        return;
    }

    let read_dir = match std::fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(
                dir_name.as_str(),
                ".git" | "node_modules" | "dist" | "target"
            ) {
                continue;
            }
            collect_session_candidate_files(
                tool_id,
                &path,
                base_dir,
                jsonl_files,
                sqlite_files,
                depth + 1,
            );
            continue;
        }

        if !is_session_candidate_path(tool_id, &path, base_dir) {
            continue;
        }

        match path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("jsonl") => jsonl_files.push(path),
            Some("sqlite" | "db") => sqlite_files.push(path),
            _ => {}
        }
    }
}

pub(super) fn preferred_texts_from_value(
    value: &serde_json::Value,
    texts: &mut Vec<String>,
    depth: usize,
) {
    if depth > 4 || texts.len() >= 8 {
        return;
    }

    match value {
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                texts.push(trimmed.to_string());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter().take(8) {
                preferred_texts_from_value(item, texts, depth + 1);
                if texts.len() >= 8 {
                    break;
                }
            }
        }
        serde_json::Value::Object(map) => {
            for key in [
                "text", "message", "content", "preview", "prompt", "output", "title",
            ] {
                if let Some(child) = map.get(key) {
                    preferred_texts_from_value(child, texts, depth + 1);
                    if texts.len() >= 8 {
                        return;
                    }
                }
            }
            for key in ["payload", "items", "messages", "data"] {
                if let Some(child) = map.get(key) {
                    preferred_texts_from_value(child, texts, depth + 1);
                    if texts.len() >= 8 {
                        return;
                    }
                }
            }
        }
        _ => {}
    }
}

pub(super) fn read_session_token_totals_from_jsonl(path: &std::path::Path) -> SessionTokenTotals {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return SessionTokenTotals::default(),
    };

    let mut totals = SessionTokenTotals::default();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        accumulate_token_usage_from_value(&value, &mut totals, 0);
    }

    totals
}

/// Resolve the full path to a CLI tool executable (returned WITHOUT quotes).
/// On Windows, checks npm global bin first, then `where`.
/// Falls back to the bare command name if nothing is found.
pub(super) fn resolve_cli_path(cmd: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        // 1) npm global bin — most Node.js CLI tools live here
        if let Ok(appdata) = std::env::var("APPDATA") {
            let npm_cmd = std::path::PathBuf::from(&appdata)
                .join("npm")
                .join(format!("{cmd}.cmd"));
            if npm_cmd.exists() {
                return npm_cmd.to_string_lossy().to_string();
            }
        }
        // 2) `where` — may return system shims (e.g. C:\Windows\claude.exe)
        let mut process = std::process::Command::new("where");
        configure_background_command(&mut process);
        if let Ok(output) = process.arg(cmd).output() {
            if output.status.success() {
                let mut fallback = None;
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let p = line.trim();
                    if p.is_empty() {
                        continue;
                    }
                    let path = std::path::PathBuf::from(p);
                    if !path.exists() {
                        continue;
                    }
                    let lower = p.to_ascii_lowercase();
                    let is_windows_alias = lower.starts_with("c:\\windows\\")
                        && (lower.ends_with(&format!("\\{cmd}.exe"))
                            || lower.ends_with(&format!("\\{cmd}.cmd")));
                    let is_cmd_wrapper = lower.ends_with(".cmd")
                        || lower.ends_with(".bat")
                        || lower.ends_with(".ps1");

                    if !is_windows_alias && is_cmd_wrapper {
                        return p.to_string();
                    }
                    if !is_windows_alias && fallback.is_none() {
                        fallback = Some(p.to_string());
                    }
                }
                if let Some(path) = fallback {
                    return path;
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = std::process::Command::new("which").arg(cmd).output() {
            if output.status.success() {
                if let Some(line) = String::from_utf8_lossy(&output.stdout).lines().next() {
                    let p = line.trim();
                    if !p.is_empty() {
                        return p.to_string();
                    }
                }
            }
        }
    }
    cmd.to_string()
}

/// Quote a CLI path for embedding in a shell command string.
/// Only adds quotes when the path contains spaces.
pub(super) fn shell_quote_cli(path: &str) -> String {
    if path.contains(' ') {
        format!("\"{}\"", path)
    } else {
        path.to_string()
    }
}

pub(super) fn codex_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("codex"));
    format!("{cli} resume {session_id}")
}

pub(super) fn claude_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("claude"));
    format!("{cli} --resume {session_id}")
}

pub(super) fn gemini_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("gemini"));
    format!("{cli} --resume {session_id}")
}

pub(super) fn opencode_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("opencode"));
    format!("{cli} session resume {session_id}")
}

pub(super) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn resolve_openclaw_session_key(
    source_path: Option<&str>,
    session_id: &str,
) -> Result<String, String> {
    let source_path = source_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Missing OpenClaw session source path".to_string())?;
    let source = PathBuf::from(source_path);
    let index_path = source
        .parent()
        .ok_or_else(|| format!("Invalid OpenClaw session path: {source_path}"))?
        .join("sessions.json");
    let content = std::fs::read_to_string(&index_path).map_err(|e| {
        format!(
            "Failed to read OpenClaw sessions index {}: {e}",
            index_path.display()
        )
    })?;
    let parsed: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        format!(
            "Failed to parse OpenClaw sessions index {}: {e}",
            index_path.display()
        )
    })?;
    let obj = parsed.as_object().ok_or_else(|| {
        format!(
            "OpenClaw sessions index is not a JSON object: {}",
            index_path.display()
        )
    })?;

    for (session_key, entry) in obj {
        let same_id = entry.get("sessionId").and_then(|value| value.as_str()) == Some(session_id);
        let same_file = entry
            .get("sessionFile")
            .and_then(|value| value.as_str())
            .map(|value| Path::new(value) == source)
            .unwrap_or(false);
        if same_id || same_file {
            return Ok(session_key.clone());
        }
    }

    Err(format!(
        "OpenClaw session key not found for session {session_id} in {}",
        index_path.display()
    ))
}

pub(super) fn openclaw_resume_command(
    source_path: Option<&str>,
    session_id: &str,
) -> Result<String, String> {
    let session_key = resolve_openclaw_session_key(source_path, session_id)?;
    Ok(format!(
        "openclaw tui --session {}",
        shell_single_quote(&session_key)
    ))
}

pub(super) fn tool_supports_session_resume(tool_id: &str) -> bool {
    match tool_id {
        "codex" => cli_exists_in_path("codex"),
        "claude" => cli_exists_in_path("claude"),
        "gemini" => cli_exists_in_path("gemini"),
        "opencode" => cli_exists_in_path("opencode"),
        "openclaw" => cli_exists_in_path("openclaw"),
        _ => false,
    }
}

fn write_default_file_if_missing(
    path: &std::path::Path,
    content: &str,
    created_files: &mut usize,
) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(path, content).map_err(|e| e.to_string())?;
    *created_files += 1;
    Ok(())
}

fn ensure_dir_exists(path: &std::path::Path, created_dirs: &mut usize) -> Result<(), String> {
    if path.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    *created_dirs += 1;
    Ok(())
}

pub(super) fn bootstrap_tool_environment_from_conn(
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

pub(super) fn json_file_has_content(path: &std::path::Path) -> bool {
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

pub(super) fn gemini_env_has_api_key(path: &std::path::Path) -> bool {
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

pub(super) fn open_target_in_system(target: &str) -> Result<(), String> {
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

pub(super) fn set_text_app_setting(
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

fn get_text_app_setting(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>, String> {
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .ok())
}

const MANAGED_APP_IDS: [&str; 6] = [
    "claude", "codex", "gemini", "opencode", "openclaw", "hermes",
];
pub(super) const VISIBLE_APPS_SETTING_KEY: &str = "visible_apps";
pub(super) const WINDOW_PREFERENCES_SETTING_KEY: &str = "window_preferences";
pub(super) const COMMON_CONFIG_SNIPPETS_SETTING_KEY: &str = "common_config_snippets";
pub(super) const WELCOME_COMPLETED_SETTING_KEY: &str = "welcome_completed";

fn is_common_config_tool(tool_id: &str) -> bool {
    matches!(tool_id, "claude" | "codex" | "gemini")
}

fn normalize_integer_like(value: &str) -> Option<i64> {
    let normalized = value.trim().replace(['_', ',', ' '], "");
    if normalized.is_empty() {
        return None;
    }
    normalized.parse::<i64>().ok()
}

pub(super) fn normalized_non_empty(value: &str) -> Option<String> {
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

fn common_config_snippet_has_payload(snippet: &CommonConfigSnippet) -> bool {
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

pub(super) fn read_common_config_snippet_from_conn(
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

pub(super) fn write_common_config_snippet_to_conn(
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

pub(super) fn read_json_file_or_default(
    path: &std::path::Path,
) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

pub(super) fn write_json_file_pretty(
    path: &std::path::Path,
    value: &serde_json::Value,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(path, &content).map_err(|e| e.to_string())
}

pub(super) fn read_claude_config_toggles_from_conn(
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

pub(super) fn write_claude_config_toggle_to_conn(
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

pub(super) fn resolve_codex_structured_paths(
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
pub(super) const PREFERRED_TERMINAL_SETTING_KEY: &str = "preferred_terminal";
pub(super) const BACKUP_PREFERENCES_SETTING_KEY: &str = "backup_preferences";
pub(super) const LOG_PREFERENCES_SETTING_KEY: &str = "log_preferences";
pub(super) const PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY: &str = "provider_config_fragments";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPreferences {
    pub launch_at_login: bool,
    pub launch_hidden: bool,
    pub close_to_tray: bool,
    pub lightweight_mode: bool,
}

impl Default for WindowPreferences {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            launch_hidden: false,
            close_to_tray: true,
            lightweight_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalOption {
    pub id: String,
    pub label: String,
    pub command: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalPreferences {
    pub platform: String,
    pub selected_terminal: String,
    pub options: Vec<TerminalOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConflict {
    pub id: String,
    pub kind: String,
    pub variables: Vec<String>,
    pub affected_apps: Vec<String>,
}

pub(super) fn default_visible_apps() -> Vec<String> {
    MANAGED_APP_IDS.iter().map(|id| (*id).to_string()).collect()
}

pub(super) fn normalize_visible_apps(visible_apps: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for app_id in visible_apps {
        let trimmed = app_id.trim();
        if MANAGED_APP_IDS.contains(&trimmed) && seen.insert(trimmed.to_string()) {
            normalized.push(trimmed.to_string());
        }
    }

    if normalized.is_empty() {
        normalized.push("claude".to_string());
    }

    normalized
}

pub(super) fn read_backup_preferences_from_conn(conn: &rusqlite::Connection) -> BackupPreferences {
    let mut preferences: BackupPreferences =
        get_json_app_setting(conn, BACKUP_PREFERENCES_SETTING_KEY)
            .ok()
            .flatten()
            .unwrap_or_default();
    if preferences.retention_count == 0 {
        preferences.retention_count = BackupPreferences::default().retention_count;
    }
    preferences
}

pub(super) fn normalize_log_level(level: &str) -> String {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" | "warn" | "info" | "debug" | "trace" => level.trim().to_ascii_lowercase(),
        _ => "error".to_string(),
    }
}

pub fn read_log_preferences_from_conn(conn: &rusqlite::Connection) -> LogPreferences {
    let mut preferences: LogPreferences = get_json_app_setting(conn, LOG_PREFERENCES_SETTING_KEY)
        .ok()
        .flatten()
        .unwrap_or_default();
    preferences.level = normalize_log_level(&preferences.level);
    preferences
}

pub fn apply_log_preferences(preferences: &LogPreferences) {
    let level = normalize_log_level(&preferences.level);
    std::env::set_var("CCHUB_LOG_LEVEL", &level);
    std::env::set_var("RUST_LOG", &level);
    std::env::set_var(
        "RUST_BACKTRACE",
        if matches!(level.as_str(), "debug" | "trace") {
            "full"
        } else {
            "1"
        },
    );
}

pub(super) fn build_log_file_targets() -> LogFileTargets {
    LogFileTargets {
        runtime_log_path: crate::utils::runtime_log_path()
            .to_string_lossy()
            .to_string(),
        crash_log_path: crate::utils::crash_log_path().to_string_lossy().to_string(),
    }
}

fn read_disable_auto_updater_env() -> Option<String> {
    std::env::var("DISABLE_AUTOUPDATER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn updater_environment_state() -> UpdaterEnvironmentState {
    let env_var_value = read_disable_auto_updater_env();
    let normalized = env_var_value
        .as_deref()
        .map(|value| value.to_ascii_lowercase());

    UpdaterEnvironmentState {
        disabled_by_env: matches!(normalized.as_deref(), Some("1" | "true" | "yes" | "on")),
        env_var_value,
    }
}

fn log_level_for_provider_status(status: &str) -> &'static str {
    match status {
        "error" => "warn",
        "healthy" | "reachable" | "fast" | "medium" | "slow" => "info",
        _ => "debug",
    }
}

fn log_provider_result(
    kind: &str,
    tool_id: &str,
    provider_name: &str,
    base_url: Option<&str>,
    status: &str,
    message: &str,
) {
    let target = base_url.unwrap_or("n/a");
    crate::utils::append_runtime_log(
        log_level_for_provider_status(status),
        "providers",
        &format!("{kind} [{tool_id}] {provider_name} -> {target} [{status}] {message}"),
    );
}

pub fn read_window_preferences_from_conn(conn: &rusqlite::Connection) -> WindowPreferences {
    get_json_app_setting(conn, WINDOW_PREFERENCES_SETTING_KEY)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn current_platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

#[cfg(target_os = "macos")]
fn macos_app_exists(name: &str) -> bool {
    let mut candidates = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("Applications"));
    }

    candidates
        .into_iter()
        .any(|base| base.join(format!("{name}.app")).exists())
}

fn terminal_options_for_current_platform() -> Vec<TerminalOption> {
    #[cfg(target_os = "windows")]
    {
        vec![
            TerminalOption {
                id: "windows-terminal".to_string(),
                label: "Windows Terminal".to_string(),
                command: "wt".to_string(),
                installed: cli_exists_in_path("wt"),
            },
            TerminalOption {
                id: "powershell".to_string(),
                label: "PowerShell".to_string(),
                command: "powershell".to_string(),
                installed: cli_exists_in_path("powershell"),
            },
            TerminalOption {
                id: "cmd".to_string(),
                label: "Command Prompt".to_string(),
                command: "cmd".to_string(),
                installed: cli_exists_in_path("cmd"),
            },
        ]
    }

    #[cfg(target_os = "macos")]
    {
        return vec![
            TerminalOption {
                id: "terminal".to_string(),
                label: "Terminal".to_string(),
                command: "open -a Terminal".to_string(),
                installed: macos_app_exists("Terminal"),
            },
            TerminalOption {
                id: "iterm2".to_string(),
                label: "iTerm".to_string(),
                command: "open -a iTerm".to_string(),
                installed: macos_app_exists("iTerm"),
            },
            TerminalOption {
                id: "warp".to_string(),
                label: "Warp".to_string(),
                command: "open -a Warp".to_string(),
                installed: macos_app_exists("Warp"),
            },
            TerminalOption {
                id: "ghostty".to_string(),
                label: "Ghostty".to_string(),
                command: "open -a Ghostty".to_string(),
                installed: macos_app_exists("Ghostty"),
            },
            TerminalOption {
                id: "kaku".to_string(),
                label: "Kaku".to_string(),
                command: "open -a Kaku".to_string(),
                installed: macos_app_exists("Kaku"),
            },
            TerminalOption {
                id: "kitty".to_string(),
                label: "Kitty".to_string(),
                command: "kitty".to_string(),
                installed: cli_exists_in_path("kitty"),
            },
            TerminalOption {
                id: "alacritty".to_string(),
                label: "Alacritty".to_string(),
                command: "alacritty".to_string(),
                installed: cli_exists_in_path("alacritty"),
            },
        ];
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            TerminalOption {
                id: "gnome-terminal".to_string(),
                label: "GNOME Terminal".to_string(),
                command: "gnome-terminal".to_string(),
                installed: cli_exists_in_path("gnome-terminal"),
            },
            TerminalOption {
                id: "konsole".to_string(),
                label: "Konsole".to_string(),
                command: "konsole".to_string(),
                installed: cli_exists_in_path("konsole"),
            },
            TerminalOption {
                id: "xterm".to_string(),
                label: "xterm".to_string(),
                command: "xterm".to_string(),
                installed: cli_exists_in_path("xterm"),
            },
            TerminalOption {
                id: "kitty".to_string(),
                label: "Kitty".to_string(),
                command: "kitty".to_string(),
                installed: cli_exists_in_path("kitty"),
            },
            TerminalOption {
                id: "alacritty".to_string(),
                label: "Alacritty".to_string(),
                command: "alacritty".to_string(),
                installed: cli_exists_in_path("alacritty"),
            },
            TerminalOption {
                id: "wezterm".to_string(),
                label: "WezTerm".to_string(),
                command: "wezterm".to_string(),
                installed: cli_exists_in_path("wezterm"),
            },
        ]
    }
}

pub(super) fn read_terminal_preferences_from_conn(
    conn: &rusqlite::Connection,
) -> Result<TerminalPreferences, String> {
    let options = terminal_options_for_current_platform();
    let stored = get_text_app_setting(conn, PREFERRED_TERMINAL_SETTING_KEY)?;

    let selected_terminal = stored
        .filter(|terminal_id| options.iter().any(|option| option.id == *terminal_id))
        .or_else(|| {
            options
                .iter()
                .find(|option| option.installed)
                .map(|option| option.id.clone())
        })
        .or_else(|| options.first().map(|option| option.id.clone()))
        .unwrap_or_default();

    Ok(TerminalPreferences {
        platform: current_platform_name().to_string(),
        selected_terminal,
        options,
    })
}

#[allow(dead_code)]
fn shell_quote_single(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'"'"'"#))
}

pub(super) fn normalize_terminal_target(path: Option<String>) -> Result<PathBuf, String> {
    let base = match path.filter(|value| !value.trim().is_empty()) {
        Some(path) => PathBuf::from(path),
        None => dirs::home_dir().ok_or("Cannot find home directory")?,
    };

    if base.is_dir() {
        return Ok(base);
    }

    if base.is_file() {
        return base
            .parent()
            .map(|parent| parent.to_path_buf())
            .ok_or_else(|| "Cannot determine file parent directory".to_string());
    }

    Err(format!("Path does not exist: {}", base.display()))
}

pub(super) fn launch_preferred_terminal_impl(
    preferences: &TerminalPreferences,
    target_dir: &std::path::Path,
    shell_command: Option<&str>,
) -> Result<bool, String> {
    let target_text = target_dir.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        // Use raw_arg to bypass Rust's msvcrt arg escaping which causes
        // quote-nesting issues with cmd.exe / wt on Windows.
        use std::os::windows::process::CommandExt;

        if let Some(command) = shell_command {
            match preferences.selected_terminal.as_str() {
                "windows-terminal" => {
                    std::process::Command::new("wt")
                        .raw_arg(format!("-d \"{}\" cmd.exe /K {}", target_text, command))
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "powershell" => {
                    let ps_cmd = format!(
                        "Set-Location -LiteralPath '{}'; {}",
                        target_text.replace('\'', "''"),
                        command,
                    );
                    std::process::Command::new("powershell")
                        .raw_arg(format!("-NoExit -Command \"{}\"", ps_cmd))
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "cmd" => {
                    std::process::Command::new("cmd.exe")
                        .raw_arg(format!("/K cd /d \"{}\" && {}", target_text, command))
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                _ => {
                    return Err(format!(
                        "Unsupported terminal: {}",
                        preferences.selected_terminal
                    ))
                }
            }
            return Ok(true);
        }

        match preferences.selected_terminal.as_str() {
            "windows-terminal" => {
                std::process::Command::new("wt")
                    .raw_arg(format!("-d \"{}\"", target_text))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "powershell" => {
                let ps_cmd = format!(
                    "Set-Location -LiteralPath '{}'",
                    target_text.replace('\'', "''")
                );
                std::process::Command::new("powershell")
                    .raw_arg(format!("-NoExit -Command \"{}\"", ps_cmd))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "cmd" => {
                std::process::Command::new("cmd.exe")
                    .raw_arg(format!("/K cd /d \"{}\"", target_text))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                return Err(format!(
                    "Unsupported terminal: {}",
                    preferences.selected_terminal
                ))
            }
        }
        return Ok(true);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(command) = shell_command {
            let shell_line = format!(
                "cd {} && {} ; exec bash",
                shell_quote_single(&target_text),
                command,
            );
            match preferences.selected_terminal.as_str() {
                "kitty" => {
                    std::process::Command::new("kitty")
                        .args(["--directory", &target_text, "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(true);
                }
                "alacritty" => {
                    std::process::Command::new("alacritty")
                        .args([
                            "--working-directory",
                            &target_text,
                            "-e",
                            "bash",
                            "-lc",
                            &shell_line,
                        ])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(true);
                }
                "terminal" => {
                    std::process::Command::new("open")
                        .args(["-a", "Terminal", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                "iterm2" => {
                    std::process::Command::new("open")
                        .args(["-a", "iTerm", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                "warp" => {
                    std::process::Command::new("open")
                        .args(["-a", "Warp", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                "ghostty" => {
                    std::process::Command::new("open")
                        .args(["-a", "Ghostty", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                "kaku" => {
                    std::process::Command::new("open")
                        .args(["-a", "Kaku", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                _ => {
                    return Err(format!(
                        "Unsupported terminal: {}",
                        preferences.selected_terminal
                    ))
                }
            }
        }

        match preferences.selected_terminal.as_str() {
            "terminal" => {
                std::process::Command::new("open")
                    .args(["-a", "Terminal", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "iterm2" => {
                std::process::Command::new("open")
                    .args(["-a", "iTerm", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "warp" => {
                std::process::Command::new("open")
                    .args(["-a", "Warp", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "ghostty" => {
                std::process::Command::new("open")
                    .args(["-a", "Ghostty", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "kaku" => {
                std::process::Command::new("open")
                    .args(["-a", "Kaku", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "kitty" => {
                std::process::Command::new("kitty")
                    .args(["--directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "alacritty" => {
                std::process::Command::new("alacritty")
                    .args(["--working-directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                return Err(format!(
                    "Unsupported terminal: {}",
                    preferences.selected_terminal
                ))
            }
        }
        return Ok(true);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(command) = shell_command {
            let shell_line = format!(
                "cd {} && {} ; exec bash",
                shell_quote_single(&target_text),
                command,
            );
            match preferences.selected_terminal.as_str() {
                "gnome-terminal" => {
                    std::process::Command::new("gnome-terminal")
                        .args([
                            "--working-directory",
                            &target_text,
                            "--",
                            "bash",
                            "-lc",
                            &shell_line,
                        ])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "konsole" => {
                    std::process::Command::new("konsole")
                        .args(["--workdir", &target_text, "-e", "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "xterm" => {
                    std::process::Command::new("xterm")
                        .args(["-e", "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "kitty" => {
                    std::process::Command::new("kitty")
                        .args(["--directory", &target_text, "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "alacritty" => {
                    std::process::Command::new("alacritty")
                        .args([
                            "--working-directory",
                            &target_text,
                            "-e",
                            "bash",
                            "-lc",
                            &shell_line,
                        ])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "wezterm" => {
                    std::process::Command::new("wezterm")
                        .args(["start", "--cwd", &target_text, "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                _ => {
                    return Err(format!(
                        "Unsupported terminal: {}",
                        preferences.selected_terminal
                    ))
                }
            }
            return Ok(true);
        }

        match preferences.selected_terminal.as_str() {
            "gnome-terminal" => {
                std::process::Command::new("gnome-terminal")
                    .args(["--working-directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "konsole" => {
                std::process::Command::new("konsole")
                    .args(["--workdir", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "xterm" => {
                std::process::Command::new("xterm")
                    .args([
                        "-e",
                        "bash",
                        "-lc",
                        &format!("cd {} && exec bash", shell_quote_single(&target_text)),
                    ])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "kitty" => {
                std::process::Command::new("kitty")
                    .args(["--directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "alacritty" => {
                std::process::Command::new("alacritty")
                    .args(["--working-directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "wezterm" => {
                std::process::Command::new("wezterm")
                    .args(["start", "--cwd", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                return Err(format!(
                    "Unsupported terminal: {}",
                    preferences.selected_terminal
                ))
            }
        }
        return Ok(true);
    }

    #[allow(unreachable_code)]
    Ok(false)
}

pub(super) fn build_session_resume_command(
    tool_id: &str,
    session_id: &str,
    source_path: Option<&str>,
) -> Result<String, String> {
    match tool_id {
        "codex" => Ok(codex_resume_command(session_id)),
        "claude" => Ok(claude_resume_command(session_id)),
        "gemini" => Ok(gemini_resume_command(session_id)),
        "opencode" => Ok(opencode_resume_command(session_id)),
        "openclaw" => openclaw_resume_command(source_path, session_id),
        _ => Err(format!("Session restore is not supported for {tool_id}")),
    }
}

#[cfg(target_os = "windows")]
fn autostart_entry_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA is not set".to_string())?;
    Ok(PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join("CCHub.cmd"))
}

#[cfg(target_os = "windows")]
fn autostart_entry_content(exe: &std::path::Path) -> String {
    format!("@echo off\r\nstart \"\" \"{}\"\r\n", exe.display())
}

#[cfg(target_os = "macos")]
fn autostart_entry_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join("com.cchub.app.plist"))
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(target_os = "macos")]
fn autostart_entry_content(exe: &std::path::Path) -> String {
    let exe = xml_escape(&exe.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.cchub.app</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn autostart_entry_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir().ok_or("Cannot find config directory")?;
    Ok(config_dir.join("autostart").join("com.cchub.app.desktop"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn autostart_entry_content(exe: &std::path::Path) -> String {
    let escaped = exe.to_string_lossy().replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=CCHub\nExec=\"{escaped}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

pub(super) fn sync_launch_at_login(enabled: bool) -> Result<(), String> {
    let path = autostart_entry_path()?;

    if !enabled {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &autostart_entry_content(&exe))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn scan_environment_conflicts() -> Vec<EnvironmentConflict> {
    let env_groups = [
        (
            "claude",
            vec![
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "ANTHROPIC_BASE_URL",
                "ANTHROPIC_MODEL",
            ],
        ),
        (
            "codex",
            vec![
                "OPENAI_API_KEY",
                "OPENAI_BASE_URL",
                "OPENAI_ORG_ID",
                "OPENAI_MODEL",
            ],
        ),
        (
            "gemini",
            vec![
                "GEMINI_API_KEY",
                "GOOGLE_API_KEY",
                "GOOGLE_GEMINI_BASE_URL",
                "GEMINI_MODEL",
            ],
        ),
    ];

    let mut conflicts = Vec::new();
    let mut apps_with_overrides = Vec::new();
    let mut all_variables = Vec::new();

    for (app_id, keys) in env_groups {
        let variables: Vec<String> = keys
            .into_iter()
            .filter(|key| {
                std::env::var(key)
                    .ok()
                    .is_some_and(|value| !value.trim().is_empty())
            })
            .map(str::to_string)
            .collect();

        if variables.is_empty() {
            continue;
        }

        all_variables.extend(variables.iter().cloned());
        apps_with_overrides.push(app_id.to_string());
        conflicts.push(EnvironmentConflict {
            id: format!("{app_id}_env_override"),
            kind: "tool_override".to_string(),
            variables,
            affected_apps: vec![app_id.to_string()],
        });
    }

    if apps_with_overrides.len() >= 2 {
        conflicts.insert(
            0,
            EnvironmentConflict {
                id: "shared_env_overrides".to_string(),
                kind: "multi_tool_override".to_string(),
                variables: all_variables,
                affected_apps: apps_with_overrides,
            },
        );
    }

    conflicts
}

fn candidate_home_dirs() -> Vec<PathBuf> {
    let mut homes = Vec::new();

    if let Some(home) = dirs::home_dir() {
        homes.push(home);
    }

    for key in ["USERPROFILE", "HOME"] {
        if let Ok(value) = std::env::var(key) {
            let path = PathBuf::from(value);
            if !homes.iter().any(|item| item == &path) {
                homes.push(path);
            }
        }
    }

    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        let home = PathBuf::from(format!("{}{}", drive, path));
        if !homes.iter().any(|item| item == &home) {
            homes.push(home);
        }
    }

    #[cfg(target_family = "unix")]
    {
        let mnt_root = PathBuf::from("/mnt");
        if mnt_root.exists() {
            if let Ok(drives) = std::fs::read_dir(&mnt_root) {
                for drive in drives.flatten() {
                    let users_dir = drive.path().join("Users");
                    if !users_dir.exists() {
                        continue;
                    }
                    if let Ok(users) = std::fs::read_dir(users_dir) {
                        for user in users.flatten() {
                            let home = user.path();
                            if !homes.iter().any(|item| item == &home) {
                                homes.push(home);
                            }
                        }
                    }
                }
            }
        }
    }

    homes
}

fn compatible_db_paths() -> Vec<PathBuf> {
    let compat_dir = [".cc", "switch"].join("-");
    let compat_db = ["cc", "switch.db"].join("-");

    candidate_home_dirs()
        .into_iter()
        .map(|home| home.join(&compat_dir).join(&compat_db))
        .filter(|path| path.exists())
        .collect()
}

fn current_profile_setting_key(tool_id: &str) -> String {
    format!("current_config_profile:{}", tool_id)
}

fn next_profile_sort_order(conn: &rusqlite::Connection, tool_id: &str) -> i64 {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM config_profiles WHERE tool_id = ?1",
        rusqlite::params![tool_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub(crate) fn ensure_official_config_profiles_seeded(
    conn: &rusqlite::Connection,
) -> Result<(), String> {
    let seeded_at = chrono::Utc::now().to_rfc3339();
    let codex_config = [
        r#"model_provider = "custom""#,
        r#"model = "gpt-5.4""#,
        r#"model_reasoning_effort = "high""#,
        "disable_response_storage = true",
        "",
        "[model_providers.custom]",
        r#"name = "custom""#,
        r#"base_url = "https://api.openai.com/v1""#,
        r#"wire_api = "responses""#,
        "requires_openai_auth = true",
    ]
    .join("\n");

    let seeds = vec![
        (
            "claude",
            "Claude Official",
            serde_json::json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
                },
                "includeCoAuthoredBy": false,
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://www.anthropic.com/api",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "codex",
            "OpenAI Official",
            serde_json::json!({
                "auth": {},
                "config": codex_config,
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://platform.openai.com/",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "gemini",
            "Google Official",
            serde_json::json!({
                "env": {
                    "GOOGLE_GEMINI_BASE_URL": "https://generativelanguage.googleapis.com/v1beta",
                    "GEMINI_MODEL": "gemini-2.5-pro",
                },
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://ai.google.dev/",
                    "seededAt": seeded_at,
                },
                "config": {},
            })
            .to_string(),
        ),
        (
            "openclaw",
            "Anthropic Direct",
            serde_json::json!({
                "baseUrl": "https://api.anthropic.com",
                "apiKey": "",
                "api": "anthropic-messages",
                "models": [{
                    "id": "claude-sonnet-4-5",
                    "name": "claude-sonnet-4-5",
                }],
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://www.anthropic.com/api",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "opencode",
            "OpenAI Responses",
            serde_json::json!({
                "npm": "@ai-sdk/openai",
                "name": "custom",
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://platform.openai.com/",
                    "seededAt": seeded_at,
                },
                "options": {
                    "baseURL": "https://api.openai.com/v1",
                    "apiKey": "",
                },
                "models": {
                    "gpt-5.4": {
                        "name": "gpt-5.4",
                    },
                },
            })
            .to_string(),
        ),
    ];

    for (tool_id, name, config_snapshot) in seeds {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM config_profiles WHERE tool_id = ?1 AND name = ?2",
                rusqlite::params![tool_id, name],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let next_sort_order = next_profile_sort_order(conn, tool_id);
        conn.execute(
            "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'manual', NULL, ?6, ?6)",
            rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn clear_active_profile_if_selected(
    conn: &rusqlite::Connection,
    tool_id: &str,
    profile_id: &str,
) -> Result<(), String> {
    let setting_key = current_profile_setting_key(tool_id);
    let stored_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![&setting_key],
            |row| row.get(0),
        )
        .ok();
    if stored_id.as_deref() == Some(profile_id) {
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn apply_snapshot_if_profile_active(
    conn: &rusqlite::Connection,
    profile_id: &str,
    tool_id: &str,
    config_snapshot: &str,
) -> Result<(), String> {
    let setting_key = current_profile_setting_key(tool_id);
    let active_profile_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
            |row| row.get(0),
        )
        .ok();

    if active_profile_id.as_deref() == Some(profile_id) {
        apply_tool_snapshot(conn, tool_id, config_snapshot)?;
    }

    Ok(())
}

fn delete_profile_record(
    conn: &rusqlite::Connection,
    profile_id: &str,
    tool_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM config_profiles WHERE id = ?1",
        rusqlite::params![profile_id],
    )
    .map_err(|e| e.to_string())?;
    clear_active_profile_if_selected(conn, tool_id, profile_id)?;
    Ok(())
}

fn get_stored_current_profile_ids(
    conn: &rusqlite::Connection,
) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM app_settings WHERE key LIKE 'current_config_profile:%'")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut current = HashMap::new();
    for row in rows {
        let (key, value) = row.map_err(|e| e.to_string())?;
        if let Some(tool_id) = key.strip_prefix("current_config_profile:") {
            current.insert(tool_id.to_string(), value);
        }
    }

    Ok(current)
}

fn normalize_provider_fragment_target_tools(target_tools: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    target_tools
        .into_iter()
        .map(|tool_id| tool_id.trim().to_string())
        .filter(|tool_id| !tool_id.is_empty())
        .filter(|tool_id| MANAGED_APP_IDS.contains(&tool_id.as_str()))
        .filter(|tool_id| seen.insert(tool_id.clone()))
        .collect()
}

fn read_provider_config_fragments_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<ProviderConfigFragment>, String> {
    let mut fragments = get_json_app_setting::<Vec<ProviderConfigFragment>>(
        conn,
        PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY,
    )?
    .unwrap_or_default();

    for fragment in &mut fragments {
        fragment.name = fragment.name.trim().to_string();
        fragment.target_tools =
            normalize_provider_fragment_target_tools(fragment.target_tools.clone());
    }

    fragments.retain(|fragment| {
        !fragment.id.trim().is_empty()
            && !fragment.name.is_empty()
            && !fragment.target_tools.is_empty()
            && fragment.fields.is_object()
    });

    fragments.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(fragments)
}

fn get_compatible_current_profile_ids() -> Result<HashMap<String, String>, String> {
    let mut current = HashMap::new();

    for db_path in compatible_db_paths() {
        let external = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;

        let mut stmt = external
            .prepare(
                "SELECT id, app_type
                 FROM providers
                 WHERE is_current = 1 AND app_type IN ('claude', 'codex', 'gemini')",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (provider_id, tool_id) = row.map_err(|e| e.to_string())?;
            current.insert(
                tool_id.clone(),
                format!("compat-{}-{}", tool_id, provider_id),
            );
        }
    }

    Ok(current)
}

fn read_all_config_profiles_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<ConfigProfile>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, tool_id, config_snapshot, COALESCE(sort_order, 0), source_type, source_key, created_at, updated_at
             FROM config_profiles
             ORDER BY tool_id ASC, COALESCE(sort_order, 0) ASC, updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let profiles = stmt
        .query_map([], |row| {
            Ok(ConfigProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                tool_id: row.get(2)?,
                config_snapshot: row.get(3)?,
                sort_order: row.get(4)?,
                source_type: row.get(5)?,
                source_key: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    Ok(profiles)
}

fn get_active_config_profile_ids_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<String>, String> {
    let profiles = read_all_config_profiles_from_conn(conn)?;
    let mut active_ids = Vec::new();
    let stored_current = get_stored_current_profile_ids(conn)?;
    let compatible_current = get_compatible_current_profile_ids().unwrap_or_default();
    let mut cache: HashMap<String, Option<String>> = HashMap::new();
    let mut resolved_tools = std::collections::HashSet::new();

    for profile in &profiles {
        if resolved_tools.contains(&profile.tool_id) {
            continue;
        }

        let preferred_id = stored_current
            .get(&profile.tool_id)
            .or_else(|| compatible_current.get(&profile.tool_id));

        if let Some(preferred_id) = preferred_id {
            if profiles
                .iter()
                .any(|item| item.tool_id == profile.tool_id && item.id == *preferred_id)
            {
                active_ids.push(preferred_id.clone());
                resolved_tools.insert(profile.tool_id.clone());
            }
        }
    }

    for profile in profiles {
        if resolved_tools.contains(&profile.tool_id) {
            continue;
        }

        if !cache.contains_key(&profile.tool_id) {
            let content = read_tool_snapshot(conn, &profile.tool_id).ok();
            cache.insert(profile.tool_id.clone(), content);
        }

        if cache
            .get(&profile.tool_id)
            .and_then(|value| value.as_ref())
            .is_some_and(|value| config_contents_match(value, &profile.config_snapshot))
        {
            active_ids.push(profile.id);
            resolved_tools.insert(profile.tool_id.clone());
        }
    }

    Ok(active_ids)
}

pub fn read_config_profiles_for_tray(
    conn: &rusqlite::Connection,
) -> Result<Vec<ConfigProfile>, String> {
    read_all_config_profiles_from_conn(conn)
}

pub fn read_active_config_profile_ids_for_tray(
    conn: &rusqlite::Connection,
) -> Result<Vec<String>, String> {
    get_active_config_profile_ids_from_conn(conn)
}

fn normalize_external_profile_snapshot(tool_id: &str, settings_config: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(settings_config).ok()?;

    match tool_id {
        "claude" | "codex" | "gemini" => serde_json::to_string_pretty(&value).ok(),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn upsert_synced_profile(
    conn: &rusqlite::Connection,
    id: &str,
    name: &str,
    tool_id: &str,
    config_snapshot: &str,
    source_type: &str,
    source_key: Option<&str>,
    now: &str,
) -> Result<(), String> {
    let existing_source_type: Option<String> = conn
        .query_row(
            "SELECT source_type FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .ok();

    if existing_source_type.as_deref() == Some("manual") {
        return Ok(());
    }

    if existing_source_type.is_some() {
        conn.execute(
            "UPDATE config_profiles
             SET name = ?1, tool_id = ?2, config_snapshot = ?3, source_type = ?4, source_key = ?5, updated_at = ?6
             WHERE id = ?7",
            rusqlite::params![name, tool_id, config_snapshot, source_type, source_key, now, id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let next_sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM config_profiles WHERE tool_id = ?1",
                rusqlite::params![tool_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO config_profiles
             (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, source_type, source_key, now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub(super) fn sync_profiles_from_compatible_databases(
    conn: &rusqlite::Connection,
    now: &str,
) -> Result<HashMap<String, usize>, String> {
    let mut counts = HashMap::new();
    let mut seen_ids = std::collections::HashSet::new();

    for db_path in compatible_db_paths() {
        let external = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;

        let mut stmt = external
            .prepare(
                "SELECT id, app_type, name, settings_config
                 FROM providers
                 WHERE app_type IN ('claude', 'codex', 'gemini')
                 ORDER BY app_type, name",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (provider_id, tool_id, name, settings_config) = row.map_err(|e| e.to_string())?;
            let Some(config_snapshot) =
                normalize_external_profile_snapshot(&tool_id, &settings_config)
            else {
                continue;
            };
            let id = format!("compat-{}-{}", tool_id, provider_id);
            let source_key = format!("{}#{}", db_path.display(), provider_id);

            upsert_synced_profile(
                conn,
                &id,
                &name,
                &tool_id,
                &config_snapshot,
                "compatible",
                Some(&source_key),
                now,
            )?;

            *counts.entry(tool_id).or_insert(0) += 1;
            seen_ids.insert(id);
        }
    }

    let mut stale_stmt = conn
        .prepare("SELECT id FROM config_profiles WHERE source_type = 'compatible'")
        .map_err(|e| e.to_string())?;
    let stale_ids: Vec<String> = stale_stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .filter(|id: &String| !seen_ids.contains(id))
        .collect();

    for id in stale_ids {
        conn.execute(
            "DELETE FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(counts)
}

pub(super) fn sync_live_profiles(
    conn: &rusqlite::Connection,
    imported_counts: &HashMap<String, usize>,
    now: &str,
) -> Result<(), String> {
    for tool_id in [
        "claude", "codex", "gemini", "opencode", "openclaw", "hermes",
    ] {
        let id = format!("live-{}", tool_id);

        if imported_counts.get(tool_id).copied().unwrap_or(0) > 0 {
            conn.execute(
                "DELETE FROM config_profiles WHERE id = ?1",
                rusqlite::params![id],
            )
            .map_err(|e| e.to_string())?;
            continue;
        }

        match read_tool_snapshot(conn, tool_id) {
            Ok(config_snapshot) => {
                let name = format!("{} 当前配置", tool_id);
                upsert_synced_profile(
                    conn,
                    &id,
                    &name,
                    tool_id,
                    &config_snapshot,
                    "live",
                    Some(tool_id),
                    now,
                )?;
            }
            Err(_) => {
                conn.execute(
                    "DELETE FROM config_profiles WHERE id = ?1",
                    rusqlite::params![id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

fn config_contents_match(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }

    match (
        serde_json::from_str::<serde_json::Value>(left),
        serde_json::from_str::<serde_json::Value>(right),
    ) {
        (Ok(mut a), Ok(mut b)) => {
            // Strip metadata keys used for claude profile splitting
            for key in &["__claude_json_keys__", "__settings_json_keys__"] {
                a.as_object_mut().map(|o| o.remove(*key));
                b.as_object_mut().map(|o| o.remove(*key));
            }
            a == b
        }
        _ => left.trim() == right.trim(),
    }
}

pub(super) fn read_tool_snapshot(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<String, String> {
    match tool_id {
        "codex" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            let auth_path = dir.join("auth.json");
            if !auth_path.exists() {
                return Err(format!("Config file not found: {}", auth_path.display()));
            }
            let auth: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&auth_path).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let config_path = dir.join("config.toml");
            let config = if config_path.exists() {
                std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?
            } else {
                String::new()
            };
            serde_json::to_string_pretty(&serde_json::json!({
                "auth": auth,
                "config": config,
            }))
            .map_err(|e| e.to_string())
        }
        "gemini" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            let env_path = dir.join(".env");
            if !env_path.exists() {
                return Err(format!("Config file not found: {}", env_path.display()));
            }
            let env_text = std::fs::read_to_string(&env_path).map_err(|e| e.to_string())?;
            let env: HashMap<String, String> = env_text
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .filter_map(|l| {
                    l.split_once('=')
                        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                })
                .collect();
            let settings_path = dir.join("settings.json");
            let config = if settings_path.exists() {
                serde_json::from_str::<serde_json::Value>(
                    &std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?
            } else {
                serde_json::json!({})
            };
            serde_json::to_string_pretty(&serde_json::json!({
                "env": env,
                "config": config,
            }))
            .map_err(|e| e.to_string())
        }
        "hermes" => hermes::snapshot::read_snapshot(conn),
        "claude" => {
            let (claude_json, settings_json) = resolve_claude_paths(conn)?;

            let claude_json_obj: serde_json::Map<String, serde_json::Value> =
                if claude_json.exists() {
                    std::fs::read_to_string(&claude_json)
                        .ok()
                        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                        .and_then(|v| v.as_object().cloned())
                        .unwrap_or_default()
                } else {
                    serde_json::Map::new()
                };

            let settings_json_obj: serde_json::Map<String, serde_json::Value> =
                if settings_json.exists() {
                    std::fs::read_to_string(&settings_json)
                        .ok()
                        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                        .and_then(|v| v.as_object().cloned())
                        .unwrap_or_default()
                } else {
                    serde_json::Map::new()
                };

            if claude_json_obj.is_empty() && settings_json_obj.is_empty() {
                return Err("No Claude config found".to_string());
            }

            // Store both sources separately so apply can split them back
            let claude_json_keys: Vec<String> = claude_json_obj.keys().cloned().collect();
            let settings_json_keys: Vec<String> = settings_json_obj.keys().cloned().collect();

            let mut combined = claude_json_obj;
            for (k, v) in settings_json_obj {
                if !combined.contains_key(&k) {
                    combined.insert(k, v);
                }
            }
            combined.insert(
                "__claude_json_keys__".to_string(),
                serde_json::json!(claude_json_keys),
            );
            combined.insert(
                "__settings_json_keys__".to_string(),
                serde_json::json!(settings_json_keys),
            );

            serde_json::to_string_pretty(&serde_json::Value::Object(combined))
                .map_err(|e| e.to_string())
        }
        _ => {
            let config_path = resolve_tool_config_path(conn, tool_id)?;
            if !config_path.exists() {
                return Err(format!("Config file not found: {}", config_path.display()));
            }
            std::fs::read_to_string(&config_path).map_err(|e| e.to_string())
        }
    }
}

pub(super) fn apply_tool_snapshot(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
) -> Result<(), String> {
    apply_tool_snapshot_with_options(conn, tool_id, snapshot, false)
}

/// Codex TOML keys that are managed by the Tools page / ConfigFiles page and should survive
/// startup reapply of the active profile. On explicit profile switch we intentionally overwrite
/// them, but on unattended startup reapply (whose only real job is to rewrite the proxy base_url)
/// we want the user's last-known values to persist across restarts.
const CODEX_USER_MANAGED_KEYS: &[&str] = &[
    "personality",
    "model_reasoning_effort",
    "disable_response_storage",
    "model_context_window",
    "model_auto_compact_token_limit",
];

/// Overlay the Tools-page-managed codex fields from the existing config.toml on disk onto
/// the snapshot's inline config TOML. Returns the updated snapshot JSON string.
fn overlay_codex_user_fields_into_snapshot(
    snapshot_json: &str,
    existing_config_path: &std::path::Path,
) -> String {
    if !existing_config_path.exists() {
        return snapshot_json.to_string();
    }
    let Ok(existing_text) = std::fs::read_to_string(existing_config_path) else {
        return snapshot_json.to_string();
    };
    let Ok(existing_doc) = existing_text.parse::<toml_edit::DocumentMut>() else {
        return snapshot_json.to_string();
    };
    let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(snapshot_json) else {
        return snapshot_json.to_string();
    };
    let Some(obj) = parsed.as_object_mut() else {
        return snapshot_json.to_string();
    };
    let Some(config_text) = obj
        .get("config")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    else {
        return snapshot_json.to_string();
    };
    let Ok(mut snapshot_doc) = config_text.parse::<toml_edit::DocumentMut>() else {
        return snapshot_json.to_string();
    };

    for key in CODEX_USER_MANAGED_KEYS {
        if let Some(existing_value) = existing_doc.get(key) {
            snapshot_doc[*key] = existing_value.clone();
        }
    }

    obj.insert(
        "config".to_string(),
        serde_json::Value::String(snapshot_doc.to_string()),
    );
    serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| snapshot_json.to_string())
}

pub(super) fn apply_tool_snapshot_with_options(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
    preserve_user_edits: bool,
) -> Result<(), String> {
    let effective_snapshot =
        crate::provider_proxy::materialize_tool_snapshot_for_runtime(conn, tool_id, snapshot)?;

    match tool_id {
        "codex" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let auth_path = dir.join("auth.json");
            let config_path = dir.join("config.toml");

            let snapshot_to_apply = if preserve_user_edits {
                overlay_codex_user_fields_into_snapshot(&effective_snapshot, &config_path)
            } else {
                effective_snapshot.clone()
            };

            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&snapshot_to_apply) {
                if let (Some(auth), Some(config)) = (
                    value.get("auth"),
                    value.get("config").and_then(|v| v.as_str()),
                ) {
                    let auth_text =
                        serde_json::to_string_pretty(auth).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&auth_path, &auth_text)
                        .map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&config_path, config)
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }

            crate::utils::atomic_write_string(&config_path, &snapshot_to_apply)
                .map_err(|e| e.to_string())
        }
        "gemini" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let env_path = dir.join(".env");
            let settings_path = dir.join("settings.json");

            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&effective_snapshot) {
                if let (Some(env), Some(config)) = (
                    value.get("env").and_then(|v| v.as_object()),
                    value.get("config"),
                ) {
                    let env_map: std::collections::HashMap<String, String> = env
                        .iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|v| (key.clone(), v.to_string()))
                        })
                        .collect();
                    let env_text = env_map
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let config_text =
                        serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&env_path, &env_text)
                        .map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&settings_path, &config_text)
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }

            crate::utils::atomic_write_string(&settings_path, &effective_snapshot)
                .map_err(|e| e.to_string())
        }
        "claude" => {
            let (claude_json_path, settings_json_path) = resolve_claude_paths(conn)?;

            let snap: serde_json::Value =
                serde_json::from_str(&effective_snapshot).map_err(|e| e.to_string())?;
            let snap_obj = snap.as_object().ok_or("Invalid claude snapshot")?;

            // Determine which keys belong to which file
            let claude_json_keys: std::collections::HashSet<String> = snap_obj
                .get("__claude_json_keys__")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let settings_json_keys: std::collections::HashSet<String> = snap_obj
                .get("__settings_json_keys__")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            // Keys that should be preserved in settings.json during profile switch
            let preserve_keys: std::collections::HashSet<&str> =
                ["statusLine", "enabledPlugins", "mcpServers"]
                    .iter()
                    .copied()
                    .collect();

            // Split snapshot fields back to their original files
            let mut claude_data = serde_json::Map::new();
            let mut settings_data = serde_json::Map::new();

            for (k, v) in snap_obj {
                if k == "__claude_json_keys__" || k == "__settings_json_keys__" {
                    continue;
                }
                if !claude_json_keys.is_empty() || !settings_json_keys.is_empty() {
                    // We have source metadata — use it
                    if claude_json_keys.contains(k) {
                        claude_data.insert(k.clone(), v.clone());
                    }
                    if settings_json_keys.contains(k) {
                        settings_data.insert(k.clone(), v.clone());
                    }
                    // Key in neither list (shouldn't happen) — try settings
                    if !claude_json_keys.contains(k) && !settings_json_keys.contains(k) {
                        settings_data.insert(k.clone(), v.clone());
                    }
                } else {
                    // Legacy snapshot without metadata — use known-settings heuristic
                    let settings_known = [
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
                    if settings_known.contains(&k.as_str()) {
                        settings_data.insert(k.clone(), v.clone());
                    } else {
                        claude_data.insert(k.clone(), v.clone());
                    }
                }
            }

            // Write .claude.json — merge with existing
            if !claude_data.is_empty() {
                let mut existing: serde_json::Map<String, serde_json::Value> =
                    if claude_json_path.exists() {
                        std::fs::read_to_string(&claude_json_path)
                            .ok()
                            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                            .and_then(|v| v.as_object().cloned())
                            .unwrap_or_default()
                    } else {
                        serde_json::Map::new()
                    };
                for (k, v) in claude_data {
                    existing.insert(k, v);
                }
                let text = serde_json::to_string_pretty(&serde_json::Value::Object(existing))
                    .map_err(|e| e.to_string())?;
                crate::utils::atomic_write_string(&claude_json_path, &text)
                    .map_err(|e| e.to_string())?;
            }

            // Write settings.json — merge, preserving protected keys
            {
                let mut existing: serde_json::Map<String, serde_json::Value> =
                    if settings_json_path.exists() {
                        std::fs::read_to_string(&settings_json_path)
                            .ok()
                            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                            .and_then(|v| v.as_object().cloned())
                            .unwrap_or_default()
                    } else {
                        serde_json::Map::new()
                    };
                for (k, v) in settings_data {
                    if preserve_keys.contains(k.as_str()) {
                        // Don't overwrite preserved keys — keep current value
                        continue;
                    }
                    existing.insert(k, v);
                }
                let settings_parent = settings_json_path
                    .parent()
                    .ok_or_else(|| "Cannot determine settings.json parent directory".to_string())?;
                std::fs::create_dir_all(settings_parent).map_err(|e| e.to_string())?;
                let text = serde_json::to_string_pretty(&serde_json::Value::Object(existing))
                    .map_err(|e| e.to_string())?;
                crate::utils::atomic_write_string(&settings_json_path, &text)
                    .map_err(|e| e.to_string())?;
            }

            Ok(())
        }
        "hermes" => hermes::snapshot::apply_snapshot(conn, &effective_snapshot).map(|_| ()),
        _ => {
            let config_path = resolve_tool_config_path(conn, tool_id)?;
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            crate::utils::atomic_write_string(&config_path, &effective_snapshot)
                .map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub fn sync_config_profiles(db: State<'_, DbState>) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        let imported_counts = sync_profiles_from_compatible_databases(&conn, &now)?;
        sync_live_profiles(&conn, &imported_counts, &now)?;
        Ok(())
    })();
    log_command_timing("sync_config_profiles", started_at);
    result
}

#[tauri::command]
pub fn get_config_profiles(db: State<'_, DbState>) -> Result<Vec<ConfigProfile>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_all_config_profiles_from_conn(&conn)
    })();
    log_command_timing("get_config_profiles", started_at);
    result
}

#[tauri::command]
pub fn get_provider_config_fragments(
    db: State<'_, DbState>,
) -> Result<Vec<ProviderConfigFragment>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_provider_config_fragments_from_conn(&conn)
    })();
    log_command_timing("get_provider_config_fragments", started_at);
    result
}

#[tauri::command]
pub fn save_provider_config_fragment(
    id: Option<String>,
    name: String,
    target_tools: Vec<String>,
    fields: serde_json::Value,
    db: State<'_, DbState>,
) -> Result<ProviderConfigFragment, String> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err("Fragment name is required".to_string());
    }
    if !fields.is_object() {
        return Err("Fragment fields must be a JSON object".to_string());
    }

    let normalized_tools = normalize_provider_fragment_target_tools(target_tools);
    if normalized_tools.is_empty() {
        return Err("At least one target app is required".to_string());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut fragments = read_provider_config_fragments_from_conn(&conn)?;
    let now = chrono::Utc::now().to_rfc3339();
    let next_id = id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let saved = if let Some(existing) = fragments.iter_mut().find(|fragment| fragment.id == next_id)
    {
        existing.name = trimmed_name.to_string();
        existing.target_tools = normalized_tools.clone();
        existing.fields = fields.clone();
        existing.updated_at = now.clone();
        existing.clone()
    } else {
        let fragment = ProviderConfigFragment {
            id: next_id.clone(),
            name: trimmed_name.to_string(),
            target_tools: normalized_tools.clone(),
            fields: fields.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        fragments.push(fragment.clone());
        fragment
    };

    fragments.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    set_json_app_setting(&conn, PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY, &fragments)?;
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!(
            "Saved provider config fragment {} for apps {}",
            saved.id,
            saved.target_tools.join(",")
        ),
    );

    Ok(saved)
}

#[tauri::command]
pub fn delete_provider_config_fragment(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut fragments = read_provider_config_fragments_from_conn(&conn)?;
    let initial_len = fragments.len();
    fragments.retain(|fragment| fragment.id != id);
    if fragments.len() == initial_len {
        return Err("Provider fragment not found".to_string());
    }

    set_json_app_setting(&conn, PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY, &fragments)?;
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!("Deleted provider config fragment {id}"),
    );
    Ok(())
}

#[tauri::command]
pub fn save_config_profile(
    name: String,
    tool_id: String,
    config_snapshot: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let next_sort_order = next_profile_sort_order(&conn, &tool_id);

    conn.execute(
        "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'manual', NULL, ?6, ?6)",
        rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, now],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn save_shared_config_profiles(
    name: String,
    profiles: Vec<SharedConfigProfileInput>,
    group_key: Option<String>,
    replace_profile_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    if profiles.is_empty() {
        return Err("At least one target tool is required".to_string());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let shared_group_key = group_key.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut existing_by_tool: HashMap<String, (String, Option<String>)> = HashMap::new();
    let mut stale_manual_replace: Option<(String, String)> = None;

    {
        let mut stmt = conn
            .prepare(
                "SELECT id, tool_id, source_type
                 FROM config_profiles
                 WHERE source_type = 'shared' AND source_key = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![&shared_group_key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (id, tool_id, source_type) = row.map_err(|e| e.to_string())?;
            existing_by_tool.insert(tool_id, (id, source_type));
        }
    }

    if let Some(profile_id) = replace_profile_id.as_ref() {
        let existing = conn
            .query_row(
                "SELECT tool_id, source_type
                 FROM config_profiles
                 WHERE id = ?1",
                rusqlite::params![profile_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .ok();

        if let Some((tool_id, source_type)) = existing {
            if source_type.as_deref() != Some("shared") && !existing_by_tool.contains_key(&tool_id)
            {
                if profiles.iter().any(|item| item.tool_id == tool_id) {
                    existing_by_tool.insert(tool_id, (profile_id.clone(), source_type));
                } else {
                    stale_manual_replace = Some((tool_id, profile_id.clone()));
                }
            }
        }
    }

    for profile in &profiles {
        if let Some((existing_id, _)) = existing_by_tool.remove(&profile.tool_id) {
            conn.execute(
                "UPDATE config_profiles
                 SET name = ?1, tool_id = ?2, config_snapshot = ?3, source_type = 'shared', source_key = ?4, updated_at = ?5
                 WHERE id = ?6",
                rusqlite::params![
                    &name,
                    &profile.tool_id,
                    &profile.config_snapshot,
                    &shared_group_key,
                    &now,
                    &existing_id
                ],
            )
            .map_err(|e| e.to_string())?;
            apply_snapshot_if_profile_active(
                &conn,
                &existing_id,
                &profile.tool_id,
                &profile.config_snapshot,
            )?;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            let next_sort_order = next_profile_sort_order(&conn, &profile.tool_id);
            conn.execute(
                "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'shared', ?6, ?7, ?7)",
                rusqlite::params![
                    id,
                    &name,
                    &profile.tool_id,
                    &profile.config_snapshot,
                    next_sort_order,
                    &shared_group_key,
                    &now
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    if let Some((tool_id, profile_id)) = stale_manual_replace {
        delete_profile_record(&conn, &profile_id, &tool_id)?;
    }

    for (tool_id, (profile_id, _)) in existing_by_tool {
        delete_profile_record(&conn, &profile_id, &tool_id)?;
    }

    Ok(shared_group_key)
}

#[tauri::command]
pub fn update_config_profile(
    id: String,
    name: String,
    config_snapshot: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let tool_id: String = conn
        .query_row(
            "SELECT tool_id FROM config_profiles WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    conn.execute(
        "UPDATE config_profiles SET name = ?1, config_snapshot = ?2, source_type = 'manual', source_key = NULL, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![name, config_snapshot, now, id],
    )
    .map_err(|e| e.to_string())?;

    let setting_key = current_profile_setting_key(&tool_id);
    let active_profile_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
            |row| row.get(0),
        )
        .ok();

    if active_profile_id.as_deref() == Some(id.as_str()) {
        apply_tool_snapshot(&conn, &tool_id, &config_snapshot)?;
    }

    Ok(())
}

pub fn apply_config_profile_from_conn(
    conn: &rusqlite::Connection,
    id: &str,
    preserve_user_edits: bool,
) -> Result<(String, String), String> {
    let (tool_id, snapshot): (String, String) = conn
        .query_row(
            "SELECT tool_id, config_snapshot FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    apply_tool_snapshot_with_options(conn, &tool_id, &snapshot, preserve_user_edits)?;

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE config_profiles SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![current_profile_setting_key(&tool_id), id],
    )
    .map_err(|e| e.to_string())?;

    crate::db::record_activity(conn, &tool_id, "profile_switch", "success", None);
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!("Applied profile {id} for tool {tool_id}"),
    );
    Ok((tool_id, snapshot))
}

#[tauri::command]
pub fn apply_config_profile(
    id: String,
    db: State<'_, DbState>,
) -> Result<ApplyConfigProfileResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (tool_id, _) = apply_config_profile_from_conn(&conn, &id, false)?;
    let active_profile_ids = get_active_config_profile_ids_from_conn(&conn)?;
    Ok(ApplyConfigProfileResult {
        tool_id,
        profile_id: id,
        active_profile_ids,
        applied_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub fn delete_config_profile(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (tool_id, source_type): (String, Option<String>) = conn
        .query_row(
            "SELECT tool_id, source_type FROM config_profiles WHERE id = ?1",
            rusqlite::params![&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    if source_type.as_deref() != Some("manual") {
        return Err("Only manual profiles can be deleted".to_string());
    }

    delete_profile_record(&conn, &id, &tool_id)?;

    Ok(())
}

#[tauri::command]
pub fn delete_config_profile_group(
    source_key: String,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, tool_id
             FROM config_profiles
             WHERE source_type = 'shared' AND source_key = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![&source_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(row.map_err(|e| e.to_string())?);
    }

    if profiles.is_empty() {
        return Err("Shared profile group not found".to_string());
    }

    for (profile_id, tool_id) in &profiles {
        delete_profile_record(&conn, profile_id, tool_id)?;
    }

    Ok(profiles.len())
}

#[tauri::command]
pub fn reorder_config_profiles(
    tool_id: String,
    ordered_ids: Vec<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    for (index, profile_id) in ordered_ids.iter().enumerate() {
        let belongs_to_tool: Option<String> = conn
            .query_row(
                "SELECT tool_id FROM config_profiles WHERE id = ?1",
                rusqlite::params![profile_id],
                |row| row.get(0),
            )
            .ok();

        if belongs_to_tool.as_deref() != Some(tool_id.as_str()) {
            return Err(format!(
                "Profile does not belong to tool {tool_id}: {profile_id}"
            ));
        }

        conn.execute(
            "UPDATE config_profiles SET sort_order = ?1 WHERE id = ?2",
            rusqlite::params![index as i64, profile_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_active_config_profile_ids(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        get_active_config_profile_ids_from_conn(&conn)
    })();
    log_command_timing("get_active_config_profile_ids", started_at);
    result
}

#[tauri::command]
pub fn refresh_tray_provider_menu(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::refresh_tray_menu(&app_handle).map_err(|e| e.to_string())
}

fn parse_toml_assignment(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(key) {
            return None;
        }
        let (_, raw_value) = trimmed.split_once('=')?;
        let value = raw_value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn parse_toml_section_assignment(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed.trim_matches(['[', ']']) == section;
            continue;
        }
        if !in_section || !trimmed.starts_with(key) {
            continue;
        }
        let (_, raw_value) = trimmed.split_once('=')?;
        let value = raw_value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            return None;
        }
        return Some(value.to_string());
    }
    None
}

pub(super) fn read_codex_structured_config_from_content(
    content: &str,
    api_key: String,
) -> CodexTomlStructuredConfig {
    let model_provider =
        parse_toml_assignment(content, "model_provider").unwrap_or_else(|| "custom".to_string());
    let provider_section = format!("model_providers.{model_provider}");

    let mcp_servers = content
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|doc| {
            doc.get("mcp_servers")
                .and_then(|item| item.as_table())
                .map(|table| {
                    table
                        .iter()
                        .map(|(key, _)| key.to_string())
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or_default();

    let malformed_mcp_servers = content
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|doc| doc.get("mcp_servers").map(|item| !item.is_table()))
        .unwrap_or(false);

    CodexTomlStructuredConfig {
        model_provider: model_provider.clone(),
        provider_label: parse_toml_section_assignment(content, &provider_section, "name")
            .unwrap_or_else(|| model_provider.clone()),
        base_url: parse_toml_section_assignment(content, &provider_section, "base_url")
            .unwrap_or_default(),
        wire_api: parse_toml_section_assignment(content, &provider_section, "wire_api")
            .unwrap_or_else(|| "responses".to_string()),
        model: parse_toml_assignment(content, "model").unwrap_or_default(),
        reasoning_effort: parse_toml_assignment(content, "model_reasoning_effort")
            .unwrap_or_else(|| "medium".to_string()),
        personality: parse_toml_assignment(content, "personality")
            .unwrap_or_else(|| "pragmatic".to_string()),
        disable_response_storage: parse_toml_assignment(content, "disable_response_storage")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        model_context_window: parse_toml_assignment(content, "model_context_window")
            .unwrap_or_default(),
        model_auto_compact_token_limit: parse_toml_assignment(
            content,
            "model_auto_compact_token_limit",
        )
        .unwrap_or_default(),
        api_key,
        mcp_servers,
        malformed_mcp_servers,
    }
}

pub(super) fn write_codex_structured_config_to_text(
    raw_toml: &str,
    config: &CodexTomlStructuredConfig,
) -> String {
    let mut doc = raw_toml
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());

    let provider_name =
        normalized_non_empty(&config.model_provider).unwrap_or_else(|| "custom".to_string());
    let provider_label =
        normalized_non_empty(&config.provider_label).unwrap_or_else(|| provider_name.clone());
    let wire_api =
        normalized_non_empty(&config.wire_api).unwrap_or_else(|| "responses".to_string());
    let reasoning_effort =
        normalized_non_empty(&config.reasoning_effort).unwrap_or_else(|| "medium".to_string());
    let personality =
        normalized_non_empty(&config.personality).unwrap_or_else(|| "pragmatic".to_string());

    doc["model_provider"] = toml_edit::value(provider_name.clone());
    doc["model"] = toml_edit::value(config.model.trim());
    doc["model_reasoning_effort"] = toml_edit::value(reasoning_effort);
    doc["personality"] = toml_edit::value(personality);
    doc["disable_response_storage"] = toml_edit::value(config.disable_response_storage);

    if let Some(context_window) = normalize_integer_like(&config.model_context_window) {
        doc["model_context_window"] = toml_edit::value(context_window);
    } else {
        doc.as_table_mut().remove("model_context_window");
    }

    if let Some(compact_limit) = normalize_integer_like(&config.model_auto_compact_token_limit) {
        doc["model_auto_compact_token_limit"] = toml_edit::value(compact_limit);
    } else {
        doc.as_table_mut().remove("model_auto_compact_token_limit");
    }

    doc["model_providers"][provider_name.as_str()]["name"] = toml_edit::value(provider_label);
    doc["model_providers"][provider_name.as_str()]["base_url"] =
        toml_edit::value(config.base_url.trim());
    doc["model_providers"][provider_name.as_str()]["wire_api"] = toml_edit::value(wire_api);
    doc["model_providers"][provider_name.as_str()]["requires_openai_auth"] = toml_edit::value(true);

    let malformed_mcp_servers = doc
        .get("mcp_servers")
        .map(|item| !item.is_table())
        .unwrap_or(false);
    if malformed_mcp_servers {
        doc.as_table_mut().remove("mcp_servers");
    }
    if doc.get("mcp_servers").is_none() {
        doc["mcp_servers"] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    doc.to_string()
}

fn apply_common_config_to_claude_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Claude snapshot".to_string())?;

    if snippet.hide_attribution {
        obj.insert(
            "attribution".to_string(),
            serde_json::json!({ "commit": "", "pr": "" }),
        );
    }
    if snippet.effort_level_high {
        obj.insert(
            "effortLevel".to_string(),
            serde_json::Value::String("high".to_string()),
        );
    }

    let env = obj
        .entry("env")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Claude env must be an object".to_string())?;
    if snippet.enable_teammates {
        env.insert(
            "CLAUDE_CODE_ENABLE_TEAMMATES".to_string(),
            serde_json::json!("true"),
        );
        env.insert(
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".to_string(),
            serde_json::json!("1"),
        );
    }
    if snippet.enable_tool_search {
        env.insert("ENABLE_TOOL_SEARCH".to_string(), serde_json::json!("true"));
    }
    for (key, value) in &snippet.custom_values {
        env.insert(key.clone(), serde_json::json!(value));
    }

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

fn apply_common_config_to_codex_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Codex snapshot".to_string())?;
    let current_config = obj
        .get("config")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let current_config = current_config.to_string();
    let current_api_key = obj
        .get("auth")
        .and_then(|value| value.get("OPENAI_API_KEY"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let mut structured =
        read_codex_structured_config_from_content(&current_config, current_api_key);
    if snippet.effort_level_high {
        structured.reasoning_effort = "high".to_string();
    }
    for (key, value) in &snippet.custom_values {
        if key == "model_auto_compact_token_limit" {
            structured.model_auto_compact_token_limit = value.clone();
        }
    }
    let mut next_toml = write_codex_structured_config_to_text(&current_config, &structured);
    for (key, value) in &snippet.custom_values {
        if key == "model_auto_compact_token_limit" {
            continue;
        }
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            continue;
        }
        let mut doc = next_toml
            .parse::<toml_edit::DocumentMut>()
            .unwrap_or_else(|_| toml_edit::DocumentMut::new());
        if let Some(integer) = normalize_integer_like(value) {
            doc[normalized_key] = toml_edit::value(integer);
        } else if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
            doc[normalized_key] = toml_edit::value(value.eq_ignore_ascii_case("true"));
        } else {
            doc[normalized_key] = toml_edit::value(value.as_str());
        }
        next_toml = doc.to_string();
    }
    obj.insert("config".to_string(), serde_json::Value::String(next_toml));
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

fn apply_common_config_to_gemini_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Gemini snapshot".to_string())?;
    let env = obj
        .entry("env")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Gemini env must be an object".to_string())?;
    for (key, value) in &snippet.custom_values {
        env.insert(key.clone(), serde_json::json!(value));
    }
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

#[allow(dead_code)]
fn apply_common_config_snippet_to_snapshot(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
) -> Result<String, String> {
    let snippet = read_common_config_snippet_from_conn(conn, tool_id)?;
    if !common_config_snippet_has_payload(&snippet) {
        return Ok(snapshot.to_string());
    }

    match tool_id {
        "claude" => apply_common_config_to_claude_snapshot(snapshot, &snippet),
        "codex" => apply_common_config_to_codex_snapshot(snapshot, &snippet),
        "gemini" => apply_common_config_to_gemini_snapshot(snapshot, &snippet),
        _ => Ok(snapshot.to_string()),
    }
}

fn join_api_endpoint(base_url: &str, suffix: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed_base = base_url.trim().trim_end_matches('/');
    let trimmed_suffix = suffix.trim_start_matches('/');
    if trimmed_base.ends_with(trimmed_suffix) {
        trimmed_base.to_string()
    } else {
        format!("{trimmed_base}/{trimmed_suffix}")
    }
}

fn build_claude_messages_endpoint(base_url: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/messages") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/messages")
    } else {
        format!("{trimmed}/v1/messages")
    }
}

fn build_gemini_stream_endpoint(base_url: &str, model: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.contains(":streamGenerateContent") {
        trimmed.to_string()
    } else if trimmed.ends_with(&format!("/models/{model}")) {
        format!("{trimmed}:streamGenerateContent?alt=sse")
    } else {
        format!("{trimmed}/models/{model}:streamGenerateContent?alt=sse")
    }
}

struct StreamCheckRequestSpec {
    endpoint: String,
    headers: Vec<(String, String)>,
    body: serde_json::Value,
}

fn build_provider_probe_client(conn: &rusqlite::Connection) -> Result<reqwest::Client, String> {
    let proxy_url = get_text_app_setting(conn, "proxy_url")?.unwrap_or_default();
    http_client::build_http_client(
        Some(proxy_url.as_str()),
        Some("CCHub Provider Probe"),
        Duration::from_secs(10),
    )
}

fn extract_profile_metadata(
    parsed: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    parsed
        .get("metadata")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default()
}

fn extract_provider_type_from_snapshot(parsed: &serde_json::Value) -> Option<String> {
    extract_profile_metadata(parsed)
        .get("providerType")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn extract_use_full_url_from_snapshot(parsed: &serde_json::Value) -> bool {
    extract_profile_metadata(parsed)
        .get("useFullUrl")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn extract_copilot_account_id_from_snapshot(parsed: &serde_json::Value) -> Option<String> {
    let metadata = extract_profile_metadata(parsed);
    metadata
        .get("authBinding")
        .and_then(|value| {
            value
                .get("authProvider")
                .and_then(|item| item.as_str())
                .map(|provider| (value, provider))
        })
        .and_then(|(value, provider)| {
            if provider == "github_copilot" {
                value
                    .get("accountId")
                    .and_then(|item| item.as_str())
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
            } else {
                None
            }
        })
        .or_else(|| {
            metadata
                .get("githubAccountId")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn build_openai_chat_endpoint(
    base_url: &str,
    provider_type: Option<&str>,
    use_full_url: bool,
) -> String {
    if provider_type == Some("github_copilot") {
        join_api_endpoint(base_url, "chat/completions", use_full_url)
    } else {
        join_api_endpoint(base_url, "v1/chat/completions", use_full_url)
    }
}

async fn resolve_copilot_headers(
    app_handle: &AppHandle,
    parsed: &serde_json::Value,
) -> Result<Vec<(String, String)>, String> {
    let account_id = extract_copilot_account_id_from_snapshot(parsed);
    let manager = app_handle.state::<CopilotAuthState>().0.clone();
    let token = manager
        .get_valid_token_for_account(account_id.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    Ok(copilot_auth::copilot_request_headers(&token))
}

async fn extract_probe_target(
    app_handle: &AppHandle,
    profile: &ConfigProfile,
) -> Result<(Option<String>, Vec<(String, String)>), String> {
    let parsed: serde_json::Value =
        serde_json::from_str(&profile.config_snapshot).map_err(|e| e.to_string())?;
    let provider_type = extract_provider_type_from_snapshot(&parsed);
    let use_full_url = extract_use_full_url_from_snapshot(&parsed);

    match profile.tool_id.as_str() {
        "claude" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                base_url
            } else if provider_type.as_deref() == Some("github_copilot") {
                base_url.map(|value| join_api_endpoint(&value, "models", false))
            } else {
                base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let headers = if provider_type.as_deref() == Some("github_copilot") {
                resolve_copilot_headers(app_handle, &parsed).await?
            } else {
                let mut headers = Vec::new();
                if let Some(token) = env
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .or_else(|| env.get("ANTHROPIC_API_KEY"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let api_format = env
                        .get("ANTHROPIC_API_FORMAT")
                        .and_then(|value| value.as_str())
                        .unwrap_or("anthropic");
                    if api_format == "anthropic" {
                        headers.push(("x-api-key".to_string(), token.to_string()));
                        headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
                    } else {
                        headers.push(("authorization".to_string(), format!("Bearer {token}")));
                    }
                }
                headers
            };
            Ok((base_url, headers))
        }
        "codex" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let explicit_base_url = parse_toml_assignment(config, "base_url");
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.openai.com/v1".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("auth")
                .and_then(|value| value.get("OPENAI_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
        }
        "gemini" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| {
                    Some("https://generativelanguage.googleapis.com/v1beta".to_string())
                })
            };
            let mut headers = Vec::new();
            if let Some(token) = env
                .get("GEMINI_API_KEY")
                .or_else(|| env.get("GOOGLE_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("x-goog-api-key".to_string(), token.to_string()));
            }
            Ok((base_url, headers))
        }
        "openclaw" => {
            let explicit_base_url = parsed
                .get("baseUrl")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
        }
        "hermes" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let model = config
                .get("model")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let provider = model
                .get("provider")
                .and_then(|value| value.as_str())
                .unwrap_or("custom");
            let explicit_base_url = model
                .get("base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| {
                    hermes::providers::default_base_url_for_provider(provider).map(str::to_string)
                })
            };
            let env_key = parsed
                .get("metadata")
                .and_then(|value| value.get("hermesApiKeyEnv"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    hermes::providers::default_env_key_for_provider(provider).map(str::to_string)
                });
            let mut headers = Vec::new();
            if let Some(token) = env_key
                .as_deref()
                .and_then(|key| env.get(key))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if provider == "gemini" {
                    headers.push(("x-goog-api-key".to_string(), token.to_string()));
                } else if provider == "anthropic" {
                    headers.push(("x-api-key".to_string(), token.to_string()));
                    headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
                } else {
                    headers.push(("authorization".to_string(), format!("Bearer {token}")));
                }
            }
            Ok((base_url, headers))
        }
        "opencode" => {
            let explicit_base_url = parsed
                .get("options")
                .and_then(|value| value.get("baseURL"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("options")
                .and_then(|value| value.get("apiKey"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
        }
        _ => Ok((None, Vec::new())),
    }
}

fn classify_provider_latency_status(latency_ms: u64) -> String {
    if latency_ms < 200 {
        "fast".to_string()
    } else if latency_ms <= 500 {
        "medium".to_string()
    } else {
        "slow".to_string()
    }
}

async fn extract_stream_check_request(
    app_handle: &AppHandle,
    profile: &ConfigProfile,
) -> Result<StreamCheckRequestSpec, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(&profile.config_snapshot).map_err(|e| e.to_string())?;
    let provider_type = extract_provider_type_from_snapshot(&parsed);
    let use_full_url = extract_use_full_url_from_snapshot(&parsed);

    match profile.tool_id.as_str() {
        "claude" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Claude base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string())
            };
            let model = env
                .get("ANTHROPIC_MODEL")
                .or_else(|| env.get("ANTHROPIC_DEFAULT_SONNET_MODEL"))
                .or_else(|| env.get("ANTHROPIC_REASONING_MODEL"))
                .or_else(|| env.get("ANTHROPIC_DEFAULT_HAIKU_MODEL"))
                .or_else(|| env.get("ANTHROPIC_DEFAULT_OPUS_MODEL"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("claude-sonnet-4-5");
            let api_format = env
                .get("ANTHROPIC_API_FORMAT")
                .and_then(|value| value.as_str())
                .unwrap_or("anthropic");

            if provider_type.as_deref() == Some("github_copilot") || api_format == "openai_chat" {
                let headers = if provider_type.as_deref() == Some("github_copilot") {
                    resolve_copilot_headers(app_handle, &parsed).await?
                } else {
                    let token = env
                        .get("ANTHROPIC_AUTH_TOKEN")
                        .or_else(|| env.get("ANTHROPIC_API_KEY"))
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| "No Claude API token configured".to_string())?;
                    vec![("authorization".to_string(), format!("Bearer {token}"))]
                };
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_openai_chat_endpoint(
                        &base_url,
                        provider_type.as_deref(),
                        use_full_url,
                    ),
                    headers,
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                });
            }

            if api_format == "openai_responses" {
                let token = env
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .or_else(|| env.get("ANTHROPIC_API_KEY"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "No Claude API token configured".to_string())?;
                return Ok(StreamCheckRequestSpec {
                    endpoint: join_api_endpoint(&base_url, "v1/responses", use_full_url),
                    headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                });
            }

            let token = env
                .get("ANTHROPIC_AUTH_TOKEN")
                .or_else(|| env.get("ANTHROPIC_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Claude API token configured".to_string())?;

            Ok(StreamCheckRequestSpec {
                endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                headers: vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ],
                body: serde_json::json!({
                    "model": model,
                    "max_tokens": 16,
                    "stream": true,
                    "messages": [
                        { "role": "user", "content": "Reply with OK." }
                    ],
                }),
            })
        }
        "codex" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let token = parsed
                .get("auth")
                .and_then(|value| value.get("OPENAI_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Codex OPENAI_API_KEY configured".to_string())?;
            let explicit_base_url = parse_toml_assignment(config, "base_url");
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Codex base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            };
            let wire_api = parse_toml_assignment(config, "wire_api")
                .unwrap_or_else(|| "responses".to_string());
            let model =
                parse_toml_assignment(config, "model").unwrap_or_else(|| "gpt-5.4".to_string());
            let (endpoint, body) = if wire_api == "chat" {
                (
                    join_api_endpoint(&base_url, "chat/completions", use_full_url),
                    serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                )
            } else {
                (
                    join_api_endpoint(&base_url, "responses", use_full_url),
                    serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                )
            };

            Ok(StreamCheckRequestSpec {
                endpoint,
                headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                body,
            })
        }
        "gemini" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let token = env
                .get("GEMINI_API_KEY")
                .or_else(|| env.get("GOOGLE_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Gemini API key configured".to_string())?;
            let explicit_base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Gemini base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| {
                    "https://generativelanguage.googleapis.com/v1beta".to_string()
                })
            };
            let model = env
                .get("GEMINI_MODEL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gemini-2.5-flash");

            Ok(StreamCheckRequestSpec {
                endpoint: build_gemini_stream_endpoint(&base_url, model, use_full_url),
                headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                body: serde_json::json!({
                    "contents": [
                        {
                            "role": "user",
                            "parts": [{ "text": "Reply with OK." }]
                        }
                    ],
                    "generationConfig": {
                        "maxOutputTokens": 16
                    }
                }),
            })
        }
        "openclaw" => {
            let api = parsed
                .get("api")
                .and_then(|value| value.as_str())
                .unwrap_or("openai-completions");
            let explicit_base_url = parsed
                .get("baseUrl")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No OpenClaw baseUrl configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string())
            };
            let api_key = parsed
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let model = parsed
                .get("models")
                .and_then(|value| value.as_array())
                .and_then(|models| models.first())
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gpt-5.4");

            match api {
                "openai-responses" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: join_api_endpoint(&base_url, "responses", use_full_url),
                        headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                        body: serde_json::json!({
                            "model": model,
                            "stream": true,
                            "max_output_tokens": 16,
                            "input": "Reply with OK.",
                        }),
                    })
                }
                "anthropic-messages" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                        headers: vec![
                            ("x-api-key".to_string(), token),
                            ("anthropic-version".to_string(), "2023-06-01".to_string()),
                        ],
                        body: serde_json::json!({
                            "model": model,
                            "max_tokens": 16,
                            "stream": true,
                            "messages": [
                                { "role": "user", "content": "Reply with OK." }
                            ],
                        }),
                    })
                }
                "google-generative-ai" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: build_gemini_stream_endpoint(&base_url, model, use_full_url),
                        headers: vec![("x-goog-api-key".to_string(), token)],
                        body: serde_json::json!({
                            "contents": [
                                {
                                    "role": "user",
                                    "parts": [{ "text": "Reply with OK." }]
                                }
                            ],
                            "generationConfig": {
                                "maxOutputTokens": 16
                            }
                        }),
                    })
                }
                "bedrock-converse-stream" => Err("AWS Bedrock ConverseStream requires SigV4 signing and is not yet supported for stream checks".to_string()),
                _ => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                        headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                        body: serde_json::json!({
                            "model": model,
                            "stream": true,
                            "max_tokens": 16,
                            "messages": [
                                { "role": "user", "content": "Reply with OK." }
                            ],
                        }),
                    })
                }
            }
        }
        "hermes" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let model = config
                .get("model")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let provider = model
                .get("provider")
                .and_then(|value| value.as_str())
                .unwrap_or("custom");
            let explicit_base_url = model
                .get("base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Hermes base_url configured".to_string())?
            } else {
                explicit_base_url
                    .or_else(|| {
                        hermes::providers::default_base_url_for_provider(provider)
                            .map(str::to_string)
                    })
                    .ok_or_else(|| "No Hermes base_url configured".to_string())?
            };
            let model_id = model
                .get("default")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gpt-5.4");
            let env_key = parsed
                .get("metadata")
                .and_then(|value| value.get("hermesApiKeyEnv"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    hermes::providers::default_env_key_for_provider(provider).map(str::to_string)
                })
                .ok_or_else(|| "No Hermes API key env configured".to_string())?;
            let token = env
                .get(&env_key)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("No Hermes API key configured in {env_key}"))?;

            if provider == "gemini" {
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_gemini_stream_endpoint(&base_url, model_id, use_full_url),
                    headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                    body: serde_json::json!({
                        "contents": [{ "role": "user", "parts": [{ "text": "Reply with OK." }] }],
                        "generationConfig": { "maxOutputTokens": 16 },
                    }),
                });
            }

            if provider == "anthropic" {
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                    headers: vec![
                        ("x-api-key".to_string(), token.to_string()),
                        ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ],
                    body: serde_json::json!({
                        "model": model_id,
                        "max_tokens": 16,
                        "stream": true,
                        "messages": [{ "role": "user", "content": "Reply with OK." }],
                    }),
                });
            }

            Ok(StreamCheckRequestSpec {
                endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                body: serde_json::json!({
                    "model": model_id,
                    "stream": true,
                    "max_tokens": 16,
                    "messages": [{ "role": "user", "content": "Reply with OK." }],
                }),
            })
        }
        "opencode" => {
            let npm = parsed
                .get("npm")
                .and_then(|value| value.as_str())
                .unwrap_or("@ai-sdk/openai-compatible");
            let options = parsed
                .get("options")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_base_url = options
                .get("baseURL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No OpenCode baseURL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            };
            let token = options
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No OpenCode API key configured".to_string())?;
            let model = parsed
                .get("models")
                .and_then(|value| value.as_object())
                .and_then(|value| value.keys().next().cloned())
                .unwrap_or_else(|| "gpt-5.4".to_string());

            if npm.contains("anthropic") {
                Ok(StreamCheckRequestSpec {
                    endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                    headers: vec![
                        ("x-api-key".to_string(), token.to_string()),
                        ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ],
                    body: serde_json::json!({
                        "model": model,
                        "max_tokens": 16,
                        "stream": true,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                })
            } else if npm.contains("google") {
                Ok(StreamCheckRequestSpec {
                    endpoint: build_gemini_stream_endpoint(&base_url, &model, use_full_url),
                    headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                    body: serde_json::json!({
                        "contents": [
                            {
                                "role": "user",
                                "parts": [{ "text": "Reply with OK." }]
                            }
                        ],
                        "generationConfig": {
                            "maxOutputTokens": 16
                        }
                    }),
                })
            } else if npm == "@ai-sdk/openai" {
                Ok(StreamCheckRequestSpec {
                    endpoint: join_api_endpoint(&base_url, "responses", use_full_url),
                    headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                })
            } else {
                Ok(StreamCheckRequestSpec {
                    endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                    headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                })
            }
        }
        _ => Err("Stream check is not supported for this profile".to_string()),
    }
}

#[tauri::command]
pub async fn ping_provider_endpoint(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderPingResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let (base_url, headers) = match extract_probe_target(&app_handle, &profile).await {
        Ok(value) => value,
        Err(message) => {
            let result = ProviderPingResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: "error".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
            log_provider_result(
                "ping",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };

    let Some(base_url) = base_url else {
        let result = ProviderPingResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: None,
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: "No base URL configured for latency ping".to_string(),
        };
        log_provider_result(
            "ping",
            &result.tool_id,
            &result.provider_name,
            result.base_url.as_deref(),
            &result.status,
            &result.message,
        );
        return Ok(result);
    };

    let send_request = |method: reqwest::Method| {
        let client = client.clone();
        let base_url = base_url.clone();
        let headers = headers.clone();
        async move {
            let started_at = std::time::Instant::now();
            let mut request = client.request(method, &base_url);
            for (name, value) in headers {
                request = request.header(&name, value);
            }
            request
                .send()
                .await
                .map(|response| (response, started_at.elapsed().as_millis() as u64))
        }
    };

    let mut response_result = send_request(reqwest::Method::HEAD).await;
    let should_fallback_to_get = matches!(
        response_result,
        Ok((ref response, _))
            if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
                || response.status() == reqwest::StatusCode::NOT_IMPLEMENTED
    );
    if should_fallback_to_get {
        response_result = send_request(reqwest::Method::GET).await;
    }

    let result = match response_result {
        Ok((response, latency_ms)) => {
            let http_status = response.status().as_u16();
            ProviderPingResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: Some(base_url),
                status: classify_provider_latency_status(latency_ms),
                latency_ms: Some(latency_ms),
                http_status: Some(http_status),
                checked_at,
                message: format!("Endpoint responded with HTTP {http_status}"),
            }
        }
        Err(error) => ProviderPingResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: Some(base_url),
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: error.to_string(),
        },
    };

    log_provider_result(
        "ping",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
}

#[tauri::command]
pub async fn probe_config_profile(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderProbeResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let (base_url, headers) = match extract_probe_target(&app_handle, &profile).await {
        Ok(value) => value,
        Err(message) => {
            let result = ProviderProbeResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: "unconfigured".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
            log_provider_result(
                "probe",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };

    let result = if let Some(base_url) = base_url {
        let started_at = std::time::Instant::now();
        let mut request = client.get(&base_url);
        for (name, value) in headers {
            request = request.header(&name, value);
        }

        match request.send().await {
            Ok(response) => {
                let latency_ms = started_at.elapsed().as_millis() as u64;
                let http_status = response.status().as_u16();
                let status = if response.status().is_success() {
                    "healthy"
                } else if response.status().is_client_error() || response.status().is_server_error()
                {
                    "reachable"
                } else {
                    "unknown"
                };

                ProviderProbeResult {
                    profile_id: profile.id,
                    tool_id: profile.tool_id,
                    provider_name: profile.name,
                    base_url: Some(base_url),
                    status: status.to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message: format!("Endpoint responded with HTTP {http_status}"),
                }
            }
            Err(error) => ProviderProbeResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: Some(base_url),
                status: "error".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message: error.to_string(),
            },
        }
    } else {
        ProviderProbeResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: None,
            status: "unconfigured".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: "No base URL configured for probing".to_string(),
        }
    };

    log_provider_result(
        "probe",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
}

#[tauri::command]
pub async fn stream_check_config_profile(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderStreamCheckResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let request = match extract_stream_check_request(&app_handle, &profile).await {
        Ok(request) => request,
        Err(message) => {
            let status =
                if message.contains("not yet supported") || message.contains("not supported") {
                    "unsupported"
                } else {
                    "unconfigured"
                };
            let result = ProviderStreamCheckResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: status.to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
            log_provider_result(
                "stream-check",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };
    let StreamCheckRequestSpec {
        endpoint,
        headers,
        body,
    } = request;

    let started_at = std::time::Instant::now();
    let mut request_builder = client
        .post(&endpoint)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream, application/json");
    for (name, value) in headers {
        request_builder = request_builder.header(&name, value);
    }

    let result = match request_builder.json(&body).send().await {
        Ok(mut response) => {
            let latency_ms = started_at.elapsed().as_millis() as u64;
            let http_status = response.status().as_u16();

            if !response.status().is_success() {
                let detail = response.text().await.unwrap_or_default();
                ProviderStreamCheckResult {
                    profile_id: profile.id,
                    tool_id: profile.tool_id,
                    provider_name: profile.name,
                    base_url: Some(endpoint.clone()),
                    status: "reachable".to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message: if detail.trim().is_empty() {
                        format!("Endpoint responded with HTTP {http_status}")
                    } else {
                        format!(
                            "HTTP {http_status}: {}",
                            detail.chars().take(160).collect::<String>()
                        )
                    },
                }
            } else {
                match tokio::time::timeout(std::time::Duration::from_secs(15), response.chunk()).await {
                    Ok(Ok(Some(chunk))) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "healthy".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: format!("Received first stream chunk ({} bytes)", chunk.len()),
                    },
                    Ok(Ok(None)) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "reachable".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: "Stream endpoint closed without returning chunks".to_string(),
                    },
                    Ok(Err(error)) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "error".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: error.to_string(),
                    },
                    Err(_) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "reachable".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: "Connected successfully but did not receive a stream chunk within 15 seconds".to_string(),
                    },
                }
            }
        }
        Err(error) => ProviderStreamCheckResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: Some(endpoint),
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: error.to_string(),
        },
    };

    log_provider_result(
        "stream-check",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
}
