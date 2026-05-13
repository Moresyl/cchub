#![allow(clippy::too_many_arguments)]
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use crate::copilot_auth::{self, CopilotAuthState};
use crate::db::DbState;
use crate::hermes;
use crate::shared::{github_release, github_urls, http_client};

use super::config_profiles::*;
use super::log_command_timing;
use super::statusline::*;
use super::types::*;

// ── Proxy Settings ──

/// Set HTTP/HTTPS proxy for all network requests (persisted to database)
#[tauri::command]
pub fn set_proxy(proxy_url: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    if proxy_url.trim().is_empty() {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('proxy_url', '')",
            [],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let url = proxy_url.trim().to_string();
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('proxy_url', ?1)",
            rusqlite::params![url],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Get current proxy setting
#[tauri::command]
pub fn get_proxy(db: State<'_, DbState>) -> String {
    // Read from database. Network calls inject this per reqwest::Client.
    if let Ok(conn) = db.0.lock() {
        if let Ok(proxy) = conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'proxy_url'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            if !proxy.is_empty() {
                return proxy;
            }
        }
    }
    String::new()
}

#[tauri::command]
pub fn get_visible_apps(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let stored = get_json_app_setting::<Vec<String>>(&conn, VISIBLE_APPS_SETTING_KEY)?;
    Ok(stored
        .map(normalize_visible_apps)
        .unwrap_or_else(default_visible_apps))
}

#[tauri::command]
pub fn set_visible_apps(
    visible_apps: Vec<String>,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    let normalized = normalize_visible_apps(visible_apps);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, VISIBLE_APPS_SETTING_KEY, &normalized)?;
    Ok(normalized)
}

#[tauri::command]
pub fn get_welcome_completed(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(get_json_app_setting::<bool>(&conn, WELCOME_COMPLETED_SETTING_KEY)?.unwrap_or(false))
}

#[tauri::command]
pub fn set_welcome_completed(completed: bool, db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, WELCOME_COMPLETED_SETTING_KEY, &completed)?;
    Ok(completed)
}

#[tauri::command]
pub fn get_hermes_root_override(db: State<'_, DbState>) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    hermes::read_root_override(&conn)
}

#[tauri::command]
pub fn set_hermes_root_override(
    value: Option<String>,
    db: State<'_, DbState>,
) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    hermes::write_root_override(&conn, value.as_deref())
}

#[tauri::command]
pub fn get_window_preferences(db: State<'_, DbState>) -> Result<WindowPreferences, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_window_preferences_from_conn(&conn))
}

#[tauri::command]
pub fn get_log_preferences(db: State<'_, DbState>) -> Result<LogPreferences, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_log_preferences_from_conn(&conn))
}

#[tauri::command]
pub fn set_log_preferences(
    preferences: LogPreferences,
    db: State<'_, DbState>,
) -> Result<LogPreferences, String> {
    let sanitized = LogPreferences {
        level: normalize_log_level(&preferences.level),
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, LOG_PREFERENCES_SETTING_KEY, &sanitized)?;
    apply_log_preferences(&sanitized);
    crate::utils::append_runtime_log(
        "info",
        "settings",
        &format!("Log level changed to {}", sanitized.level),
    );
    Ok(sanitized)
}

#[tauri::command]
pub fn get_log_file_targets() -> LogFileTargets {
    build_log_file_targets()
}

#[tauri::command]
pub fn get_updater_environment_state() -> UpdaterEnvironmentState {
    updater_environment_state()
}

#[tauri::command]
pub fn set_window_preferences(
    preferences: WindowPreferences,
    db: State<'_, DbState>,
) -> Result<WindowPreferences, String> {
    sync_launch_at_login(preferences.launch_at_login)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, WINDOW_PREFERENCES_SETTING_KEY, &preferences)?;
    Ok(preferences)
}

#[tauri::command]
pub fn get_terminal_preferences(db: State<'_, DbState>) -> Result<TerminalPreferences, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_terminal_preferences_from_conn(&conn)
}

#[tauri::command]
pub fn set_preferred_terminal(
    terminal_id: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let preferences = read_terminal_preferences_from_conn(&conn)?;
    if !preferences
        .options
        .iter()
        .any(|option| option.id == terminal_id)
    {
        return Err(format!("Unsupported terminal: {terminal_id}"));
    }
    set_text_app_setting(&conn, PREFERRED_TERMINAL_SETTING_KEY, &terminal_id)?;
    Ok(terminal_id)
}

#[tauri::command]
pub fn open_in_preferred_terminal(
    path: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let preferences = read_terminal_preferences_from_conn(&conn)?;
    drop(conn);
    let target_dir = normalize_terminal_target(path)?;
    launch_preferred_terminal_impl(&preferences, &target_dir, None).map(|_| ())
}

#[tauri::command]
pub fn resume_session_in_preferred_terminal(
    tool_id: String,
    session_id: String,
    cwd: Option<String>,
    source_path: Option<String>,
    db: State<'_, DbState>,
) -> Result<SessionResumeResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let preferences = read_terminal_preferences_from_conn(&conn)?;
    drop(conn);

    let command = build_session_resume_command(&tool_id, &session_id, source_path.as_deref())?;
    let target_dir = normalize_terminal_target(cwd)?;
    let launched = launch_preferred_terminal_impl(&preferences, &target_dir, Some(&command))?;

    Ok(SessionResumeResult {
        launched,
        command,
        cwd: Some(target_dir.to_string_lossy().to_string()),
    })
}

#[tauri::command]
pub fn get_environment_conflicts() -> Result<Vec<EnvironmentConflict>, String> {
    Ok(scan_environment_conflicts())
}

/// Open a native folder picker dialog and return the selected path
#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select folder")
        .pick_folder()
        .await;
    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Open a native file picker dialog and return the selected path
#[tauri::command]
pub async fn pick_file() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Select file")
        .add_filter("Config", &["json", "toml", "yaml", "yml"])
        .pick_file()
        .await;
    Ok(file.map(|f| f.path().to_string_lossy().to_string()))
}

