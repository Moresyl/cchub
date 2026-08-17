//! Unified skill management commands.
//!
//! The application stores skill metadata in SQLite while the actual content
//! remains a normal Markdown file. These commands bridge both representations
//! and keep imports path-safe.

use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

use crate::commands::extra_commands::resolve_tool_skills_dir;
use crate::db::{record_activity, DbState};
use crate::skills::installer;
use crate::skills::updater;

const MAX_SKILL_BYTES: u64 = 2 * 1024 * 1024;
const KNOWN_TOOLS: &[&str] = &[
    "claude", "codex", "gemini", "opencode", "openclaw", "hermes", "pi",
];

fn text_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn safe_skill_filename(name: &str) -> Result<String, String> {
    let raw = name.trim();
    if raw.is_empty() || raw.contains(['/', '\\']) || raw == "." || raw == ".." {
        return Err("Skill name must be a single file name".to_string());
    }
    let stem = Path::new(raw)
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or(raw);
    let safe = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        return Err("Skill name is empty".to_string());
    }
    Ok(format!("{safe}.md"))
}

fn read_skill_content(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.len() > MAX_SKILL_BYTES {
        return Err(format!(
            "Skill file exceeds the 2 MiB limit: {}",
            path.display()
        ));
    }
    let content =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if content.len() as u64 > MAX_SKILL_BYTES {
        return Err(format!(
            "Skill file exceeds the 2 MiB limit: {}",
            path.display()
        ));
    }
    Ok(content)
}

fn skill_content(skill: &Value) -> Result<String, String> {
    if let Some(content) = text_field(skill, &["content", "markdown", "body"]) {
        if content.len() as u64 > MAX_SKILL_BYTES {
            return Err("Skill content exceeds the 2 MiB limit".to_string());
        }
        return Ok(content);
    }
    if let Some(path) = text_field(skill, &["sourcePath", "filePath", "path"]) {
        return read_skill_content(Path::new(&path));
    }
    Err("Skill content or sourcePath is required".to_string())
}

fn app_targets(value: &Value, default_app: Option<&str>) -> Vec<String> {
    let mut apps = value
        .get("apps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if apps.is_empty() {
        if let Some(app) = text_field(value, &["app", "targetApp", "currentApp"]) {
            apps.push(app);
        }
    }
    if apps.is_empty() {
        if let Some(app) = default_app.filter(|item| !item.trim().is_empty()) {
            apps.push(app.to_string());
        }
    }
    apps.sort();
    apps.dedup();
    apps
}

fn persist_skill(
    conn: &rusqlite::Connection,
    path: &Path,
    skill: &Value,
    content: &str,
) -> Result<(), String> {
    let name = text_field(skill, &["name", "title", "id"])
        .or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "skill".to_string());
    let description = text_field(skill, &["description", "summary"]);
    let source_url = text_field(skill, &["sourceUrl", "githubUrl", "url"]);
    updater::persist_marketplace_skill_install(
        conn,
        &path.to_string_lossy(),
        &name,
        description.as_deref(),
        None,
        source_url.as_deref(),
        content,
    )
}

fn install_to_app(conn: &rusqlite::Connection, skill: &Value, app: &str) -> Result<Value, String> {
    let app = app.trim().to_ascii_lowercase();
    if !KNOWN_TOOLS.contains(&app.as_str()) {
        return Err(format!("Unsupported skill target: {app}"));
    }
    let name = text_field(skill, &["name", "title", "id"])
        .or_else(|| {
            text_field(skill, &["sourcePath", "filePath", "path"]).and_then(|path| {
                Path::new(&path)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .map(ToOwned::to_owned)
            })
        })
        .ok_or_else(|| "Skill name is required".to_string())?;
    let content = skill_content(skill)?;
    let target_dir = resolve_tool_skills_dir(conn, &app)?;
    fs::create_dir_all(&target_dir)
        .map_err(|error| format!("{}: {error}", target_dir.display()))?;
    let target = target_dir.join(safe_skill_filename(&name)?);
    crate::utils::atomic_write_string(&target, &content)
        .map_err(|error| format!("{}: {error}", target.display()))?;
    if let Err(error) = persist_skill(conn, &target, skill, &content) {
        let _ = fs::remove_file(&target);
        return Err(error);
    }
    record_activity(conn, &name, "skill_install", "success", None);
    Ok(json!({
        "id": text_field(skill, &["id"]).unwrap_or_else(|| target.to_string_lossy().to_string()),
        "name": name,
        "description": text_field(skill, &["description", "summary"]),
        "filePath": target.to_string_lossy(),
        "apps": [app],
        "installed": true,
    }))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn install_skill_unified(
    skill: Value,
    current_app: String,
    db: State<'_, DbState>,
) -> Result<Value, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    install_to_app(&conn, &skill, &current_app)
}

