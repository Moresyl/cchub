#![allow(clippy::too_many_arguments)]
use base64::Engine;
use std::path::PathBuf;

use super::super::config_profiles::*;
use super::super::types::*;
use super::*;

const SQL_BACKUP_BATCH_ROWS: usize = 200;
const SQL_BACKUP_BATCH_BYTES: usize = 1024 * 1024;
pub(super) const TOOL_BACKUP_IDS: &[&str] = &[
    "claude",
    "codex",
    "gemini",
    "grokbuild",
    "opencode",
    "openclaw",
    "hermes",
    "pi",
    "mcode",
];
pub(super) const CLAUDE_DESKTOP_BACKUP_KEYS: &[&str] = &[
    "normal-config",
    "threep-config",
    "managed-profile",
    "profile-meta",
];

const BACKUP_DATA_TABLES: &[&str] = &[
    "mcp_servers",
    "plugins",
    "skills",
    "hooks",
    "activity_logs",
    "mcp_clients",
    "workspaces",
    "custom_paths",
    "config_profiles",
    "project_profiles",
    "app_settings",
    "prompt_library",
    "imported_project_files",
    "proxy_request_logs",
    "session_usage_dedup",
    "model_pricing",
    "proxy_usage_daily_rollups",
    "update_history",
    "metrics",
];

fn append_insert_batches(sql: &mut String, prefix: &str, rows: &[String]) {
    let mut batch: Vec<&str> = Vec::with_capacity(SQL_BACKUP_BATCH_ROWS);
    let mut batch_bytes = prefix.len() + 2;

    for row in rows {
        let row_bytes = row.len() + 2;
        if !batch.is_empty()
            && (batch.len() >= SQL_BACKUP_BATCH_ROWS
                || batch_bytes.saturating_add(row_bytes) > SQL_BACKUP_BATCH_BYTES)
        {
            sql.push_str(prefix);
            sql.push_str(&batch.join(", "));
            sql.push_str(";\n");
            batch.clear();
            batch_bytes = prefix.len() + 2;
        }

        batch.push(row);
        batch_bytes = batch_bytes.saturating_add(row_bytes);
    }

    if !batch.is_empty() {
        sql.push_str(prefix);
        sql.push_str(&batch.join(", "));
        sql.push_str(";\n");
    }
}

pub(super) fn count_backup_rows(conn: &rusqlite::Connection) -> Result<usize, String> {
    BACKUP_DATA_TABLES
        .iter()
        .chain(
            [
                "_backup_meta",
                "_tool_configs",
                "_skill_files",
                "_backup_files",
            ]
            .iter(),
        )
        .try_fold(0usize, |total, table| {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .map_err(|error| error.to_string())?;
            Ok(total.saturating_add(count.max(0) as usize))
        })
}