/// Read a tool's current config file content
#[tauri::command]
pub fn read_tool_config(tool_id: String, db: State<'_, DbState>) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_tool_snapshot(&conn, &tool_id)
}

#[tauri::command]
pub fn search_openclaw_daily_memory(
    query: Option<String>,
    limit: Option<usize>,
    db: State<'_, DbState>,
) -> Result<Vec<OpenClawDailyMemoryEntry>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let query = query.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    let max_results = limit.unwrap_or(30).clamp(1, 100);
    let mut entries = Vec::new();
    let mut scanned_roots = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        let global_dir = home.join(".openclaw");
        if global_dir.exists() && scanned_roots.insert(global_dir.to_string_lossy().to_string()) {
            collect_openclaw_daily_memory_files(
                &global_dir,
                &global_dir,
                "global",
                None,
                query.as_deref(),
                &mut entries,
                0,
            );
        }
    }

    for project_root in discover_project_roots(&conn) {
        let project_name = project_root
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| project_root.to_string_lossy().to_string());
        let memory_root = project_root.join(".openclaw");
        if !memory_root.exists() {
            continue;
        }
        let key = memory_root.to_string_lossy().to_string();
        if !scanned_roots.insert(key) {
            continue;
        }
        collect_openclaw_daily_memory_files(
            &memory_root,
            &memory_root,
            "project",
            Some(&project_name),
            query.as_deref(),
            &mut entries,
            0,
        );
    }

    entries.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    entries.truncate(max_results);
    Ok(entries)
}

#[tauri::command]
pub fn read_openclaw_daily_memory_content(
    path: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let path_buf = std::path::PathBuf::from(&path);
    if !is_valid_openclaw_daily_memory_path(&path_buf, &conn) {
        return Err("Invalid OpenClaw Daily Memory path".to_string());
    }
    std::fs::read_to_string(path_buf).map_err(|e| e.to_string())
}

fn load_codex_history_index(root: &std::path::Path) -> HashMap<String, Vec<String>> {
    let mut index = HashMap::new();
    let history_path = root.join("history.jsonl");
    let file = match std::fs::File::open(history_path) {
        Ok(file) => file,
        Err(_) => return index,
    };

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(session_id) = value.get("session_id").and_then(|item| item.as_str()) else {
            continue;
        };
        let Some(text) = value.get("text").and_then(|item| item.as_str()) else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        index
            .entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(trimmed.to_string());
    }

    index
}

fn codex_state_databases(root: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(read_dir) = std::fs::read_dir(root) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !file_name.starts_with("state_") || !file_name.ends_with(".sqlite") {
                continue;
            }
            if seen.insert(path.to_string_lossy().to_string()) {
                paths.push(path);
            }
        }
    }

    let fallback = root.join("state.sqlite");
    if fallback.exists() && seen.insert(fallback.to_string_lossy().to_string()) {
        paths.push(fallback);
    }

    paths.sort();
    paths.reverse();
    paths
}

/// 并行版 codex 扫描：plan 中的 root / db_files / generic_roots 已在 db lock 内备好,
/// 此处只做文件 IO + SQLite 读取，不再依赖主 db 连接，可以安全跨线程执行。
fn scan_codex_sessions_from_plan(
    root: Option<&std::path::Path>,
    db_files: &[PathBuf],
    generic_roots: &[PathBuf],
    query: &str,
) -> Vec<SessionSummary> {
    let Some(root) = root else { return Vec::new() };
    if !root.exists() {
        return Vec::new();
    }

    let history_index = load_codex_history_index(root);
    let mut sessions: Vec<SessionSummary> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for db_path in db_files {
        let external = match rusqlite::Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(conn) => conn,
            Err(_) => continue,
        };

        let mut stmt = match external.prepare(
            "SELECT id, rollout_path, created_at, updated_at, cwd, title, first_user_message
             FROM threads
             ORDER BY updated_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        }) else {
            continue;
        };

        for row in rows.flatten() {
            let (id, rollout_path, created_at_raw, updated_at_raw, cwd, title, first_user_message) =
                row;
            if !seen_ids.insert(id.clone()) {
                continue;
            }
            let rollout_file_path = {
                let path = PathBuf::from(&rollout_path);
                if path.is_absolute() {
                    path
                } else {
                    root.join(&rollout_path)
                }
            };
            let token_totals = read_session_token_totals_from_jsonl(&rollout_file_path);
            let history_items = history_index.get(&id).cloned().unwrap_or_default();
            let preview_source = history_items
                .last()
                .cloned()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    let trimmed = first_user_message.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .unwrap_or_else(|| id.clone());
            let preview = truncate_session_text(&preview_source, 180);
            let search_values = vec![
                title.clone(),
                preview.clone(),
                cwd.clone(),
                first_user_message.clone(),
            ];
            let search_hit_count = count_query_hits(query, &search_values);
            if !query.is_empty() && search_hit_count == 0 {
                continue;
            }
            sessions.push(SessionSummary {
                id: id.clone(),
                tool_id: "codex".to_string(),
                tool_name: "Codex".to_string(),
                title: if title.trim().is_empty() {
                    let trimmed_first_user = first_user_message.trim();
                    if trimmed_first_user.is_empty() {
                        id.clone()
                    } else {
                        truncate_session_text(trimmed_first_user, 80)
                    }
                } else {
                    title
                },
                cwd: (!cwd.trim().is_empty()).then_some(cwd),
                source_kind: "codex_jsonl".to_string(),
                source_backend: "jsonl".to_string(),
                source_path: rollout_path,
                created_at: format_unix_timestamp(created_at_raw),
                updated_at: format_unix_timestamp(updated_at_raw),
                preview,
                message_count: history_items.len(),
                input_tokens: token_totals.input_option(),
                output_tokens: token_totals.output_option(),
                tokens_used: token_totals.total_option(),
                search_hit_count,
                can_resume: tool_supports_session_resume("codex"),
                can_delete: true,
            });
        }
    }

    if !sessions.is_empty() {
        return sessions;
    }

    // sqlite 未命中 → 走 generic 兜底（用预先收集好的 roots，不再访问主 db）
    scan_generic_tool_sessions_from_roots("codex", generic_roots, query)
}

