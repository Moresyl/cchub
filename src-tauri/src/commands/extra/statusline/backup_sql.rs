use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::Connection;

pub(super) const BACKUP_TABLE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS _backup_meta (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS _tool_configs (tool_id TEXT PRIMARY KEY, config_path TEXT, config_content TEXT);
CREATE TABLE IF NOT EXISTS _skill_files (id INTEGER PRIMARY KEY AUTOINCREMENT, tool_id TEXT, name TEXT, content TEXT);
CREATE TABLE IF NOT EXISTS _backup_files (id INTEGER PRIMARY KEY AUTOINCREMENT, root_key TEXT, relative_path TEXT, content_base64 TEXT);
";

const INVALID_SQL: &str = "备份 SQL 包含不支持的指令或无效数据，恢复已停止";
const IMPORT_TIME_LIMIT: Duration = Duration::from_secs(30);

/// Load data into the trusted current schema. Backup-provided schema declarations
/// may only refer to objects already installed by CCHub; they cannot replace them.
pub(super) fn load_backup_sql(conn: &Connection, content: &str) -> Result<(), String> {
    load_backup_sql_with_limit(conn, content, IMPORT_TIME_LIMIT)
}

fn load_backup_sql_with_limit(
    conn: &Connection,
    content: &str,
    time_limit: Duration,
) -> Result<(), String> {
    let content = super::diagnostics::validate_sql_backup_content(content)?;
    conn.execute_batch(BACKUP_TABLE_SCHEMA)
        .map_err(|_| INVALID_SQL.to_string())?;
    let mut tables = HashSet::new();
    let mut indexes = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT type, name, tbl_name FROM sqlite_master")
            .map_err(|_| INVALID_SQL.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|_| INVALID_SQL.to_string())?;
        for row in rows {
            let (kind, name, table) = row.map_err(|_| INVALID_SQL.to_string())?;
            match kind.as_str() {
                "table" => {
                    tables.insert(name);
                }
                "index" => {
                    indexes.insert(name, table);
                }
                _ => return Err(INVALID_SQL.into()),
            }
        }
    }
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|_| INVALID_SQL.to_string())?;
    conn.authorizer(Some(move |context: AuthContext<'_>| {
        if context.accessor.is_some() || context.database_name.is_some_and(|name| name != "main") {
            return Authorization::Deny;
        }
        let allowed = match context.action {
            AuthAction::CreateTable { table_name } => tables.contains(table_name),
            AuthAction::CreateIndex {
                index_name,
                table_name,
            } => indexes
                .get(index_name)
                .is_some_and(|table| table == table_name),
            AuthAction::Insert { table_name }
            | AuthAction::Update { table_name, .. }
            | AuthAction::Delete { table_name }
            | AuthAction::Read { table_name, .. } => {
                tables.contains(table_name)
                    || matches!(table_name, "sqlite_master" | "sqlite_schema")
            }
            AuthAction::Select => true,
            // In particular: no ATTACH/VACUUM INTO, PRAGMA, transaction control,
            // triggers/views/virtual tables, schema replacement or SQL functions.
            _ => false,
        };
        if allowed {
            Authorization::Allow
        } else {
            Authorization::Deny
        }
    }));
    let start = Instant::now();
    conn.progress_handler(1000, Some(move || start.elapsed() >= time_limit));
    let result = conn.execute_batch(content);
    conn.progress_handler(0, None::<fn() -> bool>);
    conn.authorizer(None::<fn(AuthContext<'_>) -> Authorization>);
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK;");
        // SQLite errors can echo supplied SQL, including backup credentials.
        return Err(INVALID_SQL.into());
    }
    let valid_meta = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM _backup_meta WHERE key = 'version' AND length(trim(value)) > 0)",
        [], |row| row.get::<_, bool>(0),
    ).unwrap_or(false);
    if !valid_meta {
        let _ = conn.execute_batch("ROLLBACK;");
        return Err("备份缺少有效版本信息，恢复已停止".into());
    }
    conn.execute_batch("COMMIT;").map_err(|_| {
        let _ = conn.execute_batch("ROLLBACK;");
        INVALID_SQL.to_string()
    })
}

#[cfg(test)]
mod tests;
