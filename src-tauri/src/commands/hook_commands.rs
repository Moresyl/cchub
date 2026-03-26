use crate::db::models::Hook;
use crate::db::DbState;
use crate::hooks::manager;
use tauri::State;

const KNOWN_PROJECT_ROOTS_KEY: &str = "known_project_roots";

fn remember_project_root(
    conn: &rusqlite::Connection,
    project_path: Option<&str>,
) -> Result<(), String> {
    let Some(project_path) = project_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![KNOWN_PROJECT_ROOTS_KEY],
            |row| row.get(0),
        )
        .ok();

    let mut roots: Vec<String> = existing
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();

    if !roots.iter().any(|value| value.eq_ignore_ascii_case(project_path)) {
        roots.push(project_path.to_string());
        let payload = serde_json::to_string(&roots).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![KNOWN_PROJECT_ROOTS_KEY, payload],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn scan_hooks(db: State<'_, DbState>) -> Result<Vec<Hook>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(manager::read_hooks_from_settings(&conn))
}

#[tauri::command]
pub fn get_hooks(db: State<'_, DbState>) -> Result<Vec<Hook>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, event, matcher, command, scope, project_path, enabled FROM hooks")
        .map_err(|e| e.to_string())?;

    let hooks = stmt
        .query_map([], |row| {
            Ok(Hook {
                id: row.get(0)?,
                event: row.get(1)?,
                matcher: row.get(2)?,
                command: row.get(3)?,
                scope: row.get(4)?,
                project_path: row.get(5)?,
                source_event: None,
                source_index: None,
                enabled: row.get(6)?,
                timeout: None,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(hooks)
}

#[tauri::command]
pub fn create_hook(
    event: String,
    matcher: Option<String>,
    command: String,
    scope: String,
    project_path: Option<String>,
    db: State<'_, DbState>,
) -> Result<Hook, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO hooks (id, event, matcher, command, scope, project_path, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        rusqlite::params![id, event, matcher, command, scope, project_path],
    ).map_err(|e| e.to_string())?;

    Ok(Hook {
        id,
        event,
        matcher,
        command,
        scope,
        project_path,
        source_event: None,
        source_index: None,
        enabled: true,
        timeout: None,
    })
}

#[tauri::command]
pub fn update_hook(
    id: String,
    event: Option<String>,
    matcher: Option<String>,
    command: Option<String>,
    enabled: Option<bool>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    if let Some(event) = event {
        conn.execute("UPDATE hooks SET event = ?1 WHERE id = ?2", rusqlite::params![event, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(matcher) = matcher {
        conn.execute("UPDATE hooks SET matcher = ?1 WHERE id = ?2", rusqlite::params![matcher, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(command) = command {
        conn.execute("UPDATE hooks SET command = ?1 WHERE id = ?2", rusqlite::params![command, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(enabled) = enabled {
        conn.execute("UPDATE hooks SET enabled = ?1 WHERE id = ?2", rusqlite::params![enabled, id])
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn delete_hook(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM hooks WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn save_hook_to_settings(
    event: String,
    matcher: Option<String>,
    command: String,
    timeout: Option<u64>,
    scope: String,
    project_path: Option<String>,
    edit_index: Option<usize>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    manager::save_hook_to_settings(
        &event,
        matcher.as_deref(),
        &command,
        timeout,
        &scope,
        project_path.as_deref(),
        edit_index,
    )?;

    if scope == "project" {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        remember_project_root(&conn, project_path.as_deref())?;
    }

    Ok(())
}

#[tauri::command]
pub fn delete_hook_from_settings(
    event: String,
    index: usize,
    scope: String,
    project_path: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    manager::delete_hook_from_settings(
        &event,
        index,
        &scope,
        project_path.as_deref(),
    )?;

    if scope == "project" {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        remember_project_root(&conn, project_path.as_deref())?;
    }

    Ok(())
}