/// 并行版 generic 扫描：roots 已在 db lock 内备好，本函数只做文件遍历 + 解析。
fn scan_generic_tool_sessions_from_roots(
    tool_id: &str,
    roots: &[PathBuf],
    query: &str,
) -> Vec<SessionSummary> {
    let mut jsonl_files = Vec::new();
    let mut sqlite_files = Vec::new();
    let mut seen_jsonl = HashSet::new();
    let mut seen_sqlite = HashSet::new();

    for root in roots {
        collect_session_candidate_files(
            tool_id,
            root,
            root,
            &mut jsonl_files,
            &mut sqlite_files,
            0,
        );
    }

    let mut sessions = Vec::new();
    for path in jsonl_files {
        let key = path.to_string_lossy().to_string();
        if !seen_jsonl.insert(key) {
            continue;
        }
        if let Some(summary) = parse_generic_jsonl_session_summary(tool_id, &path, query) {
            sessions.push(summary);
        }
    }

    for path in sqlite_files {
        let key = path.to_string_lossy().to_string();
        if !seen_sqlite.insert(key) {
            continue;
        }
        sessions.extend(scan_generic_sqlite_sessions(tool_id, &path, query));
    }

    sessions
}

fn parse_generic_jsonl_session_summary(
    tool_id: &str,
    path: &std::path::Path,
    query: &str,
) -> Option<SessionSummary> {
    let file = std::fs::File::open(path).ok()?;
    let metadata = std::fs::metadata(path).ok();
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();

    let mut session_id = file_stem.clone();
    let mut title: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut first_message_summary: Option<String> = None;
    let mut created_at: Option<String> = metadata
        .as_ref()
        .and_then(|value| value.created().ok())
        .map(format_local_datetime);
    let mut updated_at: Option<String> = metadata
        .as_ref()
        .and_then(|value| value.modified().ok())
        .map(format_local_datetime);
    let mut preview: Option<String> = None;
    let mut message_count = 0usize;
    let mut token_totals = SessionTokenTotals::default();

    // 长会话（如 claude 历史几千条消息）的 jsonl 可能有上万行。原实现对每一行都做
    // serde_json::from_str + token accumulator，导致 get_sessions 在 list 阶段就被
    // 一两个大文件拖到几秒。这里做两级 cap：
    //   - token 扫描上限 MAX_TOKEN_LINES：超出后直接 break，停止 JSON 解析
    //   - metadata 提取上限 MAX_META_LINES：仍 continue 但跳过 metadata 字段，
    //     用于让 created_at/title/cwd/preview 之类字段在前面行内尽快定位完
    const MAX_TOKEN_LINES: usize = 2000;
    const MAX_META_LINES: usize = 120;
    for (line_index, line) in BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        if line_index >= MAX_TOKEN_LINES {
            break;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        accumulate_token_usage_from_value(&value, &mut token_totals, 0);

        if line_index >= MAX_META_LINES {
            continue;
        }

        if let Some(found_id) = value
            .get("sessionId")
            .or_else(|| value.get("session_id"))
            .and_then(|item| item.as_str())
        {
            if !found_id.trim().is_empty() {
                session_id = found_id.trim().to_string();
            }
        } else if value.get("type").and_then(|item| item.as_str()) == Some("session_meta") {
            if let Some(found_id) = value
                .get("payload")
                .and_then(|item| item.get("id"))
                .and_then(|item| item.as_str())
            {
                if !found_id.trim().is_empty() {
                    session_id = found_id.trim().to_string();
                }
            }
        }

        if title.is_none() {
            title = value
                .get("title")
                .and_then(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .or_else(|| {
                    value
                        .get("payload")
                        .and_then(|item| item.get("title"))
                        .and_then(|item| item.as_str())
                        .map(|item| item.trim().to_string())
                        .filter(|item| !item.is_empty())
                });
        }

        if cwd.is_none() {
            cwd = value
                .get("cwd")
                .and_then(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .or_else(|| {
                    value
                        .get("payload")
                        .and_then(|item| item.get("cwd"))
                        .and_then(|item| item.as_str())
                        .map(|item| item.trim().to_string())
                        .filter(|item| !item.is_empty())
                });
        }

        if let Some(timestamp) = value.get("timestamp").and_then(|item| item.as_str()) {
            let formatted = format_timestamp_text(timestamp);
            if created_at.is_none() {
                created_at = formatted.clone();
            }
            updated_at = formatted.or(updated_at);
        } else if let Some(ts) = value.get("ts").and_then(|item| item.as_i64()) {
            let formatted = format_unix_timestamp(ts);
            if created_at.is_none() {
                created_at = formatted.clone();
            }
            updated_at = formatted.or(updated_at);
        }

        let mut texts = Vec::new();
        preferred_texts_from_value(&value, &mut texts, 0);
        if let Some(text) = texts.into_iter().find(|item| !item.trim().is_empty()) {
            message_count += 1;
            if preview.is_none() {
                preview = Some(truncate_session_text(&text, 180));
            }
            if first_message_summary.is_none() {
                first_message_summary = Some(truncate_session_text(&text, 80));
            }
        }
    }

    let title = title
        .or(first_message_summary)
        .unwrap_or_else(|| session_id.clone());
    let preview = preview.unwrap_or_else(|| title.clone());
    let search_values = vec![
        title.clone(),
        preview.clone(),
        cwd.clone().unwrap_or_default(),
        session_id.clone(),
    ];
    let search_hit_count = count_query_hits(query, &search_values);
    if !query.is_empty() && search_hit_count == 0 {
        return None;
    }

    Some(SessionSummary {
        id: session_id,
        tool_id: tool_id.to_string(),
        tool_name: tool_label(tool_id).to_string(),
        title,
        cwd,
        source_kind: format!("{tool_id}_jsonl"),
        source_backend: "jsonl".to_string(),
        source_path: path.to_string_lossy().to_string(),
        created_at,
        updated_at,
        preview,
        message_count,
        input_tokens: token_totals.input_option(),
        output_tokens: token_totals.output_option(),
        tokens_used: token_totals.total_option(),
        search_hit_count,
        can_resume: tool_supports_session_resume(tool_id),
        can_delete: true,
    })
}

fn sqlite_table_columns(
    conn: &rusqlite::Connection,
    table_name: &str,
) -> Result<HashSet<String>, String> {
    let sql = format!("PRAGMA table_info({table_name})");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;

    let mut columns = HashSet::new();
    for row in rows {
        columns.insert(row.map_err(|e| e.to_string())?.to_ascii_lowercase());
    }
    Ok(columns)
}

fn select_sqlite_expr(columns: &HashSet<String>, names: &[&str], fallback: &str) -> String {
    for name in names {
        if columns.contains(&name.to_ascii_lowercase()) {
            return format!("CAST({name} AS TEXT)");
        }
    }
    fallback.to_string()
}

fn scan_generic_sqlite_sessions(
    tool_id: &str,
    db_path: &std::path::Path,
    query: &str,
) -> Vec<SessionSummary> {
    let external = match rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(conn) => conn,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    let mut seen_ids = HashSet::new();

    for table_name in ["threads", "sessions", "conversations"] {
        let columns = match sqlite_table_columns(&external, table_name) {
            Ok(columns) if !columns.is_empty() => columns,
            _ => continue,
        };

        let id_column = if columns.contains("id") {
            "id"
        } else if columns.contains("session_id") {
            "session_id"
        } else if columns.contains("thread_id") {
            "thread_id"
        } else {
            continue;
        };
        let title_expr = select_sqlite_expr(&columns, &["title", "name"], "''");
        let cwd_expr = select_sqlite_expr(
            &columns,
            &["cwd", "working_directory", "project_path"],
            "NULL",
        );
        let created_expr = select_sqlite_expr(
            &columns,
            &["created_at", "created_ts", "timestamp", "ts"],
            "NULL",
        );
        let updated_expr = select_sqlite_expr(
            &columns,
            &[
                "updated_at",
                "updated_ts",
                "last_updated_at",
                "timestamp",
                "ts",
            ],
            "NULL",
        );
        let sort_column = if columns.contains("updated_at") {
            "updated_at"
        } else if columns.contains("timestamp") {
            "timestamp"
        } else if columns.contains("created_at") {
            "created_at"
        } else {
            "rowid"
        };

        let sql = format!(
            "SELECT CAST({id_column} AS TEXT), {title_expr}, {cwd_expr}, {created_expr}, {updated_expr}
             FROM {table_name}
             ORDER BY {sort_column} DESC
             LIMIT 200"
        );
        let mut stmt = match external.prepare(&sql) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let rows = match stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => continue,
        };

        for row in rows.flatten() {
            let (id, title_raw, cwd_raw, created_raw, updated_raw) = row;
            if !seen_ids.insert(id.clone()) {
                continue;
            }
            let title = title_raw
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{table_name} {id}"));
            let cwd = cwd_raw
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let preview = cwd
                .as_ref()
                .map(|value| truncate_session_text(value, 180))
                .unwrap_or_else(|| truncate_session_text(&title, 180));
            let search_values = vec![
                title.clone(),
                preview.clone(),
                cwd.clone().unwrap_or_default(),
            ];
            let search_hit_count = count_query_hits(query, &search_values);
            if !query.is_empty() && search_hit_count == 0 {
                continue;
            }

            sessions.push(SessionSummary {
                id,
                tool_id: tool_id.to_string(),
                tool_name: tool_label(tool_id).to_string(),
                title,
                cwd,
                source_kind: format!("{tool_id}_sqlite"),
                source_backend: "sqlite".to_string(),
                source_path: db_path.to_string_lossy().to_string(),
                created_at: created_raw.as_deref().and_then(format_timestamp_text),
                updated_at: updated_raw.as_deref().and_then(format_timestamp_text),
                preview,
                message_count: 0,
                input_tokens: None,
                output_tokens: None,
                tokens_used: None,
                search_hit_count,
                can_resume: tool_supports_session_resume(tool_id),
                can_delete: false,
            });
        }
    }

    sessions
}

fn scan_sessions_from_conn(
    conn: &rusqlite::Connection,
    tool_id: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SessionSummary>, String> {
    let query = normalize_session_query(query);
    let max_results = limit.unwrap_or(200).clamp(1, 500);
    let requested_tool = tool_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let tool_ids: Vec<&str> = match requested_tool {
        Some("claude") => vec!["claude"],
        Some("codex") => vec!["codex"],
        Some("gemini") => vec!["gemini"],
        Some("opencode") => vec!["opencode"],
        Some("openclaw") => vec!["openclaw"],
        Some("hermes") => vec!["hermes"],
        _ => vec![
            "claude", "codex", "gemini", "opencode", "openclaw", "hermes",
        ],
    };

    // 第一阶段：在 db lock 持有期间收集每个 tool 的 session 根目录与 codex 候选文件，
    // 这是唯一需要 conn 的工作。之后释放 db 影响，把昂贵的文件 IO + JSON 解析
    // 放到独立线程并行执行 —— 6 个 tool 同时跑，磁盘 IO 并发度直接提升 ~6x。
    enum ToolPlan {
        Codex {
            root: Option<PathBuf>,
            db_files: Vec<PathBuf>,
            generic_roots: Vec<PathBuf>,
        },
        Generic {
            roots: Vec<PathBuf>,
        },
    }

    let plans: Vec<(&str, ToolPlan)> = tool_ids
        .into_iter()
        .map(|tool| {
            if tool == "codex" {
                let root = resolve_tool_config_dir(conn, "codex")
                    .ok()
                    .filter(|p| p.exists());
                let db_files = root
                    .as_ref()
                    .map(|r| codex_state_databases(r))
                    .unwrap_or_default();
                let generic_roots = session_roots_for_tool(conn, "codex").unwrap_or_default();
                (
                    tool,
                    ToolPlan::Codex {
                        root,
                        db_files,
                        generic_roots,
                    },
                )
            } else {
                let roots = session_roots_for_tool(conn, tool).unwrap_or_default();
                (tool, ToolPlan::Generic { roots })
            }
        })
        .collect();

    // 第二阶段：并行扫描（不再需要 conn）。std::thread::scope 让每个 tool 的工作借用
    // `query` 与 plan 的引用，scope 结束前 join 所有子线程，确保安全。
    let query_ref = &query;
    let sessions: Vec<SessionSummary> = std::thread::scope(|s| {
        let handles: Vec<_> = plans
            .into_iter()
            .map(|(tool, plan)| {
                s.spawn(move || -> Vec<SessionSummary> {
                    match plan {
                        ToolPlan::Codex {
                            root,
                            db_files,
                            generic_roots,
                        } => scan_codex_sessions_from_plan(
                            root.as_deref(),
                            &db_files,
                            &generic_roots,
                            query_ref,
                        ),
                        ToolPlan::Generic { roots } => {
                            scan_generic_tool_sessions_from_roots(tool, &roots, query_ref)
                        }
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });

    let mut sessions = sessions;
    sessions.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
    });
    sessions.truncate(max_results);
    Ok(sessions)
}

fn codex_message_content(content: Option<&serde_json::Value>) -> String {
    let mut texts = Vec::new();
    if let Some(content) = content {
        preferred_texts_from_value(content, &mut texts, 0);
    }
    texts.join("\n\n")
}

fn parse_codex_session_entries(path: &std::path::Path) -> Result<Vec<SessionEntry>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();

    for (index, line) in BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let timestamp = value
            .get("timestamp")
            .and_then(|item| item.as_str())
            .and_then(format_timestamp_text);
        let item_type = value
            .get("type")
            .and_then(|item| item.as_str())
            .unwrap_or_default();

        match item_type {
            "response_item" => {
                let Some(payload) = value.get("payload") else {
                    continue;
                };
                let payload_type = payload
                    .get("type")
                    .and_then(|item| item.as_str())
                    .unwrap_or_default();
                match payload_type {
                    "message" => {
                        let role = payload
                            .get("role")
                            .and_then(|item| item.as_str())
                            .unwrap_or("assistant");
                        if matches!(role, "developer" | "system") {
                            continue;
                        }
                        let content = codex_message_content(payload.get("content"));
                        if content.trim().is_empty() {
                            continue;
                        }
                        entries.push(SessionEntry {
                            id: format!("entry-{index}"),
                            kind: role.to_string(),
                            title: match role {
                                "user" => "User".to_string(),
                                "assistant" => "Assistant".to_string(),
                                _ => role.to_string(),
                            },
                            content,
                            timestamp,
                        });
                    }
                    "function_call" => {
                        let name = payload
                            .get("name")
                            .and_then(|item| item.as_str())
                            .unwrap_or("tool");
                        let content = payload
                            .get("arguments")
                            .and_then(|item| item.as_str())
                            .unwrap_or("")
                            .to_string();
                        entries.push(SessionEntry {
                            id: format!("entry-{index}"),
                            kind: "tool_call".to_string(),
                            title: format!("Call {name}"),
                            content,
                            timestamp,
                        });
                    }
                    "function_call_output" => {
                        let content = payload
                            .get("output")
                            .and_then(|item| item.as_str())
                            .unwrap_or("")
                            .to_string();
                        if content.trim().is_empty() {
                            continue;
                        }
                        entries.push(SessionEntry {
                            id: format!("entry-{index}"),
                            kind: "tool_output".to_string(),
                            title: "Tool Output".to_string(),
                            content,
                            timestamp,
                        });
                    }
                    "reasoning" => {
                        let mut texts = Vec::new();
                        if let Some(summary) = payload.get("summary") {
                            preferred_texts_from_value(summary, &mut texts, 0);
                        }
                        if texts.is_empty() {
                            continue;
                        }
                        entries.push(SessionEntry {
                            id: format!("entry-{index}"),
                            kind: "reasoning".to_string(),
                            title: "Reasoning".to_string(),
                            content: texts.join("\n\n"),
                            timestamp,
                        });
                    }
                    _ => {}
                }
            }
            "event_msg" => {
                let Some(payload_type) = value
                    .get("payload")
                    .and_then(|item| item.get("type"))
                    .and_then(|item| item.as_str())
                else {
                    continue;
                };
                if payload_type == "token_count" {
                    continue;
                }
                entries.push(SessionEntry {
                    id: format!("entry-{index}"),
                    kind: "event".to_string(),
                    title: payload_type.replace('_', " "),
                    content: payload_type.to_string(),
                    timestamp,
                });
            }
            "turn_context" => {
                let mut lines = Vec::new();
                if let Some(cwd) = value
                    .get("payload")
                    .and_then(|item| item.get("cwd"))
                    .and_then(|item| item.as_str())
                {
                    lines.push(format!("cwd: {cwd}"));
                }
                if let Some(model) = value
                    .get("payload")
                    .and_then(|item| item.get("model"))
                    .and_then(|item| item.as_str())
                {
                    lines.push(format!("model: {model}"));
                }
                if let Some(approval) = value
                    .get("payload")
                    .and_then(|item| item.get("approval_policy"))
                    .and_then(|item| item.as_str())
                {
                    lines.push(format!("approval: {approval}"));
                }
                if lines.is_empty() {
                    continue;
                }
                entries.push(SessionEntry {
                    id: format!("entry-{index}"),
                    kind: "note".to_string(),
                    title: "Context".to_string(),
                    content: lines.join("\n"),
                    timestamp,
                });
            }
            _ => {}
        }
    }

    Ok(entries)
}

fn parse_generic_jsonl_session_entries(
    path: &std::path::Path,
) -> Result<Vec<SessionEntry>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();

    for (index, line) in BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let mut texts = Vec::new();
        preferred_texts_from_value(&value, &mut texts, 0);
        let content = texts.join("\n\n");
        if content.trim().is_empty() {
            continue;
        }
        let kind = value
            .get("role")
            .and_then(|item| item.as_str())
            .or_else(|| value.get("type").and_then(|item| item.as_str()))
            .unwrap_or("entry")
            .to_string();
        let timestamp = value
            .get("timestamp")
            .and_then(|item| item.as_str())
            .and_then(format_timestamp_text)
            .or_else(|| {
                value
                    .get("ts")
                    .and_then(|item| item.as_i64())
                    .and_then(format_unix_timestamp)
            });
        entries.push(SessionEntry {
            id: format!("entry-{index}"),
            kind: kind.clone(),
            title: kind.replace('_', " "),
            content,
            timestamp,
        });
    }

    Ok(entries)
}

fn load_generic_sqlite_entries(
    db_path: &std::path::Path,
    session_id: &str,
) -> Result<Vec<SessionEntry>, String> {
    let external =
        rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;

    for table_name in ["messages", "entries", "events"] {
        let columns = match sqlite_table_columns(&external, table_name) {
            Ok(columns) if !columns.is_empty() => columns,
            _ => continue,
        };
        let session_column = if columns.contains("session_id") {
            "session_id"
        } else if columns.contains("thread_id") {
            "thread_id"
        } else if columns.contains("conversation_id") {
            "conversation_id"
        } else {
            continue;
        };
        let role_expr = select_sqlite_expr(&columns, &["role", "kind", "type"], "'entry'");
        let content_expr =
            select_sqlite_expr(&columns, &["content", "text", "body", "message"], "''");
        let timestamp_expr = select_sqlite_expr(
            &columns,
            &["created_at", "updated_at", "timestamp", "ts"],
            "NULL",
        );
        let sort_column = if columns.contains("created_at") {
            "created_at"
        } else if columns.contains("timestamp") {
            "timestamp"
        } else {
            "rowid"
        };

        let sql = format!(
            "SELECT {role_expr}, {content_expr}, {timestamp_expr}
             FROM {table_name}
             WHERE CAST({session_column} AS TEXT) = ?1
             ORDER BY {sort_column} ASC
             LIMIT 400"
        );
        let mut stmt = match external.prepare(&sql) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let rows = match stmt.query_map(rusqlite::params![session_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => continue,
        };

        let mut entries = Vec::new();
        for (index, row) in rows.flatten().enumerate() {
            let (role, content, timestamp) = row;
            let content = content.unwrap_or_default();
            if content.trim().is_empty() {
                continue;
            }
            let kind = role.unwrap_or_else(|| "entry".to_string());
            entries.push(SessionEntry {
                id: format!("sqlite-entry-{index}"),
                kind: kind.clone(),
                title: kind.replace('_', " "),
                content,
                timestamp: timestamp.as_deref().and_then(format_timestamp_text),
            });
        }
        if !entries.is_empty() {
            return Ok(entries);
        }
    }

    Ok(vec![SessionEntry {
        id: "sqlite-fallback".to_string(),
        kind: "note".to_string(),
        title: "Metadata".to_string(),
        content: format!("Session metadata is stored in {}", db_path.display()),
        timestamp: None,
    }])
}

fn load_session_detail(session: &SessionSummary) -> Result<SessionDetail, String> {
    let source_path = std::path::PathBuf::from(&session.source_path);
    let entries = if session.tool_id == "codex" && session.source_kind == "codex_jsonl" {
        parse_codex_session_entries(&source_path)?
    } else if session.source_backend == "jsonl" {
        parse_generic_jsonl_session_entries(&source_path)?
    } else {
        load_generic_sqlite_entries(&source_path, &session.id)?
    };

    Ok(SessionDetail {
        session: session.clone(),
        entries,
    })
}

fn is_valid_session_source_path(
    conn: &rusqlite::Connection,
    tool_id: &str,
    source_path: &str,
) -> bool {
    let source = PathBuf::from(source_path);
    let normalized_source = source.canonicalize().unwrap_or(source);
    let Ok(roots) = session_roots_for_tool(conn, tool_id) else {
        return false;
    };

    roots.into_iter().any(|root| {
        let normalized_root = root.canonicalize().unwrap_or(root);
        normalized_source.starts_with(&normalized_root)
    })
}

fn scrub_codex_history(root: &std::path::Path, session_id: &str) -> Result<(), String> {
    let history_path = root.join("history.jsonl");
    if !history_path.exists() {
        return Ok(());
    }

    let file = std::fs::File::open(&history_path).map_err(|e| e.to_string())?;
    let mut kept_lines = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let keep = serde_json::from_str::<serde_json::Value>(&line)
            .ok()
            .and_then(|value| {
                value
                    .get("session_id")
                    .and_then(|item| item.as_str())
                    .map(|id| id != session_id)
            })
            .unwrap_or(true);
        if keep {
            kept_lines.push(line);
        }
    }

    let content = if kept_lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", kept_lines.join("\n"))
    };
    crate::utils::atomic_write_string(&history_path, &content).map_err(|e| e.to_string())
}

