use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

use super::super::config_profiles::*;
use super::super::types::*;

const MAX_GROK_SCAN_DEPTH: usize = 8;

fn json_timestamp(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) => format_timestamp_text(value),
        Some(Value::Number(value)) => value.as_i64().and_then(format_unix_timestamp),
        _ => None,
    }
}

fn message_text(value: &Value) -> String {
    let mut texts = Vec::new();
    preferred_texts_from_value(value, &mut texts, 0);
    texts.join("\n\n")
}

fn collect_grok_summaries(root: &Path, depth: usize, output: &mut Vec<PathBuf>) {
    if depth > MAX_GROK_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_grok_summaries(&path, depth + 1, output);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("summary.json") {
            output.push(path);
        }
    }
}

fn read_grok_summary(path: &Path) -> Result<Value, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read Grok Build session summary: {error}"))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Failed to parse Grok Build session summary: {error}"))
}

fn count_grok_messages(summary_path: &Path) -> usize {
    let Some(parent) = summary_path.parent() else {
        return 0;
    };
    let Ok(file) = File::open(parent.join("chat_history.jsonl")) else {
        return 0;
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .filter(|value| {
            matches!(
                value.get("type").and_then(Value::as_str),
                Some("system" | "user" | "assistant" | "tool")
            ) && !message_text(value.get("content").unwrap_or(&Value::Null))
                .trim()
                .is_empty()
        })
        .count()
}

fn grok_summary_to_session(path: &Path, query: &str) -> Option<SessionSummary> {
    let summary = read_grok_summary(path).ok()?;
    let info = summary.get("info")?;
    let id = info.get("id")?.as_str()?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let cwd = info
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let session_summary = summary
        .get("session_summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let title = summary
        .get("generated_title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(session_summary.as_deref())
        .map(|value| truncate_session_text(value, 80))
        .unwrap_or_else(|| id.clone());
    let preview = session_summary
        .as_deref()
        .map(|value| truncate_session_text(value, 180))
        .unwrap_or_else(|| title.clone());
    let search_hit_count = count_query_hits(
        query,
        &[
            id.clone(),
            title.clone(),
            preview.clone(),
            cwd.clone().unwrap_or_default(),
        ],
    );
    if !query.is_empty() && search_hit_count == 0 {
        return None;
    }

    Some(SessionSummary {
        id,
        tool_id: "grokbuild".to_string(),
        tool_name: tool_label("grokbuild").to_string(),
        title,
        cwd,
        source_kind: "grokbuild_summary".to_string(),
        source_backend: "grokbuild_native".to_string(),
        source_path: path.to_string_lossy().to_string(),
        created_at: json_timestamp(summary.get("created_at")),
        updated_at: json_timestamp(
            summary
                .get("last_active_at")
                .or_else(|| summary.get("updated_at")),
        ),
        preview,
        message_count: count_grok_messages(path),
        input_tokens: None,
        output_tokens: None,
        tokens_used: None,
        search_hit_count,
        can_resume: tool_supports_session_resume("grokbuild"),
        can_delete: true,
    })
}

pub fn scan_grokbuild_sessions(root: &Path, query: &str) -> Vec<SessionSummary> {
    let mut summaries = Vec::new();
    for directory in [root.join("sessions"), root.join("archived_sessions")] {
        collect_grok_summaries(&directory, 0, &mut summaries);
    }
    summaries
        .iter()
        .filter_map(|path| grok_summary_to_session(path, query))
        .collect()
}

pub fn load_grokbuild_session_entries(summary_path: &Path) -> Result<Vec<SessionEntry>, String> {
    let session_dir = summary_path
        .parent()
        .ok_or_else(|| "Invalid Grok Build session path".to_string())?;
    let chat_path = session_dir.join("chat_history.jsonl");
    let file = File::open(&chat_path)
        .map_err(|error| format!("Failed to open Grok Build chat history: {error}"))?;
    let mut entries = Vec::new();
    for (index, line) in BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(kind) = value.get("type").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(kind, "system" | "user" | "assistant" | "tool") {
            continue;
        }
        let content = message_text(value.get("content").unwrap_or(&Value::Null));
        if content.trim().is_empty() {
            continue;
        }
        entries.push(SessionEntry {
            id: format!("grok-entry-{index}"),
            kind: kind.to_string(),
            title: kind.replace('_', " "),
            content,
            timestamp: json_timestamp(value.get("timestamp").or_else(|| value.get("ts"))),
        });
    }
    Ok(entries)
}

pub fn delete_grokbuild_session(
    root: &Path,
    summary_path: &Path,
    session_id: &str,
) -> Result<(), String> {
    let normalized_root = root.canonicalize().map_err(|error| error.to_string())?;
    let normalized_source = summary_path
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !normalized_source.starts_with(&normalized_root)
        || normalized_source.file_name().and_then(|name| name.to_str()) != Some("summary.json")
    {
        return Err("Invalid Grok Build session source".to_string());
    }
    let summary = read_grok_summary(&normalized_source)?;
    let actual_id = summary
        .pointer("/info/id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if actual_id != session_id {
        return Err("Grok Build session ID does not match its summary".to_string());
    }
    let session_dir = normalized_source
        .parent()
        .ok_or_else(|| "Invalid Grok Build session directory".to_string())?;
    if session_dir == normalized_root
        || session_dir.file_name().and_then(|name| name.to_str()) != Some(session_id)
    {
        return Err("Refusing to delete an unexpected Grok Build directory".to_string());
    }
    std::fs::remove_dir_all(session_dir)
        .map_err(|error| format!("Failed to delete Grok Build session: {error}"))
}

fn has_table(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn mcode_entries(conn: &Connection, session_id: &str) -> Result<Vec<SessionEntry>, String> {
    let migrated = has_table(conn, "local_runtime_message_row_migrations")
        && conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM local_runtime_message_row_migrations WHERE session_id = ?1)",
                [session_id],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
    if migrated && has_table(conn, "local_runtime_message_rows") {
        let mut stmt = conn
            .prepare(
                "SELECT role, data_json, created_at_ms FROM local_runtime_message_rows
                 WHERE session_id = ?1 AND role IN ('user', 'assistant') ORDER BY id",
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([session_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut entries = Vec::new();
        for (index, row) in rows.flatten().enumerate() {
            let (role, data, timestamp) = row;
            let value = serde_json::from_str::<Value>(&data).unwrap_or_default();
            let content = message_text(value.get("msg_content").unwrap_or(&Value::Null));
            if content.trim().is_empty() {
                continue;
            }
            entries.push(SessionEntry {
                id: format!("mcode-entry-{index}"),
                kind: role.clone(),
                title: role,
                content,
                timestamp: timestamp.and_then(format_unix_timestamp),
            });
        }
        return Ok(entries);
    }

    if !has_table(conn, "local_runtime_messages") {
        return Ok(Vec::new());
    }
    let payload = conn
        .query_row(
            "SELECT display_messages_json FROM local_runtime_messages WHERE session_id = ?1",
            [session_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let messages = payload
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    Ok(messages
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let role = value.get("role")?.as_str()?;
            if !matches!(role, "user" | "assistant") {
                return None;
            }
            let content = message_text(value.get("msg_content").unwrap_or(&Value::Null));
            (!content.trim().is_empty()).then(|| SessionEntry {
                id: format!("mcode-entry-{index}"),
                kind: role.to_string(),
                title: role.to_string(),
                content,
                timestamp: json_timestamp(
                    value.get("timestamp").or_else(|| value.get("created_at")),
                ),
            })
        })
        .collect())
}

pub fn scan_mcode_sessions(database_path: &Path, query: &str) -> Vec<SessionSummary> {
    let Ok(conn) = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT session_id, title, workspace_dir, created_at_ms, updated_at_ms
         FROM local_runtime_sessions WHERE visibility <> 'hidden' AND archived = 0
         AND parent_session_id IS NULL AND session_kind NOT IN ('peek', 'channel', 'cron')
         ORDER BY updated_at_ms DESC",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<i64>>(4)?,
        ))
    }) else {
        return Vec::new();
    };
    rows.flatten()
        .filter_map(|(id, title, cwd, created_at, updated_at)| {
            let entries = mcode_entries(&conn, &id).unwrap_or_default();
            let preview = entries
                .last()
                .map(|entry| truncate_session_text(&entry.content, 180))
                .or_else(|| title.clone())
                .unwrap_or_else(|| id.clone());
            let title = title
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| truncate_session_text(&preview, 80));
            let search_hit_count = count_query_hits(
                query,
                &[
                    id.clone(),
                    title.clone(),
                    preview.clone(),
                    cwd.clone().unwrap_or_default(),
                ],
            );
            if !query.is_empty() && search_hit_count == 0 {
                return None;
            }
            Some(SessionSummary {
                id,
                tool_id: "mcode".to_string(),
                tool_name: tool_label("mcode").to_string(),
                title,
                cwd,
                source_kind: "mcode_runtime".to_string(),
                source_backend: "mcode_sqlite".to_string(),
                source_path: database_path.to_string_lossy().to_string(),
                created_at: created_at.and_then(format_unix_timestamp),
                updated_at: updated_at.and_then(format_unix_timestamp),
                preview,
                message_count: entries.len(),
                input_tokens: None,
                output_tokens: None,
                tokens_used: None,
                search_hit_count,
                can_resume: tool_supports_session_resume("mcode"),
                can_delete: false,
            })
        })
        .collect()
}

