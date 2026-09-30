//! Shared layout policy for transcript browsing and native usage accounting.
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;

pub(crate) struct Layout {
    pub sessions: &'static str,
    pub messages: &'static str,
    pub predicate: &'static str,
    pub migration: i64,
    pub v2: bool,
}

pub(crate) fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn open_readonly(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("Cannot read OpenCode database: {error}"))?;
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    Ok(conn)
}

pub(crate) fn layouts(conn: &Connection) -> Result<Vec<Layout>, String> {
    let v2 = table_exists(conn, "session_v2")? && table_exists(conn, "session_message")?;
    let v1 = table_exists(conn, "session")? && table_exists(conn, "message")?;
    let migration: Option<i64> = if v2 && table_exists(conn, "kv")? {
        conn.query_row(
            "SELECT MAX(COALESCE(time_created, 0), COALESCE(time_updated, 0)) FROM kv WHERE key = 'migration.v1-v2'",
            [], |row| row.get(0),
        ).optional().map_err(|error| error.to_string())?.filter(|time| *time > 0)
    } else {
        None
    };
    let mut result = Vec::new();
    if v2 {
        result.push(Layout {
            sessions: "session_v2",
            messages: "session_message",
            v2: true,
            migration: 0,
            predicate: "?1 IS NOT NULL",
        });
    }
    // Frozen pre-migration rows can contain sessions deleted in V2. Only include
    // independent V1 sessions modified after a known migration boundary.
    if v1 && (!v2 || migration.is_some()) {
        result.push(Layout {
            sessions: "session", messages: "message", v2: false,
            migration: migration.unwrap_or(0),
            predicate: if v2 {
                "s.time_updated > ?1 AND NOT EXISTS (SELECT 1 FROM session_v2 newer WHERE newer.id = s.id)"
            } else { "?1 IS NOT NULL" },
        });
    }
    Ok(result)
}