fn delete_codex_session_records(root: &std::path::Path, session_id: &str) -> Result<(), String> {
    scrub_codex_history(root, session_id)?;

    for db_path in codex_state_databases(root) {
        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(conn) => conn,
            Err(_) => continue,
        };
        let _ = conn.execute(
            "DELETE FROM thread_dynamic_tools WHERE thread_id = ?1",
            rusqlite::params![session_id],
        );
        let _ = conn.execute(
            "DELETE FROM thread_spawn_edges WHERE child_thread_id = ?1 OR parent_thread_id = ?1",
            rusqlite::params![session_id],
        );
        let _ = conn.execute(
            "DELETE FROM agent_job_items WHERE assigned_thread_id = ?1",
            rusqlite::params![session_id],
        );
        let _ = conn.execute(
            "DELETE FROM threads WHERE id = ?1",
            rusqlite::params![session_id],
        );
    }

    Ok(())
}

fn delete_session_impl(
    conn: &rusqlite::Connection,
    tool_id: &str,
    session_id: &str,
    source_path: &str,
    source_backend: &str,
) -> Result<(), String> {
    if !is_valid_session_source_path(conn, tool_id, source_path) {
        return Err("Invalid session source path".to_string());
    }
    let root = resolve_tool_config_dir(conn, tool_id)?;

    if tool_id == "codex" {
        delete_codex_session_records(&root, session_id)?;
    }

    if source_backend == "jsonl" {
        let path = PathBuf::from(source_path);
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_sessions(
    tool_id: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
    db: State<'_, DbState>,
) -> Result<Vec<SessionSummary>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        scan_sessions_from_conn(&conn, tool_id, query, limit)
    })();
    log_command_timing("get_sessions", started_at);
    result
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn get_session_detail(
    tool_id: String,
    session_id: String,
    source_path: String,
    source_kind: String,
    source_backend: String,
    cwd: Option<String>,
    title: String,
    preview: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    message_count: usize,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    tokens_used: Option<u64>,
    can_resume: bool,
    can_delete: bool,
    db: State<'_, DbState>,
) -> Result<SessionDetail, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if !is_valid_session_source_path(&conn, &tool_id, &source_path) {
            return Err("Invalid session source path".to_string());
        }

        let summary = SessionSummary {
            id: session_id,
            tool_id: tool_id.clone(),
            tool_name: tool_label(&tool_id).to_string(),
            title,
            cwd,
            source_kind,
            source_backend,
            source_path,
            created_at,
            updated_at,
            preview,
            message_count,
            input_tokens,
            output_tokens,
            tokens_used,
            search_hit_count: 0,
            can_resume,
            can_delete,
        };
        load_session_detail(&summary)
    })();
    log_command_timing("get_session_detail", started_at);
    result
}

