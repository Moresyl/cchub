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

use super::super::config_profiles::*;
use super::super::log_command_timing;
use super::super::statusline::*;
use super::super::types::*;
use super::*;

pub fn codex_message_content(content: Option<&serde_json::Value>) -> String {
    let mut texts = Vec::new();
    if let Some(content) = content {
        preferred_texts_from_value(content, &mut texts, 0);
    }
    texts.join("\n\n")
}

pub fn parse_codex_session_entries(path: &std::path::Path) -> Result<Vec<SessionEntry>, String> {
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

pub fn parse_generic_jsonl_session_entries(
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

pub fn load_generic_sqlite_entries(
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

pub fn load_session_detail(session: &SessionSummary) -> Result<SessionDetail, String> {
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

pub fn is_valid_session_source_path(
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

pub fn scrub_codex_history(root: &std::path::Path, session_id: &str) -> Result<(), String> {
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

pub fn delete_codex_session_records(
    root: &std::path::Path,
    session_id: &str,
) -> Result<(), String> {
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

pub fn delete_session_impl(
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