pub fn load_mcode_session_entries(
    database_path: &Path,
    session_id: &str,
) -> Result<Vec<SessionEntry>, String> {
    let conn = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
    mcode_entries(&conn, session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_loads_and_deletes_native_grokbuild_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".grok");
        let session_id = "session-1";
        let session_dir = root.join("sessions/project").join(session_id);
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("summary.json"),
            r#"{"info":{"id":"session-1","cwd":"C:/work"},"generated_title":"Fix tests","session_summary":"Repair the failing suite","created_at":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        std::fs::write(
            session_dir.join("chat_history.jsonl"),
            "{\"type\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"hello\"}]}\n{\"type\":\"reasoning\",\"content\":\"private\"}\n{\"type\":\"assistant\",\"content\":\"done\"}\n",
        )
        .unwrap();

        let sessions = scan_grokbuild_sessions(&root, "repair");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].message_count, 2);
        let entries = load_grokbuild_session_entries(Path::new(&sessions[0].source_path)).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].content, "done");

        delete_grokbuild_session(&root, Path::new(&sessions[0].source_path), session_id).unwrap();
        assert!(!session_dir.exists());
    }

    #[test]
    fn scans_mcode_visible_roots_and_prefers_migrated_messages() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("runtime-state.sqlite");
        let conn = Connection::open(&database).unwrap();
        conn.execute_batch(
            r#"CREATE TABLE local_runtime_sessions (
                session_id TEXT, title TEXT, workspace_dir TEXT, created_at_ms INTEGER,
                updated_at_ms INTEGER, visibility TEXT, parent_session_id TEXT,
                session_kind TEXT, archived INTEGER);
             INSERT INTO local_runtime_sessions VALUES
                ('visible','Project','C:/work',1000,2000,'visible',NULL,'conversation',0),
                ('hidden','Hidden','C:/work',1000,2000,'hidden',NULL,'conversation',0);
             CREATE TABLE local_runtime_message_row_migrations (session_id TEXT);
             INSERT INTO local_runtime_message_row_migrations VALUES ('visible');
             CREATE TABLE local_runtime_message_rows (
                id INTEGER, session_id TEXT, role TEXT, data_json TEXT, created_at_ms INTEGER);
             INSERT INTO local_runtime_message_rows VALUES
                (1,'visible','user','{"msg_content":"Question"}',1000),
                (2,'visible','assistant','{"msg_content":"Answer"}',2000);
             CREATE TABLE local_runtime_messages (session_id TEXT, display_messages_json TEXT);"#,
        )
        .unwrap();
        drop(conn);

        let sessions = scan_mcode_sessions(&database, "answer");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "visible");
        assert_eq!(sessions[0].message_count, 2);
        assert!(!sessions[0].can_delete);
        let entries = load_mcode_session_entries(&database, "visible").unwrap();
        assert_eq!(entries[1].content, "Answer");
    }
}