#[tauri::command]
pub fn delete_session(
    tool_id: String,
    session_id: String,
    source_path: String,
    source_backend: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    delete_session_impl(&conn, &tool_id, &session_id, &source_path, &source_backend)
}

#[tauri::command]
pub fn delete_sessions(
    sessions: Vec<SessionDeleteTarget>,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut deleted = 0usize;

    for session in sessions {
        delete_session_impl(
            &conn,
            &session.tool_id,
            &session.session_id,
            &session.source_path,
            &session.source_backend,
        )?;
        deleted += 1;
    }

    Ok(deleted)
}

/// Write a tool's config file content
#[tauri::command]
pub fn write_tool_config(
    tool_id: String,
    content: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    apply_tool_snapshot(&conn, &tool_id, &content)?;
    crate::utils::append_runtime_log(
        "info",
        "tools",
        &format!("Updated tool config for {tool_id}"),
    );
    Ok(())
}

#[tauri::command]
pub fn read_codex_toml_structured(
    path: Option<String>,
    db: State<'_, DbState>,
) -> Result<CodexTomlStructuredConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (config_path, auth_path) = resolve_codex_structured_paths(&conn, path)?;
    let content = if config_path.exists() {
        std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let auth = read_json_file_or_default(&auth_path)?;
    let api_key = auth
        .get("OPENAI_API_KEY")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    Ok(read_codex_structured_config_from_content(&content, api_key))
}

