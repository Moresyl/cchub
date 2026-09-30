use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

use super::super::config_profiles::{count_query_hits, truncate_session_text};
use super::super::types::{SessionEntry, SessionSummary};

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
    .unwrap_or(false)
}

fn open_readonly(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("Cannot read OpenCode sessions: {error}"))?;
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    Ok(conn)
}

fn timestamp(milliseconds: i64) -> Option<String> {
    chrono::DateTime::from_timestamp_millis(milliseconds).map(|time| {
        time.with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string()
    })
}

fn visible_text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(parts) = value.as_array() {
        return parts
            .iter()
            .map(visible_text)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
    }
    match value.get("type").and_then(Value::as_str) {
        Some("reasoning" | "thinking" | "redacted_thinking") => String::new(),
        Some("tool" | "tool_use") => {
            let name = value
                .get("tool")
                .or_else(|| value.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("tool");
            let output = value
                .pointer("/state/output")
                .or_else(|| value.get("output"))
                .map(visible_text)
                .unwrap_or_default();
            if output.is_empty() {
                format!("[Tool: {name}]")
            } else {
                format!("[Tool: {name}]\n{output}")
            }
        }
        _ => value
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| value.get("content").map(visible_text))
            .unwrap_or_default(),
    }
}

fn is_v2_session(conn: &Connection, id: &str) -> Result<bool, String> {
    if !table_exists(conn, "session_v2") {
        return Ok(false);
    }
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM session_v2 WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

fn load_entries(conn: &Connection, id: &str) -> Result<Vec<SessionEntry>, String> {
    let v2 = is_v2_session(conn, id)?;
    let query = if v2 {
        "SELECT id, type, time_created, data FROM session_message WHERE session_id = ?1 ORDER BY seq, rowid"
    } else {
        "SELECT id, '', time_created, data FROM message WHERE session_id = ?1 ORDER BY time_created, id"
    };
    let mut parts: HashMap<String, Vec<String>> = HashMap::new();
    if !v2 && table_exists(conn, "part") {
        let mut stmt = conn
            .prepare(
                "SELECT message_id, data FROM part WHERE session_id = ?1 ORDER BY time_created, id",
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (message_id, data) = row.map_err(|error| error.to_string())?;
            if let Ok(value) = serde_json::from_str::<Value>(&data) {
                let text = visible_text(&value);
                if !text.trim().is_empty() {
                    parts.entry(message_id).or_default().push(text);
                }
            }
        }
    }
    let mut stmt = conn.prepare(query).map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut entries = Vec::new();
    for row in rows {
        let (message_id, row_type, created, data) = row.map_err(|error| error.to_string())?;
        let Ok(value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let kind = if v2 {
            row_type.as_str()
        } else {
            value
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        };
        if !matches!(kind, "user" | "assistant" | "system" | "tool") {
            continue;
        }
        let content = if v2 {
            visible_text(&value)
        } else {
            parts
                .remove(&message_id)
                .map(|parts| parts.join("\n"))
                .unwrap_or_else(|| visible_text(&value))
        };
        if content.trim().is_empty() {
            continue;
        }
        entries.push(SessionEntry {
            id: message_id,
            kind: kind.to_string(),
            title: kind.to_string(),
            content,
            timestamp: timestamp(created),
        });
    }
    Ok(entries)
}

pub fn load_opencode_session_entries(path: &Path, id: &str) -> Result<Vec<SessionEntry>, String> {
    let mut connection = open_readonly(path)?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    load_entries(&transaction, id)
}

pub fn scan_opencode_sessions(path: &Path, query: &str) -> Result<Vec<SessionSummary>, String> {
    let mut connection = open_readonly(path)?;
    let conn = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let v2 = table_exists(&conn, "session_v2") && table_exists(&conn, "session_message");
    let v1 = table_exists(&conn, "session") && table_exists(&conn, "message");
    if !v1 && !v2 {
        return Ok(Vec::new());
    }

    let migration: Option<i64> = if v2 && table_exists(&conn, "kv") {
        conn.query_row("SELECT MAX(COALESCE(time_created, 0), COALESCE(time_updated, 0)) FROM kv WHERE key = 'migration.v1-v2'", [], |row| row.get(0))
            .optional().map_err(|error| error.to_string())?.filter(|timestamp| *timestamp > 0)
    } else {
        None
    };
    let mut sessions = Vec::new();
    let normalized_query = query.trim().to_lowercase();
    for (table, messages, enabled) in [
        ("session_v2", "session_message", v2),
        ("session", "message", v1),
    ] {
        if !enabled {
            continue;
        }
        let predicate = if table == "session" && v2 {
            // Without a migration marker it is unsafe to resurrect leftover V1 rows.
            if migration.is_none() {
                continue;
            }
            "WHERE s.time_updated > ?1 AND NOT EXISTS (SELECT 1 FROM session_v2 newer WHERE newer.id = s.id)"
        } else {
            "WHERE ?1 IS NOT NULL"
        };
        let sql = format!("SELECT s.id, COALESCE(s.title, ''), COALESCE(s.directory, ''), s.time_created,
            MAX(s.time_updated, COALESCE((SELECT MAX(time_updated) FROM {messages} m WHERE m.session_id = s.id), s.time_updated))
            FROM {table} s {predicate} ORDER BY 5 DESC");
        let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([migration.unwrap_or(0)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (id, mut title, directory, created, updated) =
                row.map_err(|error| error.to_string())?;
            let entries = load_entries(&conn, &id)?;
            if title.trim().is_empty() {
                title = Path::new(&directory)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&id)
                    .to_string();
            }
            let mut search_values = vec![title.clone(), directory.clone()];
            search_values.extend(entries.iter().map(|entry| entry.content.clone()));
            let hits = count_query_hits(&normalized_query, &search_values);
            if !normalized_query.is_empty() && hits == 0 {
                continue;
            }
            let preview = entries
                .iter()
                .find(|entry| {
                    !normalized_query.is_empty()
                        && entry.content.to_lowercase().contains(&normalized_query)
                })
                .or_else(|| entries.iter().find(|entry| entry.kind == "user"))
                .map(|entry| truncate_session_text(&entry.content, 180))
                .unwrap_or_else(|| title.clone());
            sessions.push((
                updated,
                SessionSummary {
                    id,
                    tool_id: "opencode".to_string(),
                    tool_name: "OpenCode".to_string(),
                    title,
                    cwd: if directory.is_empty() {
                        None
                    } else {
                        Some(directory)
                    },
                    source_kind: "opencode_sqlite".to_string(),
                    source_backend: "opencode_sqlite".to_string(),
                    source_path: path.to_string_lossy().to_string(),
                    created_at: timestamp(created),
                    updated_at: timestamp(updated),
                    preview,
                    message_count: entries.len(),
                    input_tokens: None,
                    output_tokens: None,
                    tokens_used: None,
                    search_hit_count: hits,
                    can_resume: true,
                    can_delete: true,
                },
            ));
        }
    }
    sessions.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    Ok(sessions.into_iter().map(|(_, session)| session).collect())
}

pub fn delete_opencode_session(path: &Path, id: &str) -> Result<(), String> {
    // Path authorization happens in delete_session_impl. Never create a missing database.
    let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|error| error.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON")
        .map_err(|error| error.to_string())?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let mut deleted = 0;
    for (table, key) in [
        ("session_pending", "session_id"),
        ("session_inbox", "session_id"),
        ("instruction_entry", "session_id"),
        ("instruction_state", "session_id"),
        ("event_sequence", "aggregate_id"),
        ("session_message", "session_id"),
        ("part", "session_id"),
        ("message", "session_id"),
        ("session_v2", "id"),
        ("session", "id"),
    ] {
        if !table_exists(&tx, table) {
            continue;
        }
        let count = tx
            .execute(&format!("DELETE FROM {table} WHERE {key} = ?1"), [id])
            .map_err(|error| {
                format!("Cannot remove OpenCode session; no changes saved: {error}")
            })?;
        if matches!(table, "session" | "session_v2") {
            deleted += count;
        }
    }
    if deleted == 0 {
        return Err("OpenCode session no longer exists; refresh the session list".to_string());
    }
    tx.commit().map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "opencode_sessions_tests.rs"]
mod tests;
