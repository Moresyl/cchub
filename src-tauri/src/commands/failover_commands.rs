use serde::Serialize;
use tauri::State;

use crate::db::DbState;

const QUEUE_PREFIX: &str = "proxy_failover_queue:";
const AUTO_FAILOVER_KEY: &str = "proxy_auto_failover_enabled";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverQueueItem {
    pub provider_id: String,
    pub provider_name: String,
    pub priority: usize,
    pub enabled: bool,
}

fn queue_key(app_type: &str) -> String {
    format!("{QUEUE_PREFIX}{app_type}")
}

fn read_queue(conn: &rusqlite::Connection, app_type: &str) -> Vec<String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        rusqlite::params![queue_key(app_type)],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
    .unwrap_or_default()
}

fn write_queue(
    conn: &rusqlite::Connection,
    app_type: &str,
    queue: &[String],
) -> Result<(), String> {
    let payload = serde_json::to_string(queue).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![queue_key(app_type), payload],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn validate_app_type(app_type: &str) -> Result<(), String> {
    if matches!(
        app_type,
        "claude" | "codex" | "gemini" | "grokbuild" | "opencode" | "openclaw" | "hermes"
    ) {
        Ok(())
    } else {
        Err(format!("Unsupported proxy app type: {app_type}"))
    }
}

#[tauri::command]
pub fn get_failover_queue(
    app_type: String,
    db: State<'_, DbState>,
) -> Result<Vec<FailoverQueueItem>, String> {
    validate_app_type(&app_type)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let configured = read_queue(&conn, &app_type);
    let mut stmt = conn
        .prepare("SELECT id, name FROM config_profiles WHERE tool_id = ?1 ORDER BY COALESCE(sort_order, 0), updated_at DESC")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![app_type], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?;
    let mut available = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut ordered = Vec::with_capacity(available.len());
    for id in configured {
        if let Some(index) = available.iter().position(|item| item.0 == id) {
            ordered.push(available.remove(index));
        }
    }
    ordered.extend(available);
    Ok(ordered
        .into_iter()
        .enumerate()
        .map(
            |(priority, (provider_id, provider_name))| FailoverQueueItem {
                provider_id,
                provider_name,
                priority,
                enabled: true,
            },
        )
        .collect())
}

#[tauri::command]
pub fn get_available_providers_for_failover(
    app_type: String,
    db: State<'_, DbState>,
) -> Result<Vec<FailoverQueueItem>, String> {
    get_failover_queue(app_type, db)
}

#[tauri::command]
pub fn add_to_failover_queue(
    app_type: String,
    provider_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    validate_app_type(&app_type)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let exists = conn
        .query_row(
            "SELECT 1 FROM config_profiles WHERE tool_id = ?1 AND id = ?2",
            rusqlite::params![app_type, provider_id],
            |_| Ok(()),
        )
        .is_ok();
    if !exists {
        return Err("Provider profile does not exist for this app".to_string());
    }
    let mut queue = read_queue(&conn, &app_type);
    if !queue.iter().any(|item| item == &provider_id) {
        queue.push(provider_id);
        write_queue(&conn, &app_type, &queue)?;
    }
    Ok(())
}

#[tauri::command]
pub fn remove_from_failover_queue(
    app_type: String,
    provider_id: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    validate_app_type(&app_type)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut queue = read_queue(&conn, &app_type);
    queue.retain(|item| item != &provider_id);
    write_queue(&conn, &app_type, &queue)
}

#[tauri::command]
pub fn set_failover_queue(
    app_type: String,
    provider_ids: Vec<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    validate_app_type(&app_type)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut seen = std::collections::HashSet::new();
    let mut queue = Vec::with_capacity(provider_ids.len());
    for provider_id in provider_ids {
        if !seen.insert(provider_id.clone()) {
            continue;
        }
        let exists = conn
            .query_row(
                "SELECT 1 FROM config_profiles WHERE tool_id = ?1 AND id = ?2",
                rusqlite::params![app_type, provider_id],
                |_| Ok(()),
            )
            .is_ok();
        if exists {
            queue.push(provider_id);
        }
    }
    write_queue(&conn, &app_type, &queue)
}

#[tauri::command]
pub fn get_auto_failover_enabled(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![AUTO_FAILOVER_KEY],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| serde_json::from_str::<bool>(&value).ok())
        .unwrap_or(true))
}

#[tauri::command]
pub fn set_auto_failover_enabled(enabled: bool, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let payload = serde_json::to_string(&enabled).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![AUTO_FAILOVER_KEY, payload],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_app_type;

    #[test]
    fn accepts_supported_proxy_apps() {
        assert!(validate_app_type("claude").is_ok());
        assert!(validate_app_type("codex").is_ok());
        assert!(validate_app_type("grokbuild").is_ok());
    }

    #[test]
    fn rejects_unknown_proxy_apps() {
        assert!(validate_app_type("terminal").is_err());
    }
}