#[tauri::command]
pub fn write_codex_toml_structured(
    path: Option<String>,
    raw_toml: String,
    config: CodexTomlStructuredConfig,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (config_path, auth_path) = resolve_codex_structured_paths(&conn, path)?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let written_toml = write_codex_structured_config_to_text(&raw_toml, &config);
    crate::utils::atomic_write_string(&config_path, &written_toml).map_err(|e| e.to_string())?;

    let mut auth = read_json_file_or_default(&auth_path)?;
    if !auth.is_object() {
        auth = serde_json::json!({});
    }
    if let Some(api_key) = normalized_non_empty(&config.api_key) {
        auth["OPENAI_API_KEY"] = serde_json::json!(api_key);
    } else if let Some(auth_obj) = auth.as_object_mut() {
        auth_obj.remove("OPENAI_API_KEY");
    }
    write_json_file_pretty(&auth_path, &auth)?;

    Ok(written_toml)
}

#[tauri::command]
pub fn get_common_config_snippet(
    tool_id: String,
    db: State<'_, DbState>,
) -> Result<CommonConfigSnippet, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_common_config_snippet_from_conn(&conn, &tool_id)
}

#[tauri::command]
pub fn set_common_config_snippet(
    tool_id: String,
    snippet: CommonConfigSnippet,
    db: State<'_, DbState>,
) -> Result<CommonConfigSnippet, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    write_common_config_snippet_to_conn(&conn, &tool_id, snippet)
}

