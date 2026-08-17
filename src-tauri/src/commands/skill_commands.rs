use crate::db::models::{Plugin, Skill};
use crate::db::{record_activity, DbState};
use crate::skills::{installer, plugin_installer, scanner, tools, updater};
use tauri::State;

fn log_command_timing(command: &str, started_at: std::time::Instant) {
    eprintln!(
        "[cchub][invoke] {command} completed in {}ms",
        started_at.elapsed().as_millis()
    );
}

#[tauri::command]
pub fn scan_skills(db: State<'_, DbState>) -> Result<Vec<Skill>, String> {
    let started_at = std::time::Instant::now();
    let plan = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        scanner::prepare_local_skill_scan(&conn)
    };
    let result = Ok(scanner::scan_local_skills_from_plan(&plan));
    log_command_timing("scan_skills", started_at);
    result
}

#[tauri::command]
pub fn get_skills(db: State<'_, DbState>) -> Result<Vec<Skill>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, plugin_id, trigger_command, file_path, version, installed_at,
                    source_url, baseline_sha256, latest_sha256, last_checked_at
             FROM skills",
        )
        .map_err(|e| e.to_string())?;

    let skills = stmt
        .query_map([], |row| {
            Ok(Skill {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                tool_id: None,
                plugin_id: row.get(3)?,
                trigger_command: row.get(4)?,
                file_path: row.get(5)?,
                version: row.get(6)?,
                installed_at: row.get(7)?,
                source_url: row.get(8)?,
                baseline_sha256: row.get(9)?,
                latest_sha256: row.get(10)?,
                last_checked_at: row.get(11)?,
                current_sha256: None,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(skills)
}

#[tauri::command]
pub fn get_plugins(_db: State<'_, DbState>) -> Result<Vec<Plugin>, String> {
    Ok(scanner::scan_local_plugins())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn install_plugin(source_url: String, db: State<'_, DbState>) -> Result<String, String> {
    let installed = plugin_installer::install_plugin(&source_url).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO plugins (id, name, description, source_url, version, installed_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            source_url = excluded.source_url,
            version = excluded.version,
            updated_at = excluded.updated_at",
        rusqlite::params![
            &installed.id,
            &installed.name,
            &installed.description,
            &installed.source_url,
            &installed.version,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM skills WHERE plugin_id = ?1",
        rusqlite::params![&installed.id],
    )
    .map_err(|e| e.to_string())?;
    for skill in scanner::scan_local_skills_for_conn(&conn)
        .into_iter()
        .filter(|skill| skill.plugin_id.as_deref() == Some(installed.id.as_str()))
    {
        updater::persist_skill_metadata(&conn, &skill)?;
    }
    record_activity(&conn, &installed.id, "plugin_install", "success", None);
    Ok(installed.path)
}

#[tauri::command]
pub fn read_skill_content(file_path: String) -> Result<String, String> {
    std::fs::read_to_string(&file_path).map_err(|e| format!("Failed to read {}: {}", file_path, e))
}

#[tauri::command]
pub fn uninstall_plugin(plugin_id: String, db: State<'_, DbState>) -> Result<(), String> {
    let plugin_id = plugin_installer::validate_plugin_id(&plugin_id)?;
    let plugins_dir = scanner::get_plugins_dir().ok_or("Cannot find plugins directory")?;
    let plugin_path = plugins_dir.join(&plugin_id);
    if plugin_path.exists() {
        let backup_dir = dirs::home_dir()
            .ok_or("Cannot find home directory")?
            .join(".cchub")
            .join("plugin-backups");
        std::fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
        let backup_path = backup_dir.join(format!("{}-{}", plugin_id, uuid::Uuid::new_v4()));
        std::fs::rename(&plugin_path, backup_path).map_err(|e| e.to_string())?;
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM skills WHERE plugin_id = ?1",
        rusqlite::params![plugin_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM plugins WHERE id = ?1",
        rusqlite::params![plugin_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── New commands for enhanced skill management ──

#[tauri::command]
pub fn detect_tools(db: State<'_, DbState>) -> Result<Vec<tools::DetectedTool>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(tools::detect_tools_for_conn(&conn))
}

#[tauri::command]
pub fn get_skill_folder_tree(base_dir: String) -> Result<scanner::FolderNode, String> {
    scanner::get_folder_tree(&base_dir)
}

#[tauri::command]
pub fn check_path_exists(path: String) -> bool {
    scanner::check_path_exists(&path)
}

#[tauri::command]
pub fn get_skill_categories(db: State<'_, DbState>) -> Result<scanner::CategoryCounts, String> {
    let plan = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        scanner::prepare_local_skill_scan(&conn)
    };
    let skills = scanner::scan_local_skills_from_plan(&plan);
    Ok(scanner::get_category_counts(&skills))
}

#[tauri::command]
pub fn install_skill_file(
    source: String,
    target_skills_dir: String,
    method: Option<String>,
) -> Result<String, String> {
    let m = method.as_deref().unwrap_or("copy");
    installer::install_skill_file(&source, &target_skills_dir, m)
}

#[tauri::command]
pub fn uninstall_skill_file(path: String, db: State<'_, DbState>) -> Result<(), String> {
    let skill_name = std::path::Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    installer::uninstall_skill_file(&path)?;
    if let Ok(conn) = db.0.lock() {
        let _ = updater::remove_skill_metadata(&conn, &path);
        record_activity(&conn, &skill_name, "skill_uninstall", "success", None);
    }
    Ok(())
}

#[tauri::command]
pub fn get_skill_backups() -> Result<Vec<installer::SkillBackup>, String> {
    let started_at = std::time::Instant::now();
    let result = installer::list_skill_backups();
    log_command_timing("get_skill_backups", started_at);
    result
}

#[tauri::command]
pub fn restore_skill_backup(id: String, target_path: Option<String>) -> Result<String, String> {
    installer::restore_skill_backup(&id, target_path.as_deref())
}

#[tauri::command]
pub fn delete_skill_backup(id: String) -> Result<(), String> {
    installer::delete_skill_backup(&id)
}

#[tauri::command]
pub fn copy_skill_between_tools(
    path: String,
    target_skills_dir: String,
    method: Option<String>,
) -> Result<String, String> {
    let m = method.as_deref().unwrap_or("copy");
    installer::copy_skill_between_tools(&path, &target_skills_dir, m)
}

/// Remove a synced skill from a target tool's skills directory
#[tauri::command]
pub fn remove_synced_skill(skill_name: String, target_skills_dir: String) -> Result<(), String> {
    let dir = std::path::Path::new(&target_skills_dir);
    // Try to find and delete the skill file by name
    if dir.exists() {
        // Try exact filename
        for ext in ["", ".md", ".disabled", ".md.disabled"] {
            let path = dir.join(format!("{}{}", skill_name, ext));
            if path.exists() {
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
                } else {
                    installer::uninstall_skill_file(&path.to_string_lossy())?;
                }
                return Ok(());
            }
        }
        // Try scanning for files containing the skill name
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.contains(&skill_name) {
                    let path = entry.path();
                    if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                    } else {
                        let _ = installer::uninstall_skill_file(&path.to_string_lossy());
                    }
                    return Ok(());
                }
            }
        }
    }
    Err(format!(
        "Skill '{}' not found in {}",
        skill_name, target_skills_dir
    ))
}

#[tauri::command]
pub fn write_skill_content(
    file_path: String,
    content: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    crate::utils::atomic_write_string(std::path::Path::new(&file_path), &content)
        .map_err(|e| format!("Failed to write {}: {}", file_path, e))?;
    if let Ok(conn) = db.0.lock() {
        let _ = updater::sync_skill_baseline_to_current(&conn, &file_path);
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_skill_file(
    file_path: String,
    enabled: bool,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let skill_name = std::path::Path::new(&file_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.clone());
    let disabled_suffix = ".disabled";

    let result = if enabled {
        // Remove .disabled suffix
        if file_path.ends_with(disabled_suffix) {
            let new_path = &file_path[..file_path.len() - disabled_suffix.len()];
            std::fs::rename(&file_path, new_path)
                .map_err(|e| format!("Failed to enable: {}", e))?;
            Ok(new_path.to_string())
        } else {
            Ok(file_path.clone()) // Already enabled
        }
    } else {
        // Add .disabled suffix
        if !file_path.ends_with(disabled_suffix) {
            let new_path = format!("{}{}", file_path, disabled_suffix);
            std::fs::rename(&file_path, &new_path)
                .map_err(|e| format!("Failed to disable: {}", e))?;
            Ok(new_path)
        } else {
            Ok(file_path.clone()) // Already disabled
        }
    };
    if let Ok(conn) = db.0.lock() {
        if let Ok(new_path) = &result {
            let _ = updater::rename_skill_metadata(&conn, &file_path, new_path);
        }
        record_activity(
            &conn,
            &skill_name,
            if enabled {
                "skill_enable"
            } else {
                "skill_disable"
            },
            "success",
            None,
        );
    }
    result
}

#[tauri::command]
pub async fn check_skill_updates(
    ids: Vec<String>,
    db: State<'_, DbState>,
) -> Result<Vec<updater::SkillUpdateStatus>, String> {
    let metadata_map = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        updater::load_skill_metadata_map(&conn)?
    };

    let mut result = Vec::new();
    for id in ids {
        let metadata = metadata_map.get(&id);
        let current_sha256 = updater::file_sha256(std::path::Path::new(&id)).ok();
        let Some(source_url) = metadata.and_then(|item| item.source_url.as_deref()) else {
            result.push(updater::SkillUpdateStatus {
                id,
                update_available: false,
                latest_sha256: metadata.and_then(|item| item.latest_sha256.clone()),
                current_sha256,
                last_checked_at: metadata.and_then(|item| item.last_checked_at),
                error: Some("No upstream source URL is recorded for this skill".to_string()),
            });
            continue;
        };

        let now = chrono::Utc::now().timestamp();
        match updater::fetch_remote_skill_content(source_url).await {
            Ok(content) => {
                let latest_sha256 = updater::sha256_hex(&content);
                {
                    let conn = db.0.lock().map_err(|e| e.to_string())?;
                    conn.execute(
                        "UPDATE skills SET latest_sha256 = ?1, last_checked_at = ?2 WHERE id = ?3 OR file_path = ?3",
                        rusqlite::params![latest_sha256, now, id],
                    )
                    .map_err(|e| e.to_string())?;
                }
                let update_available = current_sha256
                    .as_ref()
                    .map(|current| current != &latest_sha256)
                    .unwrap_or(false);
                result.push(updater::SkillUpdateStatus {
                    id,
                    update_available,
                    latest_sha256: Some(latest_sha256),
                    current_sha256,
                    last_checked_at: Some(now),
                    error: None,
                });
            }
            Err(error) => {
                {
                    let conn = db.0.lock().map_err(|e| e.to_string())?;
                    conn.execute(
                        "UPDATE skills SET last_checked_at = ?1 WHERE id = ?2 OR file_path = ?2",
                        rusqlite::params![now, id],
                    )
                    .map_err(|e| e.to_string())?;
                }
                result.push(updater::SkillUpdateStatus {
                    id,
                    update_available: false,
                    latest_sha256: metadata.and_then(|item| item.latest_sha256.clone()),
                    current_sha256,
                    last_checked_at: Some(now),
                    error: Some(error),
                });
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn batch_update_skills(
    ids: Vec<String>,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let metadata_map = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        updater::load_skill_metadata_map(&conn)?
    };

    let mut updated = 0usize;
    for id in ids {
        let Some(source_url) = metadata_map
            .get(&id)
            .and_then(|item| item.source_url.as_deref())
        else {
            continue;
        };
        let content = updater::fetch_remote_skill_content(source_url).await?;
        crate::utils::atomic_write_string(std::path::Path::new(&id), &content)
            .map_err(|e| format!("Failed to write updated skill {}: {}", id, e))?;
        let hash = updater::sha256_hex(&content);
        let now = chrono::Utc::now().timestamp();
        {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE skills SET baseline_sha256 = ?1, latest_sha256 = ?1, last_checked_at = ?2 WHERE id = ?3 OR file_path = ?3",
                rusqlite::params![hash, now, id],
            )
            .map_err(|e| e.to_string())?;
        }
        updated += 1;
    }

    Ok(updated)
}

#[tauri::command]
pub fn delete_plugin_dir(plugin_name: String) -> Result<(), String> {
    let plugin_name = plugin_installer::validate_plugin_id(&plugin_name)?;
    let plugins_dir = scanner::get_plugins_dir().ok_or("Cannot find plugins directory")?;
    let plugin_path = plugins_dir.join(plugin_name);
    if plugin_path.exists() && plugin_path.is_dir() {
        std::fs::remove_dir_all(&plugin_path)
            .map_err(|e| format!("Failed to delete plugin directory: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_skill_sync_method(db: State<'_, DbState>) -> String {
    if let Ok(conn) = db.0.lock() {
        if let Ok(val) = conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'skill_sync_method'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            if !val.is_empty() {
                return val;
            }
        }
    }
    "copy".to_string()
}

#[tauri::command]
pub fn set_skill_sync_method(method: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('skill_sync_method', ?1)",
        rusqlite::params![method],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Import a skill .md file from disk via file dialog
#[tauri::command]
pub async fn import_skill_file(
    target_skills_dir: String,
    method: Option<String>,
) -> Result<String, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Import Skill")
        .add_filter("Skill Package", &["md", "zip", "skill"])
        .pick_file()
        .await
        .ok_or("Cancelled")?;

    let source = file.path().to_string_lossy().to_string();
    let m = method.as_deref().unwrap_or("copy");
    crate::skills::installer::install_skill_file(&source, &target_skills_dir, m)
}
