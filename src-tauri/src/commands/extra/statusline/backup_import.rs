use std::path::Path;
use tauri::State;

use super::super::config_profiles::*;
use super::super::types::LastImportSummary;
use super::*;
use crate::db::DbState;

pub(crate) fn import_backup_from_path_impl(
    db: &State<'_, DbState>,
    file_path: &Path,
) -> Result<String, String> {
    import_with_mode(db, file_path, false)
}

pub(crate) fn import_cloud_backup_from_path_impl(
    db: &State<'_, DbState>,
    file_path: &Path,
) -> Result<String, String> {
    import_with_mode(db, file_path, true)
}

fn import_with_mode(
    db: &State<'_, DbState>,
    file_path: &Path,
    cloud: bool,
) -> Result<String, String> {
    let content = std::fs::read_to_string(file_path).map_err(|_| "无法读取备份文件")?;
    let mut conn = db.0.lock().map_err(|_| "数据库当前不可用")?;
    import_into_connection_with_mode(&mut conn, &content, cloud)
}

fn install_database(
    source: &rusqlite::Connection,
    target: &mut rusqlite::Connection,
) -> Result<(), String> {
    let result = {
        let backup =
            rusqlite::backup::Backup::new(source, target).map_err(|_| "无法开始数据库恢复")?;
        backup.step(-1).map_err(|_| "数据库恢复失败")?
    };
    if !matches!(result, rusqlite::backup::StepResult::Done) {
        return Err("数据库正被其他操作占用，恢复已停止，请稍后重试".into());
    }
    target.flush_prepared_statement_cache();
    Ok(())
}

fn install_restored_database(
    source: &rusqlite::Connection,
    target: &mut rusqlite::Connection,
    files: &super::backup_file_rollback::FileRollback,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    files.verify()?;
    install_database(source, target)
}

fn rollback_restored_files(
    files: &mut super::backup_file_rollback::FileRollback,
    original_error: String,
) -> String {
    // Artifact preparation and live snapshot readers release the write lock.
    // Reacquire it for recovery so another application writer cannot interleave
    // our ownership check and replacement. A poisoned lock rejects all other
    // application writers, but must not discard recoverable original files.
    let guard = crate::json_config::write_lock();
    let error = match &guard {
        Ok(_) => original_error,
        Err(error) => format!("{original_error}；{error}"),
    };
    files.rollback(error)
}

#[cfg(test)]
fn import_into_connection(
    conn: &mut rusqlite::Connection,
    content: &str,
) -> Result<String, String> {
    import_into_connection_with_mode(conn, content, false)
}

fn import_into_connection_with_mode(
    conn: &mut rusqlite::Connection,
    content: &str,
    cloud: bool,
) -> Result<String, String> {
    validate_sql_backup_content(content)?;
    let db_path = get_main_db_path(conn)?;
    let db_dir = db_path.parent().ok_or("无法确定数据库目录")?;
    let temp_file = tempfile::Builder::new()
        .prefix("cchub-import-")
        .suffix(".db")
        .tempfile_in(db_dir)
        .map_err(|_| "无法创建备份导入暂存文件")?;
    let prepared =
        rusqlite::Connection::open(temp_file.path()).map_err(|_| "无法打开备份导入暂存数据库")?;
    configure_database_connection(&prepared, false)?;
    super::backup_sql::load_backup_sql(&prepared, content)?;
    super::backup_paths::validate_artifact_paths(&prepared)?;
    let library = crate::mcp::service::prepare_backup_library(&prepared)?;
    let mut preserved_rows = if cloud {
        super::backup_ownership::preserve_device_state(conn, &prepared)?
    } else {
        0
    };
    preserved_rows += crate::mcp::service::restore_backup_library(conn, &prepared, library)?;
    let count =
        super::backups_restore::count_backup_rows(&prepared)?.saturating_sub(preserved_rows);

    let backup_dir = db_dir.join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|_| "无法创建安全备份目录")?;
    let safety = backup_dir.join(format!("cchub-safety-{}.db", uuid::Uuid::new_v4()));
    create_safety_db_backup(conn, &safety)?;
    let mut rollback = super::backup_file_rollback::FileRollback::new(db_dir)?;
    let result = (|| {
        let (rows, configs, skills, files, pending) =
            super::backup_artifacts::restore_artifacts_with_rollback(
                &prepared,
                count,
                &mut rollback,
                cloud,
            )?;
        let now = chrono::Utc::now().to_rfc3339();
        let imported = sync_profiles_from_compatible_databases(&prepared, &now)?;
        sync_live_profiles(&prepared, &imported, &now)?;
        let summary = LastImportSummary {
            imported_at: now,
            db_rows_restored: rows,
            tool_configs_restored: configs,
            skills_restored: skills,
            full_files_restored: files,
            pending_project_files: pending,
            safety_backup_path: safety.to_string_lossy().into_owned(),
        };
        set_json_app_setting(&prepared, "last_import_summary", &summary)?;
        // No placeholder connection, close/rename window, or fallback to an empty
        // database. SQLite commits the page copy while the application mutex stays held.
        install_restored_database(&prepared, conn, &rollback)?;
        crate::skills::tools::invalidate_detect_tools_cache();
        let mut message = format!("已恢复 {rows} 条数据记录, {configs} 个工具配置, {skills} 个技能文件, {files} 个附属文件。安全备份: {}", safety.display());
        if pending > 0 {
            message.push_str(&format!("；另有 {pending} 个项目文件已保留为迁移快照，修改工作区/项目路径后会自动恢复到新路径"));
        }
        Ok(message)
    })();
    match result {
        Ok(message) => {
            rollback.commit();
            Ok(message)
        }
        Err(error) => Err(rollback_restored_files(&mut rollback, error)),
    }
}

#[cfg(test)]
mod tests;
