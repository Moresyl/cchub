#![allow(clippy::too_many_arguments)]
use base64::Engine;
use std::path::PathBuf;
use tauri::State;

use crate::db::DbState;

use super::super::config_profiles::*;
use super::super::types::*;
use super::*;

pub fn restore_imported_artifacts(
    conn: &rusqlite::Connection,
    restored_count: usize,
) -> Result<(usize, usize, usize, usize, usize), String> {
    let temp_backup_rows = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM _backup_meta) +
                (SELECT COUNT(*) FROM _tool_configs) +
                (SELECT COUNT(*) FROM _skill_files) +
                (SELECT COUNT(*) FROM _backup_files)",
            [],
            |row| row.get::<_, usize>(0),
        )
        .unwrap_or(0);

    let mut tool_configs_restored = 0;
    let mut skills_restored = 0;
    let mut full_files_restored = 0;
    let mut pending_project_files = 0;

    if let Ok(mut stmt) =
        conn.prepare("SELECT tool_id, config_path, config_content FROM _tool_configs")
    {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }) {
            for row in rows.flatten() {
                let (tool_id, _config_path, config_content) = row;
                let restored = match tool_id.as_str() {
                    "claude-settings" => {
                        let (_, settings_json_path) = resolve_claude_paths(conn)?;
                        if let Some(parent) = settings_json_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        crate::utils::atomic_write_string(&settings_json_path, &config_content)
                            .is_ok()
                    }
                    "claude" => {
                        let parsed =
                            serde_json::from_str::<serde_json::Value>(&config_content).ok();
                        let is_snapshot = parsed
                            .as_ref()
                            .and_then(|value| value.as_object())
                            .is_some_and(|obj| {
                                obj.contains_key("__claude_json_keys__")
                                    || obj.contains_key("__settings_json_keys__")
                            });

                        if is_snapshot {
                            apply_tool_snapshot(conn, "claude", &config_content).is_ok()
                        } else {
                            let (claude_json_path, _) = resolve_claude_paths(conn)?;
                            if let Some(parent) = claude_json_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            crate::utils::atomic_write_string(&claude_json_path, &config_content)
                                .is_ok()
                        }
                    }
                    "codex" | "gemini" | "opencode" | "openclaw" | "hermes" => {
                        apply_tool_snapshot(conn, &tool_id, &config_content).is_ok()
                    }
                    _ => false,
                };
                if restored {
                    tool_configs_restored += 1;
                }
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare("SELECT tool_id, name, content FROM _skill_files") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }) {
            for row in rows.flatten() {
                let (tool_id, name, file_content) = row;
                let normalized_tool_id = match tool_id.as_str() {
                    "claude-settings" => "claude",
                    "claude" => "claude",
                    "codex" => "codex",
                    "gemini" => "gemini",
                    "opencode" => "opencode",
                    "openclaw" => "openclaw",
                    _ => continue,
                };
                let skills_dir = match resolve_tool_skills_dir(conn, normalized_tool_id) {
                    Ok(path) => path,
                    Err(_) => continue,
                };
                let _ = std::fs::create_dir_all(&skills_dir);
                if crate::utils::atomic_write_string(&skills_dir.join(&name), &file_content).is_ok()
                {
                    skills_restored += 1;
                }
            }
        }
    }

    if let Ok(mut stmt) = conn
        .prepare("SELECT root_key, relative_path, content_base64 FROM _backup_files ORDER BY id")
    {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }) {
            for row in rows.flatten() {
                let (root_key, relative_path, content_base64) = row;
                if let Some(project_root) = root_key.strip_prefix("project:") {
                    store_imported_project_file(
                        conn,
                        project_root,
                        &relative_path,
                        &content_base64,
                    )?;
                    if !PathBuf::from(project_root).exists() {
                        pending_project_files += 1;
                        continue;
                    }
                }

                let root_path = match resolve_backup_root(conn, &root_key) {
                    Ok(path) => path,
                    Err(_) => continue,
                };
                let target_path = if relative_path.is_empty() {
                    root_path
                } else {
                    root_path.join(relative_path.replace('/', std::path::MAIN_SEPARATOR_STR))
                };
                let bytes = match base64::engine::general_purpose::STANDARD.decode(content_base64) {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                if let Some(parent) = target_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::write(&target_path, bytes).is_ok() {
                    full_files_restored += 1;
                }
            }
        }
    }

    let _ = conn.execute_batch("DROP TABLE IF EXISTS _backup_meta;");
    let _ = conn.execute_batch("DROP TABLE IF EXISTS _tool_configs;");
    let _ = conn.execute_batch("DROP TABLE IF EXISTS _skill_files;");
    let _ = conn.execute_batch("DROP TABLE IF EXISTS _backup_files;");

    let db_rows_restored = restored_count.saturating_sub(temp_backup_rows);
    Ok((
        db_rows_restored,
        tool_configs_restored,
        skills_restored,
        full_files_restored,
        pending_project_files,
    ))
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

    // Backup metadata table
    sql.push_str("CREATE TABLE IF NOT EXISTS _backup_meta (key TEXT PRIMARY KEY, value TEXT);\n");
    sql.push_str(&format!(
        "INSERT OR REPLACE INTO _backup_meta VALUES ('version', '{}');\n",
        env!("CARGO_PKG_VERSION")
    ));
    sql.push_str(&format!(
        "INSERT OR REPLACE INTO _backup_meta VALUES ('created_at', '{}');\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));

    // Tool configs table
    sql.push_str("CREATE TABLE IF NOT EXISTS _tool_configs (tool_id TEXT PRIMARY KEY, config_path TEXT, config_content TEXT);\n");

    // Skill files table
    sql.push_str("CREATE TABLE IF NOT EXISTS _skill_files (id INTEGER PRIMARY KEY AUTOINCREMENT, tool_id TEXT, name TEXT, content TEXT);\n\n");
    sql.push_str("CREATE TABLE IF NOT EXISTS _backup_files (id INTEGER PRIMARY KEY AUTOINCREMENT, root_key TEXT, relative_path TEXT, content_base64 TEXT);\n\n");

    // Data dump for all 12 business tables
    sql.push_str("-- ── Data ──\n\n");
    let tables = [
        "mcp_servers",
        "plugins",
        "skills",
        "hooks",
        "activity_logs",
        "mcp_clients",
        "workspaces",
        "custom_paths",
        "config_profiles",
        "app_settings",
        "imported_project_files",
        "update_history",
        "metrics",
    ];

    for table in tables {
        let query = format!("SELECT * FROM {}", table);
        if let Ok(mut stmt) = conn.prepare(&query) {
            let col_count = stmt.column_count();
            let col_names: Vec<String> = (0..col_count)
                .map(|i| stmt.column_name(i).unwrap_or("").to_string())
                .collect();

            let mut has_rows = false;
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
                    if !has_rows {
                        sql.push_str(&format!("-- Table: {}\n", table));
                        has_rows = true;
                    }
                    sql.push_str(&format!(
                        "INSERT OR REPLACE INTO {} ({}) VALUES ({});\n",
                        table,
                        col_names.join(", "),
                        row.join(", ")
                    ));
                }
            }
            if has_rows {
                sql.push('\n');
            }
        }
    }

    // Tool config files
    sql.push_str("-- ── Tool Configs ──\n\n");
    let tool_ids = [
        "claude", "codex", "gemini", "opencode", "openclaw", "hermes",
    ];
    for tool_id in tool_ids {
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

            sql.push_str(&format!(
                "INSERT OR REPLACE INTO _tool_configs VALUES ('{}', '{}', '{}');\n",
                tool_id,
                sql_escape(&config_path),
                sql_escape(&content)
            ));
        }
    }
    sql.push('\n');

    // Skill files
    sql.push_str("-- ── Skill Files ──\n\n");
    for tool_id in tool_ids {
        let skills_dir = match resolve_tool_skills_dir(conn, tool_id) {
            Ok(path) => path,
            Err(_) => continue,
        };
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
                            sql.push_str(&format!(
                                "INSERT INTO _skill_files (tool_id, name, content) VALUES ('{}', '{}', '{}');\n",
                                tool_id, sql_escape(&name), sql_escape(&content)
                            ));
                        }
                    }
                }
            }
        }
    }

    // Full file backup for tool directories and standalone config files
    sql.push_str("-- ── Full File Backup ──\n\n");
    let mut backup_roots: Vec<(String, PathBuf)> = Vec::new();
    for tool_id in tool_ids {
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

    for (root_key, relative_path, content_base64) in backup_file_rows {
        sql.push_str(&format!(
            "INSERT INTO _backup_files (root_key, relative_path, content_base64) VALUES ('{}', '{}', '{}');\n",
            sql_escape(&root_key),
            sql_escape(&relative_path),
            sql_escape(&content_base64),
        ));
    }

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

pub(crate) fn import_backup_from_path_impl(
    db: &State<'_, DbState>,
    file_path: &std::path::Path,
) -> Result<String, String> {
    let raw_content = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
    let content = validate_sql_backup_content(&raw_content)?;
    let restored_count = content.matches("\nINSERT").count();

    let db_path;
    let db_dir;
    let safety_backup_path;
    let pre_import_path;
    let temp_file = {
        let mut conn = db.0.lock().map_err(|e| e.to_string())?;
        db_path = get_main_db_path(&conn)?;
        db_dir = db_path
            .parent()
            .map(|path| path.to_path_buf())
            .ok_or("Cannot determine database directory")?;

        let backups_dir = db_dir.join("backups");
        std::fs::create_dir_all(&backups_dir).map_err(|e| e.to_string())?;

        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        safety_backup_path = backups_dir.join(format!("cchub-safety-{}.db", stamp));
        pre_import_path = backups_dir.join(format!("cchub-pre-import-{}.db", stamp));

        create_safety_db_backup(&conn, &safety_backup_path)?;

        let temp_file = tempfile::Builder::new()
            .prefix("cchub-import-")
            .suffix(".db")
            .tempfile_in(&db_dir)
            .map_err(|e| e.to_string())?;

        {
            let temp_conn =
                rusqlite::Connection::open(temp_file.path()).map_err(|e| e.to_string())?;
            configure_database_connection(&temp_conn, false)?;
            temp_conn
                .execute_batch(content)
                .map_err(|e| e.to_string())?;
            crate::db::schema::run_migrations(&temp_conn).map_err(|e| e.to_string())?;
            validate_imported_backup_tables(&temp_conn)?;
        }

        let placeholder = rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?;
        let old_conn = std::mem::replace(&mut *conn, placeholder);
        drop(conn);

        if let Err((old_conn, err)) = old_conn.close() {
            let mut conn = db.0.lock().map_err(|e| e.to_string())?;
            *conn = old_conn;
            return Err(err.to_string());
        }

        temp_file
    };

    let import_result =
        (|| -> Result<(rusqlite::Connection, usize, usize, usize, usize, usize), String> {
            remove_db_sidecars(&db_path);

            if db_path.exists() {
                std::fs::rename(&db_path, &pre_import_path).map_err(|e| e.to_string())?;
            }

            temp_file
                .persist(&db_path)
                .map_err(|e| e.error.to_string())?;

            let reopened = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
            configure_database_connection(&reopened, true)?;
            let (
                db_rows_restored,
                tool_configs_restored,
                skills_restored,
                full_files_restored,
                pending_project_files,
            ) = restore_imported_artifacts(&reopened, restored_count)?;
            let now = chrono::Utc::now().to_rfc3339();
            let imported_counts = sync_profiles_from_compatible_databases(&reopened, &now)?;
            sync_live_profiles(&reopened, &imported_counts, &now)?;

            Ok((
                reopened,
                db_rows_restored,
                tool_configs_restored,
                skills_restored,
                full_files_restored,
                pending_project_files,
            ))
        })();

    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    match import_result {
        Ok((
            reopened,
            db_rows_restored,
            tool_configs_restored,
            skills_restored,
            full_files_restored,
            pending_project_files,
        )) => {
            *conn = reopened;
            drop(conn);

            let _ = std::fs::remove_file(&pre_import_path);

            let mut message = format!(
                "已恢复 {} 条数据记录, {} 个工具配置, {} 个技能文件, {} 个附属文件。安全备份: {}",
                db_rows_restored,
                tool_configs_restored,
                skills_restored,
                full_files_restored,
                safety_backup_path.display()
            );
            if pending_project_files > 0 {
                message.push_str(&format!(
                    "；另有 {} 个项目文件已保留为迁移快照，修改工作区/项目路径后会自动恢复到新路径",
                    pending_project_files
                ));
            }
            let summary = LastImportSummary {
                imported_at: chrono::Utc::now().to_rfc3339(),
                db_rows_restored,
                tool_configs_restored,
                skills_restored,
                full_files_restored,
                pending_project_files,
                safety_backup_path: safety_backup_path.to_string_lossy().to_string(),
            };
            let reopened_conn = db.0.lock().map_err(|e| e.to_string())?;
            set_json_app_setting(&reopened_conn, "last_import_summary", &summary)?;
            Ok(message)
        }
        Err(err) => {
            remove_db_sidecars(&db_path);
            if pre_import_path.exists() {
                let _ = std::fs::remove_file(&db_path);
                let _ = std::fs::rename(&pre_import_path, &db_path);
            }

            let fallback = rusqlite::Connection::open(&db_path)
                .or_else(|_| rusqlite::Connection::open_in_memory())
                .map_err(|e| e.to_string())?;
            let _ = configure_database_connection(&fallback, true);
            *conn = fallback;

            Err(err)
        }
    }
}
