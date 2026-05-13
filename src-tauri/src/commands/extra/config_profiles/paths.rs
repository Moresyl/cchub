#![allow(clippy::too_many_arguments)]
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::hermes;
use crate::utils::configure_background_command;

use super::super::statusline::*;

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

pub fn tool_config_file_name(tool_id: &str) -> Result<&'static str, String> {
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

pub fn default_tool_config_dir(home: &std::path::Path, tool_id: &str) -> Result<PathBuf, String> {
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

pub fn resolve_tool_config_dir(
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

pub fn resolve_tool_config_path(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<PathBuf, String> {
    if tool_id == "hermes" {
        return hermes::config_path(conn);
    }
    Ok(resolve_tool_config_dir(conn, tool_id)?.join(tool_config_file_name(tool_id)?))
}

pub fn resolve_claude_paths(conn: &rusqlite::Connection) -> Result<(PathBuf, PathBuf), String> {
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

pub fn resolve_tool_skills_dir(
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

pub fn tool_cli_command(tool_id: &str) -> &'static str {
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

pub fn is_executable_file(path: &std::path::Path) -> bool {
    path.is_file()
}

pub fn cli_exists_in_path(command: &str) -> bool {
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

pub fn tool_label(tool_id: &str) -> &'static str {
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

pub fn tool_hidden_dir(tool_id: &str) -> Option<&'static str> {
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

pub fn format_unix_timestamp(value: i64) -> Option<String> {
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

pub fn format_timestamp_text(value: &str) -> Option<String> {
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

pub fn truncate_session_text(text: &str, max_chars: usize) -> String {
    let condensed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if condensed.chars().count() <= max_chars {
        condensed
    } else {
        let mut result = condensed.chars().take(max_chars).collect::<String>();
        result.push_str("...");
        result
    }
}

pub fn count_query_hits(query: &str, values: &[String]) -> usize {
    if query.is_empty() {
        return 0;
    }

    values
        .iter()
        .filter(|value| value.to_lowercase().contains(query))
        .count()
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SessionTokenTotals {
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

    pub fn input_option(self) -> Option<u64> {
        self.has_usage.then_some(self.input_tokens)
    }

    pub fn output_option(self) -> Option<u64> {
        self.has_usage.then_some(self.output_tokens)
    }

    pub fn total_option(self) -> Option<u64> {
        self.has_usage.then_some(self.total_tokens)
    }
}

pub fn read_token_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

pub fn object_usage_totals(
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

pub fn accumulate_token_usage_from_value(
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

pub fn normalize_session_query(query: Option<String>) -> String {
    query.unwrap_or_default().trim().to_lowercase()
}

pub fn session_roots_for_tool(
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

pub fn is_session_candidate_path(
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

pub fn collect_session_candidate_files(
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

pub fn preferred_texts_from_value(
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

pub fn read_session_token_totals_from_jsonl(path: &std::path::Path) -> SessionTokenTotals {
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
pub fn resolve_cli_path(cmd: &str) -> String {
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
pub fn shell_quote_cli(path: &str) -> String {
    if path.contains(' ') {
        format!("\"{}\"", path)
    } else {
        path.to_string()
    }
}

pub fn codex_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("codex"));
    format!("{cli} resume {session_id}")
}

pub fn claude_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("claude"));
    format!("{cli} --resume {session_id}")
}

pub fn gemini_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("gemini"));
    format!("{cli} --resume {session_id}")
}

pub fn opencode_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("opencode"));
    format!("{cli} session resume {session_id}")
}

pub fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub fn resolve_openclaw_session_key(
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

pub fn openclaw_resume_command(
    source_path: Option<&str>,
    session_id: &str,
) -> Result<String, String> {
    let session_key = resolve_openclaw_session_key(source_path, session_id)?;
    Ok(format!(
        "openclaw tui --session {}",
        shell_single_quote(&session_key)
    ))
}

pub fn tool_supports_session_resume(tool_id: &str) -> bool {
    match tool_id {
        "codex" => cli_exists_in_path("codex"),
        "claude" => cli_exists_in_path("claude"),
        "gemini" => cli_exists_in_path("gemini"),
        "opencode" => cli_exists_in_path("opencode"),
        "openclaw" => cli_exists_in_path("openclaw"),
        _ => false,
    }
}

pub fn write_default_file_if_missing(
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

pub fn ensure_dir_exists(path: &std::path::Path, created_dirs: &mut usize) -> Result<(), String> {
    if path.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    *created_dirs += 1;
    Ok(())
}