/// Remove a managed skill without losing the last installed file.  The backup
/// is created before the file is deleted so a failed removal cannot leave the
/// database pointing at an unrecoverable path.
pub fn uninstall_skill_unified(id: String, db: State<'_, DbState>) -> Result<Value, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let (name, file_path): (String, String) = conn
        .query_row(
            "SELECT name, file_path FROM skills WHERE id = ?1 OR name = ?1 OR file_path = ?1 LIMIT 1",
            rusqlite::params![id.trim()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| format!("Managed skill not found: {id}: {error}"))?;
    let path = PathBuf::from(&file_path);
    let backup = installer::create_skill_backup(&path)?;
    if let Err(error) = fs::remove_file(&path) {
        let _ = installer::delete_skill_backup(&backup.id);
        return Err(format!(
            "Failed to remove skill {}: {error}",
            path.display()
        ));
    }
    updater::remove_skill_metadata(&conn, &file_path)?;
    record_activity(&conn, &name, "skill_uninstall", "success", None);
    Ok(json!({
        "backupPath": backup.backup_path,
        "preservedPiPath": Value::Null,
        "piCleanupIncomplete": false,
    }))
}

fn known_skill_paths(conn: &rusqlite::Connection) -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();
    for tool in KNOWN_TOOLS {
        if let Ok(dir) = resolve_tool_skills_dir(conn, tool) {
            result.push(((*tool).to_string(), dir));
        }
    }
    result
}

fn collect_skill_files(root: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let nested = path.join("SKILL.md");
            if nested.is_file() {
                output.push(nested);
            } else {
                collect_skill_files(&path, output);
            }
        } else if path.extension().and_then(|value| value.to_str()) == Some("md") {
            output.push(path);
        }
    }
}

fn managed_path(conn: &rusqlite::Connection, path: &Path) -> bool {
    conn.query_row(
        "SELECT COUNT(1) FROM skills WHERE file_path = ?1",
        rusqlite::params![path.to_string_lossy().to_string()],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

#[tauri::command]
pub fn scan_unmanaged_skills(db: State<'_, DbState>) -> Result<Vec<Value>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for (app, root) in known_skill_paths(&conn) {
        let mut files = Vec::new();
        collect_skill_files(&root, &mut files);
        for path in files {
            let key = path.to_string_lossy().to_string();
            if !seen.insert(key.clone()) || managed_path(&conn, &path) {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("skill")
                .to_string();
            result.push(json!({
                "name": name,
                "path": key,
                "filePath": key,
                "app": app,
                "sizeBytes": fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0),
            }));
        }
    }
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
pub fn import_skills_from_apps(
    imports: Vec<Value>,
    db: State<'_, DbState>,
) -> Result<Vec<Value>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut result = Vec::new();
    for item in imports {
        let source = text_field(&item, &["sourcePath", "filePath", "path"])
            .ok_or_else(|| "Each skill import requires sourcePath".to_string())?;
        let content = read_skill_content(Path::new(&source))?;
        let name = text_field(&item, &["name", "title", "id"])
            .or_else(|| {
                Path::new(&source)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .map(ToOwned::to_owned)
            })
            .ok_or_else(|| "Imported skill name is missing".to_string())?;
        let payload = json!({
            "id": text_field(&item, &["id"]).unwrap_or_else(|| source.clone()),
            "name": name,
            "description": text_field(&item, &["description", "summary"]),
            "content": content,
            "sourcePath": source,
        });
        let apps = app_targets(&item, None);
        if apps.is_empty() {
            return Err(format!("No target app selected for {source}"));
        }
        for app in apps {
            result.push(install_to_app(&conn, &payload, &app)?);
        }
    }
    Ok(result)
}

fn migration_target(value: &Value) -> Result<PathBuf, String> {
    let explicit = value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| text_field(value, &["path", "directory", "targetPath"]));
    if let Some(path) = explicit.filter(|path| !path.trim().is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let kind = text_field(value, &["kind", "type"]).unwrap_or_else(|| "cchub".to_string());
    if kind.eq_ignore_ascii_case("cchub") || kind.eq_ignore_ascii_case("central") {
        return dirs::home_dir()
            .map(|home| home.join(".cchub").join("skills"))
            .ok_or_else(|| "Cannot determine the home directory".to_string());
    }
    if kind.eq_ignore_ascii_case("unified") || kind.eq_ignore_ascii_case("global") {
        return dirs::home_dir()
            .map(|home| home.join(".agents").join("skills"))
            .ok_or_else(|| "Cannot determine the home directory".to_string());
    }
    Err("Skill migration target must include a directory".to_string())
}

#[tauri::command]
pub fn get_skill_storage_location(db: State<'_, DbState>) -> String {
    db.0.lock()
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT value FROM app_settings WHERE key = 'skill_storage_location'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
        })
        .filter(|value| matches!(value.as_str(), "cchub" | "unified" | "tool"))
        .unwrap_or_else(|| "tool".to_string())
}