pub(crate) fn append_backup_database_rows(conn: &rusqlite::Connection, sql: &mut String) {
    for table in BACKUP_DATA_TABLES {
        let query = format!("SELECT * FROM {}", table);
        if let Ok(mut stmt) = conn.prepare(&query) {
            let col_count = stmt.column_count();
            let col_names: Vec<String> = (0..col_count)
                .map(|i| stmt.column_name(i).unwrap_or("").to_string())
                .collect();

            let mut rows_to_insert = Vec::new();
            if let Ok(rows) = stmt.query_map([], |row| {
                let mut vals = Vec::new();
                for i in 0..col_count {
                    let val: rusqlite::Result<String> = row.get(i);
                    match val {
                        Ok(s) => vals.push(format!("'{}'", sql_escape(&s))),
                        Err(_) => {
                            let int_val: rusqlite::Result<i64> = row.get(i);
                            match int_val {
                                Ok(n) => vals.push(n.to_string()),
                                Err(_) => {
                                    let float_val: rusqlite::Result<f64> = row.get(i);
                                    match float_val {
                                        Ok(f) => vals.push(f.to_string()),
                                        Err(_) => vals.push("NULL".to_string()),
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(vals)
            }) {
                for row in rows.flatten() {
                    rows_to_insert.push(format!("({})", row.join(", ")));
                }
            }
            if !rows_to_insert.is_empty() {
                sql.push_str(&format!("-- Table: {}\n", table));
                append_insert_batches(
                    sql,
                    &format!(
                        "INSERT OR REPLACE INTO {} ({}) VALUES ",
                        table,
                        col_names.join(", ")
                    ),
                    &rows_to_insert,
                );
                sql.push('\n');
            }
        }
    }
}

/// Generate complete .sql backup content
pub(crate) fn generate_sql_backup(conn: &rusqlite::Connection, home: &std::path::Path) -> String {
    let mut sql = String::new();

    // Header
    sql.push_str("-- ═══════════════════════════════════════════════════════\n");
    sql.push_str("-- CCHub Database Backup (.sql)\n");
    sql.push_str(&format!("-- Version: {}\n", env!("CARGO_PKG_VERSION")));
    sql.push_str(&format!(
        "-- Created: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    sql.push_str("-- ═══════════════════════════════════════════════════════\n\n");

    // Schema (CREATE TABLE IF NOT EXISTS)
    sql.push_str("-- ── Schema ──\n\n");
    sql.push_str(&crate::db::schema::get_schema_sql());
    sql.push('\n');

    // Trusted artifact tables shared by export and import.
    sql.push_str(super::backup_sql::BACKUP_TABLE_SCHEMA);
    sql.push_str(&format!(
        "INSERT OR REPLACE INTO _backup_meta VALUES ('version', '{}');\n",
        env!("CARGO_PKG_VERSION")
    ));
    sql.push_str(&format!(
        "INSERT OR REPLACE INTO _backup_meta VALUES ('created_at', '{}');\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));

    // Data dump for all business tables
    sql.push_str("-- ── Data ──\n\n");
    append_backup_database_rows(conn, &mut sql);

    // Tool config files
    sql.push_str("-- ── Tool Configs ──\n\n");
    for &tool_id in TOOL_BACKUP_IDS {
        if let Ok(content) = read_tool_snapshot(conn, tool_id) {
            let config_path = match tool_id {
                "claude" => resolve_claude_paths(conn)
                    .map(|(claude_json, settings_json)| {
                        format!("{} | {}", claude_json.display(), settings_json.display())
                    })
                    .unwrap_or_else(|_| {
                        home.join(".claude")
                            .join("settings.json")
                            .display()
                            .to_string()
                    }),
                _ => resolve_tool_config_path(conn, tool_id)
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| home.join(format!(".{}", tool_id)).display().to_string()),
            };

            let row = format!(
                "('{}', '{}', '{}')",
                tool_id,
                sql_escape(&config_path),
                sql_escape(&content)
            );
            append_insert_batches(
                &mut sql,
                "INSERT OR REPLACE INTO _tool_configs VALUES ",
                &[row],
            );
        }
    }
    sql.push('\n');

    // Skill files
    sql.push_str("-- ── Skill Files ──\n\n");
    for &tool_id in TOOL_BACKUP_IDS {
        let skills_dir = match resolve_tool_skills_dir(conn, tool_id) {
            Ok(path) => path,
            Err(_) => continue,
        };
        let mut rows_to_insert = Vec::new();
        if skills_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&skills_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            let name = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            rows_to_insert.push(format!(
                                "('{}', '{}', '{}')",
                                tool_id,
                                sql_escape(&name),
                                sql_escape(&content)
                            ));
                        }
                    }
                }
            }
        }
        append_insert_batches(
            &mut sql,
            "INSERT INTO _skill_files (tool_id, name, content) VALUES ",
            &rows_to_insert,
        );
    }

    // Full file backup for tool directories and standalone config files
    sql.push_str("-- ── Full File Backup ──\n\n");
    let mut backup_roots: Vec<(String, PathBuf)> = Vec::new();
    for &tool_id in TOOL_BACKUP_IDS {
        if let Ok(tool_dir) = resolve_tool_config_dir(conn, tool_id) {
            backup_roots.push((format!("tooldir:{}", tool_id), tool_dir.clone()));

            if let Ok(skills_dir) = resolve_tool_skills_dir(conn, tool_id) {
                if !path_is_within(&skills_dir, &tool_dir) {
                    backup_roots.push((format!("skillsdir:{}", tool_id), skills_dir));
                }
            }

            if tool_id == "claude" {
                if let Ok((claude_mcp, _)) = resolve_claude_paths(conn) {
                    if !path_is_within(&claude_mcp, &tool_dir) {
                        backup_roots.push(("claude_mcp".to_string(), claude_mcp));
                    }
                }
            }
        }
    }
    for key in CLAUDE_DESKTOP_BACKUP_KEYS {
        if let Ok(path) = crate::commands::claude_desktop_profiles::backup_path(key) {
            backup_roots.push((format!("claude-desktop:{key}"), path));
        }
    }

    let mut backup_file_rows = Vec::new();
    for (root_key, root_path) in &backup_roots {
        if root_path.is_dir() {
            collect_backup_file_rows(
                root_path,
                root_key,
                std::path::Path::new(""),
                &mut backup_file_rows,
            );
        } else if root_path.is_file() {
            if let Ok(bytes) = std::fs::read(root_path) {
                let content_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                backup_file_rows.push((root_key.clone(), String::new(), content_base64));
            }
        }
    }

    // Project-level tool files so workspace/project-scoped settings migrate too.
    let project_relative_files = [
        "CLAUDE.md",
        "CLAUDE.md.bak",
        "AGENTS.md",
        "AGENTS.md.bak",
        "GEMINI.md",
        "GEMINI.md.bak",
        ".claude.json",
    ];
    let project_relative_dirs = [
        ".claude",
        ".codex",
        ".gemini",
        ".opencode",
        ".openclaw",
        ".hermes",
    ];

    for project_root in discover_project_roots(conn) {
        let root_key = format!("project:{}", project_root.to_string_lossy());

        for relative_file in project_relative_files {
            let relative_path = std::path::Path::new(relative_file);
            let absolute_path = project_root.join(relative_path);
            collect_backup_entry_row(
                &absolute_path,
                &root_key,
                relative_path,
                &mut backup_file_rows,
            );
        }

        for relative_dir in project_relative_dirs {
            let relative_path = std::path::Path::new(relative_dir);
            let absolute_path = project_root.join(relative_path);
            if absolute_path.is_dir() {
                collect_backup_file_rows(
                    &absolute_path,
                    &root_key,
                    relative_path,
                    &mut backup_file_rows,
                );
            }
        }
    }

    let backup_rows = backup_file_rows
        .into_iter()
        .map(|(root_key, relative_path, content_base64)| {
            format!(
                "('{}', '{}', '{}')",
                sql_escape(&root_key),
                sql_escape(&relative_path),
                sql_escape(&content_base64),
            )
        })
        .collect::<Vec<_>>();
    append_insert_batches(
        &mut sql,
        "INSERT INTO _backup_files (root_key, relative_path, content_base64) VALUES ",
        &backup_rows,
    );

    sql.push_str("\n-- ── End of Backup ──\n");
    sql
}

pub fn managed_backups_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(home.join(".cchub").join("backups"))
}

pub fn ensure_managed_backups_dir() -> Result<PathBuf, String> {
    let dir = managed_backups_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn sanitize_backup_file_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => ch,
        })
        .collect::<String>()
}

pub fn infer_backup_kind(name: &str) -> String {
    if name.contains("auto") {
        "scheduled".to_string()
    } else {
        "manual".to_string()
    }
}

pub fn map_backup_entry(path: &std::path::Path) -> Result<ManagedBackupFile, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let modified = metadata
        .modified()
        .unwrap_or_else(|_| std::time::SystemTime::now());
    let modified_at: chrono::DateTime<chrono::Local> = modified.into();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("Invalid backup file name: {}", path.display()))?
        .to_string();

    Ok(ManagedBackupFile {
        path: path.to_string_lossy().to_string(),
        name: name.clone(),
        created_at: modified_at.to_rfc3339(),
        size_bytes: metadata.len(),
        kind: infer_backup_kind(&name),
        can_restore: path.extension().and_then(|value| value.to_str()) == Some("sql"),
    })
}

