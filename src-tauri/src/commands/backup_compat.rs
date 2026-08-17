use serde_json::json;
use tauri::State;

use crate::commands::extra_commands::{
    ensure_managed_backups_dir, generate_sql_backup, import_backup_from_path_impl,
    rename_managed_backup,
};
use crate::db::DbState;

#[tauri::command(rename_all = "camelCase")]
pub fn export_config_to_file(
    file_path: String,
    db: State<'_, DbState>,
) -> Result<serde_json::Value, String> {
    let target = std::path::PathBuf::from(&file_path);
    let parent = target.parent().ok_or("Invalid export path")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let content = generate_sql_backup(&conn, &home);
    crate::utils::atomic_write_string(&target, &content).map_err(|error| error.to_string())?;
    Ok(json!({"success": true, "filePath": file_path}))
}

#[tauri::command(rename_all = "camelCase")]
pub fn import_config_from_file(
    file_path: String,
    db: State<'_, DbState>,
) -> Result<serde_json::Value, String> {
    let path = std::path::PathBuf::from(&file_path);
    let restored = import_backup_from_path_impl(&db, &path)?;
    Ok(json!({"success": true, "message": restored, "filePath": file_path}))
}

#[tauri::command(rename_all = "camelCase")]
pub fn rename_db_backup(
    old_filename: String,
    new_name: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let directory = ensure_managed_backups_dir()?;
    let source = normalize_backup_path(&directory, &old_filename)?;
    rename_managed_backup(source.to_string_lossy().into_owned(), new_name, db)
}

fn normalize_backup_path(
    directory: &std::path::Path,
    value: &str,
) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(value);
    let candidate = if path.components().count() == 1 {
        directory.join(path)
    } else {
        path
    };
    if candidate.parent() != Some(directory) {
        return Err("Backup path must stay within the managed backup directory".to_string());
    }
    Ok(candidate)
}