#[tauri::command]
pub fn read_claude_config_toggles(db: State<'_, DbState>) -> Result<ClaudeConfigToggles, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_claude_config_toggles_from_conn(&conn)
}

#[tauri::command]
pub fn write_claude_config_toggle(
    key: String,
    enabled: bool,
    db: State<'_, DbState>,
) -> Result<ClaudeConfigToggles, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    write_claude_config_toggle_to_conn(&conn, &key, enabled)
}

/// Get Claude Code permissions level (0=strict, 1=standard, 2=relaxed, 3=bypass)
#[tauri::command]
pub fn get_claude_permissions_level() -> Result<u32, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok(0);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let mode = settings
        .get("permissions")
        .and_then(|p| p.get("defaultMode"))
        .and_then(|m| m.as_str())
        .unwrap_or("");

    if mode == "bypassPermissions" {
        return Ok(3);
    }

    let allow = settings
        .get("permissions")
        .and_then(|p| p.get("allow"))
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    // NOTE: level 3 is already short-circuited above via `mode == "bypassPermissions"`.
    // The setter writes level 2 with Write(*) but NOT Bash(*), so checking both
    // here misses level 2 and falsely reports it as level 1. Use Write(*) alone.
    if allow.contains(&"Write(*)") {
        Ok(2)
    } else if allow.contains(&"Read(*)") {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Set Claude Code permissions level (0=strict, 1=standard, 2=relaxed, 3=bypass)
#[tauri::command]
pub fn set_claude_permissions_level(level: u32) -> Result<u32, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");

    let mut settings: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };

    let (allow, mode, skip_prompt): (Vec<&str>, &str, bool) = match level {
        0 => (vec![], "normal", false),
        1 => (
            vec!["Read(*)", "Glob(*)", "Grep(*)", "WebSearch(*)"],
            "normal",
            false,
        ),
        2 => (
            vec![
                "Read(*)",
                "Write(*)",
                "Edit(*)",
                "Glob(*)",
                "Grep(*)",
                "WebFetch(*)",
                "WebSearch(*)",
                "Agent(*)",
                "NotebookEdit(*)",
            ],
            "normal",
            false,
        ),
        3 => (
            vec![
                "Bash(*)",
                "Read(*)",
                "Write(*)",
                "Edit(*)",
                "Glob(*)",
                "Grep(*)",
                "WebFetch(*)",
                "WebSearch(*)",
                "Agent(*)",
                "NotebookEdit(*)",
                "Skill(*)",
                "mcp__*",
            ],
            "bypassPermissions",
            true,
        ),
        _ => return Err("Invalid level".to_string()),
    };

    let allow_arr: Vec<serde_json::Value> = allow.iter().map(|s| serde_json::json!(s)).collect();
    settings["permissions"] = serde_json::json!({
        "allow": allow_arr,
        "deny": [],
        "defaultMode": mode,
    });
    settings["skipDangerousModePermissionPrompt"] = serde_json::json!(skip_prompt);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(level)
}

