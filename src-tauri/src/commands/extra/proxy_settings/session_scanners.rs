#![allow(clippy::too_many_arguments)]
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use super::super::config_profiles::*;
use super::super::statusline::*;
use super::super::types::*;

pub fn load_codex_history_index(root: &std::path::Path) -> HashMap<String, Vec<String>> {
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

pub fn codex_state_databases(root: &std::path::Path) -> Vec<PathBuf> {
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
pub fn scan_codex_sessions_from_plan(
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
pub fn scan_generic_tool_sessions_from_roots(
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

pub fn parse_generic_jsonl_session_summary(
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

pub fn sqlite_table_columns(
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

pub fn select_sqlite_expr(columns: &HashSet<String>, names: &[&str], fallback: &str) -> String {
    for name in names {
        if columns.contains(&name.to_ascii_lowercase()) {
            return format!("CAST({name} AS TEXT)");
        }
    }
    fallback.to_string()
}

pub fn scan_generic_sqlite_sessions(
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

pub fn scan_sessions_from_conn(
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