pub fn list_managed_backups_from_dir(
    dir: &std::path::Path,
) -> Result<Vec<ManagedBackupFile>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("sql") {
            continue;
        }
        items.push(map_backup_entry(&path)?);
    }

    items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(items)
}

pub fn prune_managed_backups(dir: &std::path::Path, retention_count: usize) -> Result<(), String> {
    let retention_count = retention_count.max(1);
    let backups = list_managed_backups_from_dir(dir)?;
    for backup in backups.into_iter().skip(retention_count) {
        let _ = std::fs::remove_file(&backup.path);
    }
    Ok(())
}

pub fn create_managed_backup_from_conn(
    conn: &rusqlite::Connection,
    kind: &str,
    retention_count: usize,
) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let backup_dir = ensure_managed_backups_dir()?;
    let prefix = if kind == "scheduled" {
        "cchub-auto-backup"
    } else {
        "cchub-backup"
    };
    let file_path = backup_dir.join(format!(
        "{prefix}-{}.sql",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let sql_content = generate_sql_backup(conn, &home);
    std::fs::write(&file_path, sql_content).map_err(|e| e.to_string())?;
    prune_managed_backups(&backup_dir, retention_count)?;
    Ok(file_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_batches_split_by_row_count() {
        let rows = (0..(SQL_BACKUP_BATCH_ROWS * 2 + 1))
            .map(|value| format!("({value})"))
            .collect::<Vec<_>>();
        let mut output = String::new();

        append_insert_batches(&mut output, "INSERT INTO sample VALUES ", &rows);

        assert_eq!(output.matches("INSERT INTO sample VALUES").count(), 3);
        assert!(output.ends_with(";\n"));
    }

    #[test]
    fn insert_batches_split_by_sql_size() {
        let row = format!("('{}')", "x".repeat(SQL_BACKUP_BATCH_BYTES * 2 / 3));
        let rows = vec![row.clone(), row.clone(), row, "('tail')".to_string()];
        let mut output = String::new();

        append_insert_batches(&mut output, "INSERT INTO sample VALUES ", &rows);

        assert_eq!(output.matches("INSERT INTO sample VALUES").count(), 3);
        assert!(output.contains("('tail')"));
    }

    #[test]
    fn backup_data_tables_cover_all_persisted_domains() {
        for table in [
            "prompt_library",
            "proxy_request_logs",
            "model_pricing",
            "proxy_usage_daily_rollups",
        ] {
            assert!(BACKUP_DATA_TABLES.contains(&table));
        }
        assert!(TOOL_BACKUP_IDS.contains(&"grokbuild"));
        assert!(TOOL_BACKUP_IDS.contains(&"mcode"));
        assert_eq!(CLAUDE_DESKTOP_BACKUP_KEYS.len(), 4);
    }
}