/// Get Claude Code auto-update channel
#[tauri::command]
pub fn get_claude_auto_update() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok("latest".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    // Canonical disable: env.DISABLE_AUTOUPDATER = "1". Check this first, since
    // Claude Code's autoUpdatesChannel only accepts "stable"/"latest" — there is
    // no "disabled" channel value, so removing the key alone round-trips back
    // to the default ("latest").
    let disabled = settings
        .get("env")
        .and_then(|e| e.get("DISABLE_AUTOUPDATER"))
        .and_then(|v| v.as_str())
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if disabled {
        return Ok("disabled".to_string());
    }
    Ok(settings
        .get("autoUpdatesChannel")
        .and_then(|v| v.as_str())
        .unwrap_or("latest")
        .to_string())
}

/// Set Claude Code auto-update channel
#[tauri::command]
pub fn set_claude_auto_update(channel: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };
    if channel == "disabled" {
        // Canonical disable: env.DISABLE_AUTOUPDATER = "1". Also clear the
        // channel key so the getter has a single source of truth.
        if !settings.is_object() {
            settings = serde_json::json!({});
        }
        let obj = ensure_json_object(&mut settings);
        obj.remove("autoUpdatesChannel");
        let env_entry = obj
            .entry("env".to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(env_entry)
            .insert("DISABLE_AUTOUPDATER".to_string(), serde_json::json!("1"));
    } else {
        // Re-enable: clear the env flag (if present) and write the channel.
        if let Some(env_obj) = settings.get_mut("env").and_then(|v| v.as_object_mut()) {
            env_obj.remove("DISABLE_AUTOUPDATER");
        }
        // Drop an empty env object to keep settings.json tidy.
        if settings
            .get("env")
            .and_then(|v| v.as_object())
            .map(|o| o.is_empty())
            .unwrap_or(false)
        {
            settings.as_object_mut().map(|o| o.remove("env"));
        }
        settings["autoUpdatesChannel"] = serde_json::json!(channel);
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(channel)
}

/// Get Codex CLI settings (approval_mode, reasoning_effort, disable_response_storage)
#[tauri::command]
pub fn get_codex_settings() -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".codex").join("config.toml");
    if !path.exists() {
        return Ok(serde_json::json!({
            "approval_mode": "suggest",
            "reasoning_effort": "medium",
            "disable_response_storage": false,
            "context_window_1m": false,
        }));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // NOTE: `content.parse::<toml::Value>()` is broken in toml 1.0 — the
    // `FromStr for Value` impl only parses a single TOML value expression,
    // not a whole document, so it fails on any real config.toml with
    // "unexpected content, expected nothing". Parse as `toml::Table` instead.
    let doc: toml::Table = content
        .parse()
        .map_err(|e: toml::de::Error| e.to_string())?;

    // Read approval mode from personality or dedicated field
    let personality = doc
        .get("personality")
        .and_then(|v| v.as_str())
        .unwrap_or("pragmatic");
    let approval_mode = if personality == "full-auto" {
        "full-auto"
    } else if personality == "auto-edit" {
        "auto-edit"
    } else {
        "suggest"
    };

    let reasoning = doc
        .get("model_reasoning_effort")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");
    let disable_storage = doc
        .get("disable_response_storage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let context_window_1m = doc
        .get("model_context_window")
        .and_then(|v| v.as_integer())
        .is_some_and(|value| value == 1_000_000);

    Ok(serde_json::json!({
        "approval_mode": approval_mode,
        "reasoning_effort": reasoning,
        "disable_response_storage": disable_storage,
        "context_window_1m": context_window_1m,
    }))
}

/// Set a Codex CLI setting
#[tauri::command]
pub fn set_codex_setting(key: String, value: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".codex").join("config.toml");

    let content = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;

    match key.as_str() {
        "approval_mode" => {
            // Codex doesn't have approval_mode directly, map to personality
            doc["personality"] = toml_edit::value(&value);
        }
        "reasoning_effort" => {
            doc["model_reasoning_effort"] = toml_edit::value(&value);
        }
        "disable_response_storage" => {
            doc["disable_response_storage"] = toml_edit::value(value == "true");
        }
        "context_window_1m" => {
            if value == "true" {
                doc["model_context_window"] = toml_edit::value(1_000_000);
            } else {
                doc.remove("model_context_window");
            }
        }
        _ => return Err(format!("Unknown setting: {}", key)),
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &doc.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get Claude Code model setting
#[tauri::command]
pub fn get_claude_model() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok("".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(settings
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

/// Set Claude Code model
#[tauri::command]
pub fn set_claude_model(model: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };
    settings["model"] = serde_json::json!(model);
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(settings["model"].as_str().unwrap_or_default().to_string())
}

/// Get Claude Code Tool Search (ENABLE_TOOL_SEARCH) status from settings.local.json
#[tauri::command]
pub fn get_claude_tool_search(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_claude_config_toggles_from_conn(&conn)?.enable_tool_search)
}

/// Set Claude Code Tool Search (ENABLE_TOOL_SEARCH) in settings.local.json
#[tauri::command]
pub fn set_claude_tool_search(enabled: bool, db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let _ = write_claude_config_toggle_to_conn(&conn, "enableToolSearch", enabled)?;
    Ok(enabled)
}
