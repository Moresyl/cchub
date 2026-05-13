#![allow(clippy::too_many_arguments)]
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use crate::db::DbState;
use crate::shared::{github_release, github_urls, http_client};
use crate::utils::configure_background_command;

use super::super::config_profiles::*;
use super::super::log_command_timing;
use super::super::proxy_settings::*;
use super::super::types::*;
use super::*;

#[tauri::command]
pub async fn save_backup_to_file(db: State<'_, DbState>) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let sql_content = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        generate_sql_backup(&conn, &home)
    };

    let file = rfd::AsyncFileDialog::new()
        .set_title("导出备份")
        .set_file_name(format!(
            "cchub-backup-{}.sql",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ))
        .add_filter("SQL Backup", &["sql"])
        .save_file()
        .await;

    match file {
        Some(f) => {
            let path = f.path();
            std::fs::write(path, &sql_content).map_err(|e| e.to_string())?;
            Ok(path.to_string_lossy().to_string())
        }
        None => Err("Cancelled".to_string()),
    }
}

/// Import backup from SQL only.
#[tauri::command]
pub async fn import_backup_from_file(db: State<'_, DbState>) -> Result<String, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("导入备份")
        .add_filter("CCHub SQL Backup", &["sql"])
        .pick_file()
        .await;

    let file = file.ok_or("Cancelled")?;
    import_backup_from_path_impl(&db, file.path())
}

#[tauri::command]
pub fn get_backup_preferences(db: State<'_, DbState>) -> Result<BackupPreferences, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_backup_preferences_from_conn(&conn))
}

#[tauri::command]
pub fn set_backup_preferences(
    preferences: BackupPreferences,
    db: State<'_, DbState>,
) -> Result<BackupPreferences, String> {
    let sanitized = BackupPreferences {
        auto_backup_enabled: preferences.auto_backup_enabled,
        retention_count: preferences.retention_count.max(1),
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, BACKUP_PREFERENCES_SETTING_KEY, &sanitized)?;
    Ok(sanitized)
}

#[tauri::command]
pub fn list_managed_backups(db: State<'_, DbState>) -> Result<Vec<ManagedBackupFile>, String> {
    let _conn = db.0.lock().map_err(|e| e.to_string())?;
    let dir = ensure_managed_backups_dir()?;
    list_managed_backups_from_dir(&dir)
}

#[tauri::command]
pub fn create_managed_backup(
    kind: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let preferences = read_backup_preferences_from_conn(&conn);
    create_managed_backup_from_conn(
        &conn,
        kind.as_deref().unwrap_or("manual"),
        preferences.retention_count,
    )
}

#[tauri::command]
pub fn rename_managed_backup(
    path: String,
    new_name: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let _conn = db.0.lock().map_err(|e| e.to_string())?;
    let dir = ensure_managed_backups_dir()?;
    let source = PathBuf::from(&path);
    if source.parent() != Some(dir.as_path()) {
        return Err("Backup path must stay within the managed backup directory".to_string());
    }

    let sanitized = sanitize_backup_file_name(&new_name);
    if sanitized.trim().is_empty() {
        return Err("Backup name cannot be empty".to_string());
    }

    let target_name = if sanitized.ends_with(".sql") {
        sanitized
    } else {
        format!("{sanitized}.sql")
    };
    let target = dir.join(target_name);
    std::fs::rename(&source, &target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub fn delete_managed_backup(path: String, db: State<'_, DbState>) -> Result<(), String> {
    let _conn = db.0.lock().map_err(|e| e.to_string())?;
    let dir = ensure_managed_backups_dir()?;
    let target = PathBuf::from(path);
    if target.parent() != Some(dir.as_path()) {
        return Err("Backup path must stay within the managed backup directory".to_string());
    }
    std::fs::remove_file(&target).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restore_managed_backup(path: String, db: State<'_, DbState>) -> Result<String, String> {
    let dir = ensure_managed_backups_dir()?;
    let target = PathBuf::from(path);
    if target.parent() != Some(dir.as_path()) {
        return Err("Backup path must stay within the managed backup directory".to_string());
    }
    import_backup_from_path_impl(&db, &target)
}

#[tauri::command]
pub fn run_scheduled_backup_if_needed(db: State<'_, DbState>) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let preferences = read_backup_preferences_from_conn(&conn);
    if !preferences.auto_backup_enabled {
        return Ok(None);
    }

    let dir = ensure_managed_backups_dir()?;
    let backups = list_managed_backups_from_dir(&dir)?;
    let last_auto_backup = backups
        .into_iter()
        .find(|backup| backup.kind == "scheduled")
        .and_then(|backup| chrono::DateTime::parse_from_rfc3339(&backup.created_at).ok())
        .map(|datetime| datetime.with_timezone(&chrono::Utc));

    let should_create = last_auto_backup
        .map(|last| chrono::Utc::now().signed_duration_since(last).num_minutes() >= 60)
        .unwrap_or(true);

    if !should_create {
        return Ok(None);
    }

    let path = create_managed_backup_from_conn(&conn, "scheduled", preferences.retention_count)?;
    Ok(Some(path))
}

#[tauri::command]
pub fn remap_imported_project_root(
    source_path: String,
    target_path: String,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let restored = apply_project_root_remap(&conn, &source_path, &target_path)?;
    Ok(restored)
}

#[tauri::command]
pub fn get_pending_imported_project_roots(
    db: State<'_, DbState>,
) -> Result<Vec<PendingImportedProjectRoot>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    get_pending_imported_project_roots_from_conn(&conn)
}

#[tauri::command]
pub fn get_tool_environment_report(
    db: State<'_, DbState>,
) -> Result<Vec<ToolEnvironmentReport>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    build_tool_environment_report_from_conn(&conn)
}

#[tauri::command]
pub fn bootstrap_tool_environment(
    tool_id: String,
    db: State<'_, DbState>,
) -> Result<BootstrapToolEnvironmentResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    bootstrap_tool_environment_from_conn(&conn, &tool_id)
}