#[tauri::command]
pub fn set_skill_storage_location(location: String, db: State<'_, DbState>) -> Result<(), String> {
    let normalized = location.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "cchub" | "unified" | "tool") {
        return Err("Skill storage location must be cchub, unified, or tool".to_string());
    }
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('skill_storage_location', ?1)",
        rusqlite::params![normalized],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn migrate_skill_storage(target: Value, db: State<'_, DbState>) -> Result<Value, String> {
    let target_dir = migration_target(&target)?;
    fs::create_dir_all(&target_dir)
        .map_err(|error| format!("{}: {error}", target_dir.display()))?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, file_path FROM skills WHERE file_path IS NOT NULL")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut migrated = 0u64;
    let mut skipped = 0u64;
    let mut errors = Vec::new();
    let mut pending_updates = Vec::new();
    let mut created_targets = Vec::new();
    for row in rows {
        let (id, name, source_text) = row.map_err(|error| error.to_string())?;
        let source = PathBuf::from(&source_text);
        let content = match read_skill_content(&source) {
            Ok(content) => content,
            Err(error) => {
                errors.push(error);
                skipped += 1;
                continue;
            }
        };
        let base_name = safe_skill_filename(&name)?;
        let mut target = target_dir.join(&base_name);
        if target.exists() && target != source {
            let same_content = fs::read_to_string(&target)
                .map(|existing| existing == content)
                .unwrap_or(false);
            if !same_content {
                let stem = base_name.trim_end_matches(".md");
                let suffix: String = id
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
                    .take(16)
                    .collect();
                let suffix = if suffix.is_empty() { "copy" } else { &suffix };
                let mut candidate = target_dir.join(format!("{stem}-{suffix}.md"));
                let mut index = 2u32;
                while candidate.exists() {
                    candidate = target_dir.join(format!("{stem}-{suffix}-{index}.md"));
                    index = index.saturating_add(1);
                }
                target = candidate;
            }
        }
        let target_preexisted = target.exists();
        if let Err(error) = crate::utils::atomic_write_string(&target, &content) {
            errors.push(format!("{}: {error}", target.display()));
            skipped += 1;
            continue;
        }
        if !target_preexisted {
            created_targets.push(target.clone());
        }
        pending_updates.push((id, source, target));
        migrated += 1;
    }
    if errors.is_empty() {
        let transaction = conn
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        for (id, _, target) in &pending_updates {
            transaction
                .execute(
                    "UPDATE skills SET file_path = ?1 WHERE id = ?2",
                    rusqlite::params![target.to_string_lossy().to_string(), id],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        for (_, source, target) in pending_updates {
            if source != target {
                let _ = fs::remove_file(source);
            }
        }
    } else {
        for target in created_targets {
            let _ = fs::remove_file(target);
        }
        migrated = 0;
    }
    let requested_location = text_field(&target, &["kind", "type"])
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "cchub" | "unified" | "tool"));
    if errors.is_empty() {
        if let Some(location) = requested_location.as_deref() {
            conn.execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('skill_storage_location', ?1)",
                rusqlite::params![location],
            )
            .map_err(|error| error.to_string())?;
        }
    }
    let active_location = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'skill_storage_location'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .filter(|value| matches!(value.as_str(), "cchub" | "unified" | "tool"))
        .unwrap_or_else(|| "tool".to_string());
    Ok(json!({
        "target": target_dir.to_string_lossy(),
        "migrated": migrated,
        "skipped": skipped,
        "errors": errors,
        "location": active_location,
    }))
}

#[cfg(test)]
mod tests {
    use super::safe_skill_filename;

    #[test]
    fn skill_names_are_sanitized_without_path_escape() {
        assert_eq!(
            safe_skill_filename("code review"),
            Ok("code-review.md".to_string())
        );
        assert!(safe_skill_filename("../secret").is_err());
        assert!(safe_skill_filename("").is_err());
    }
}
