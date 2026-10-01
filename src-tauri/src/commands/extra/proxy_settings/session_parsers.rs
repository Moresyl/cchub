#![allow(clippy::too_many_arguments)]
use std::io::{BufRead, BufReader};

use super::super::config_profiles::{
    format_timestamp_text, format_unix_timestamp, preferred_texts_from_value,
};
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
    let lines = crate::shared::session_archive::lines(
        path,
        crate::shared::session_archive::MAX_SESSION_BYTES,
    )
    .map_err(|e| e.to_string())?;
    let mut entries = Vec::new();

    for (index, line) in lines.enumerate() {
        let line = line.map_err(|e| e.to_string())?;
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
    let entries = if session.source_backend == "grokbuild_native" {
        load_grokbuild_session_entries(&source_path)?
    } else if session.source_backend == "mcode_sqlite" {
        load_mcode_session_entries(&source_path, &session.id)?
    } else if session.tool_id == "opencode" && session.source_backend == "opencode_sqlite" {
        load_opencode_session_entries(&source_path, &session.id)?
    } else if session.tool_id == "codex" && session.source_kind == "codex_jsonl" {
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

#[cfg(test)]
pub fn is_valid_session_source_path(
    conn: &rusqlite::Connection,
    tool_id: &str,
    source_path: &str,
) -> bool {
    super::session_tasks::SessionAccessPlan::prepare(conn, tool_id)
        .is_ok_and(|plan| plan.allows(source_path))
}