#[tauri::command]
pub fn auto_remap_imported_project_roots(
    db: State<'_, DbState>,
) -> Result<AutoRemapImportedProjectRootsResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    auto_remap_imported_project_roots_from_conn(&conn)
}

#[tauri::command]
pub fn get_last_import_summary(
    db: State<'_, DbState>,
) -> Result<Option<LastImportSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    get_json_app_setting(&conn, "last_import_summary")
}

#[tauri::command]
pub fn run_full_rescan(db: State<'_, DbState>) -> Result<FullRescanResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    run_full_rescan_from_conn(&conn)
}

#[tauri::command]
pub fn repair_all_migration_issues(db: State<'_, DbState>) -> Result<RepairAllResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let remap = auto_remap_imported_project_roots_from_conn(&conn)?;
    let reports = build_tool_environment_report_from_conn(&conn)?;
    let mut bootstrapped_tools = 0usize;
    let mut created_dirs = 0usize;
    let mut created_files = 0usize;
    let mut bootstrap_notes = Vec::new();

    for report in reports {
        if report.config_dir_exists
            && report.config_exists
            && report.mcp_config_exists
            && report.skills_dir_exists
        {
            continue;
        }

        let result = bootstrap_tool_environment_from_conn(&conn, &report.tool_id)?;
        if result.created_dirs > 0 || result.created_files > 0 {
            bootstrapped_tools += 1;
        }
        created_dirs += result.created_dirs;
        created_files += result.created_files;
        for note in result.notes {
            bootstrap_notes.push(format!("{}: {}", report.tool_name, note));
        }
    }

    let rescan = run_full_rescan_from_conn(&conn)?;
    Ok(RepairAllResult {
        remapped_roots: remap.remapped_roots,
        restored_project_files: remap.restored_files,
        skipped_remap_roots: remap.skipped_roots,
        bootstrapped_tools,
        created_dirs,
        created_files,
        bootstrap_notes,
        rescan,
    })
}

#[tauri::command]
pub fn open_in_system(target: String) -> Result<(), String> {
    open_target_in_system(&target)
}

#[cfg(test)]
mod tests {
    use super::{
        best_project_root_candidate, normalized_path_segments, project_root_match_key,
        shared_trailing_segment_count,
    };

    #[test]
    fn project_root_key_uses_last_segment() {
        assert_eq!(
            project_root_match_key("D:/work/foo-bar").as_deref(),
            Some("foo-bar")
        );
        assert_eq!(
            project_root_match_key("/tmp/demo/").as_deref(),
            Some("demo")
        );
        assert_eq!(project_root_match_key("   ").as_deref(), None);
    }

    #[test]
    fn shared_trailing_segments_counts_suffix_depth() {
        assert_eq!(
            shared_trailing_segment_count("D:/old/workspace/acme/app", "E:/new/workspace/acme/app"),
            3
        );
        assert_eq!(
            shared_trailing_segment_count(
                "D:/old/workspace/acme/app",
                "E:/new/workspace/other/app"
            ),
            1
        );
    }

    #[test]
    fn best_candidate_prefers_longest_unique_suffix_match() {
        let candidates = vec![
            "E:/new/workspace/acme/app".to_string(),
            "E:/archive/app".to_string(),
        ];

        let best = best_project_root_candidate("D:/old/workspace/acme/app", &candidates)
            .map(|value| value.as_str());

        assert_eq!(best, Some("E:/new/workspace/acme/app"));
    }

    #[test]
    fn best_candidate_rejects_ambiguous_matches() {
        let candidates = vec!["E:/new/a/app".to_string(), "F:/new/b/app".to_string()];

        let best =
            best_project_root_candidate("D:/old/c/app", &candidates).map(|value| value.as_str());

        assert_eq!(best, None);
    }

    #[test]
    fn normalized_segments_ignore_empty_parts() {
        assert_eq!(
            normalized_path_segments("D:\\foo\\\\bar\\baz"),
            vec![
                "d:".to_string(),
                "foo".to_string(),
                "bar".to_string(),
                "baz".to_string()
            ]
        );
    }
}
