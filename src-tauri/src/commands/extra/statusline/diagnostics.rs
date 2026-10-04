#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;
use std::path::PathBuf;

use super::super::config_profiles::*;
use super::super::types::*;
use super::*;

pub fn refresh_mcp_servers_from_scan(conn: &rusqlite::Connection) -> Result<usize, String> {
    Ok(crate::mcp::operations::refresh(conn)?.len())
}

pub fn run_full_rescan_from_conn(conn: &rusqlite::Connection) -> Result<FullRescanResult, String> {
    let mcp_servers = refresh_mcp_servers_from_scan(conn)?;
    let skills = crate::skills::scanner::scan_local_skills_for_conn(conn).len();
    let hooks = crate::hooks::manager::read_hooks_from_settings(conn).len();
    let instruction_files = crate::claude_md::manager::scan_claude_md_files(conn).len();
    let workflows = crate::workflows::scan_workflow_files().len();
    let config_roots = crate::commands::config_files_commands::count_existing_config_roots(conn)?;
    let pending_project_roots = get_pending_imported_project_roots_from_conn(conn)?.len();
    let tool_reports = build_tool_environment_report_from_conn(conn)?;
    let tool_health_issues = tool_reports
        .iter()
        .filter(|report| {
            !report.cli_available
                || !report.config_dir_exists
                || !report.config_exists
                || !report.mcp_config_exists
                || !report.skills_dir_exists
        })
        .count();
    let manual_setup_required = tool_reports
        .iter()
        .filter(|report| report.manual_setup_kind.is_some())
        .count();

    let now = chrono::Utc::now().to_rfc3339();
    let imported_counts = sync_profiles_from_compatible_databases(conn, &now)?;
    sync_live_profiles(conn, &imported_counts, &now)?;

    Ok(FullRescanResult {
        mcp_servers,
        skills,
        hooks,
        instruction_files,
        workflows,
        config_roots,
        pending_project_roots,
        tool_health_issues,
        manual_setup_required,
    })
}

pub fn auto_remap_imported_project_roots_from_conn(
    conn: &rusqlite::Connection,
) -> Result<AutoRemapImportedProjectRootsResult, String> {
    let pending_roots = get_pending_imported_project_roots_from_conn(conn)?;
    let mut candidate_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut pending_key_counts: HashMap<String, usize> = HashMap::new();

    for candidate in discover_project_roots(conn) {
        let candidate_str = candidate.to_string_lossy().to_string();
        if let Some(key) = project_root_match_key(&candidate_str) {
            candidate_map.entry(key).or_default().push(candidate_str);
        }
    }

    for pending in &pending_roots {
        if let Some(key) = project_root_match_key(&pending.project_root) {
            *pending_key_counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut remapped_roots = 0usize;
    let mut restored_files = 0usize;
    let mut skipped_roots = 0usize;

    for pending in pending_roots {
        let Some(key) = project_root_match_key(&pending.project_root) else {
            skipped_roots += 1;
            continue;
        };

        if pending_key_counts.get(&key).copied().unwrap_or(0) != 1 {
            skipped_roots += 1;
            continue;
        }

        let Some(candidates) = candidate_map.get(&key) else {
            skipped_roots += 1;
            continue;
        };

        let Some(best_candidate) = best_project_root_candidate(&pending.project_root, candidates)
        else {
            skipped_roots += 1;
            continue;
        };

        let restored = apply_project_root_remap(conn, &pending.project_root, best_candidate)?;
        remapped_roots += 1;
        restored_files += restored;
    }

    Ok(AutoRemapImportedProjectRootsResult {
        remapped_roots,
        restored_files,
        skipped_roots,
    })
}

pub fn resolve_backup_root(conn: &rusqlite::Connection, root_key: &str) -> Result<PathBuf, String> {
    if root_key == "claude_mcp" {
        return Ok(resolve_claude_paths(conn)?.0);
    }

    if let Some(tool_id) = root_key.strip_prefix("tooldir:") {
        return resolve_tool_config_dir(conn, tool_id);
    }

    if let Some(tool_id) = root_key.strip_prefix("skillsdir:") {
        return resolve_tool_skills_dir(conn, tool_id);
    }

    if let Some(key) = root_key.strip_prefix("claude-desktop:") {
        return crate::commands::claude_desktop_profiles::backup_path(key);
    }

    if let Some(project_root) = root_key.strip_prefix("project:") {
        return Ok(PathBuf::from(project_root));
    }

    Err(format!("Unknown backup root: {}", root_key))
}

pub fn trim_utf8_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

pub fn validate_sql_backup_content(content: &str) -> Result<&str, String> {
    let trimmed = trim_utf8_bom(content).trim_start();
    let header_ok = trimmed
        .lines()
        .take(8)
        .any(|line| line.trim() == SQL_BACKUP_MARKER);

    if header_ok {
        Ok(trimmed)
    } else {
        Err("仅支持导入由 CCHub 导出的 SQL 备份文件".to_string())
    }
}

pub fn configure_database_connection(
    conn: &rusqlite::Connection,
    db_exists: bool,
) -> Result<(), String> {
    conn.execute_batch("PRAGMA journal_mode = WAL;")
        .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA synchronous = NORMAL;")
        .map_err(|e| e.to_string())?;

    if !db_exists {
        conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")
            .map_err(|e| e.to_string())?;
    }

    crate::db::schema::run_migrations(conn).map_err(|e| e.to_string())
}

pub fn get_main_db_path(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    let mut stmt = conn
        .prepare("PRAGMA database_list")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (name, file) = row.map_err(|e| e.to_string())?;
        if name == "main" && !file.trim().is_empty() {
            return Ok(PathBuf::from(file));
        }
    }

    Err("Cannot determine database path".to_string())
}

pub fn create_safety_db_backup(
    conn: &rusqlite::Connection,
    backup_path: &std::path::Path,
) -> Result<(), String> {
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    if backup_path.exists() {
        return Err("安全备份文件已存在，请使用新的备份路径".into());
    }

    let vacuum_sql = format!(
        "PRAGMA wal_checkpoint(TRUNCATE);\nVACUUM main INTO '{}';",
        sql_escape(&backup_path.to_string_lossy())
    );
    conn.execute_batch(&vacuum_sql).map_err(|e| e.to_string())
}
