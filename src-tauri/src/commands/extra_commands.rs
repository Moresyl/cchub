use crate::db::DbState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClient {
    pub id: String,
    pub name: String,
    pub config_path: String,
    pub server_access: HashMap<String, bool>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub id: i64,
    pub server_id: String,
    pub server_name: String,
    pub request_type: String,
    pub status: String,
    pub latency_ms: Option<i64>,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapDay {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub base_path: Option<String>,
    pub is_active: bool,
    pub created_at: Option<String>,
}

// ── MCP Clients ──

#[tauri::command]
pub fn get_mcp_clients(db: State<'_, DbState>) -> Result<Vec<McpClient>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, config_path, server_access, created_at FROM mcp_clients ORDER BY name")
        .map_err(|e| e.to_string())?;

    let clients = stmt
        .query_map([], |row| {
            let access_json: String = row.get(3)?;
            let server_access: HashMap<String, bool> =
                serde_json::from_str(&access_json).unwrap_or_default();
            Ok(McpClient {
                id: row.get(0)?,
                name: row.get(1)?,
                config_path: row.get(2)?,
                server_access,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(clients)
}

#[tauri::command]
pub fn create_mcp_client(
    name: String,
    config_path: Option<String>,
    db: State<'_, DbState>,
) -> Result<McpClient, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let id = format!("client-{}", chrono::Utc::now().timestamp_millis());
    let now = chrono::Utc::now().to_rfc3339();
    let path = config_path.unwrap_or_default();

    conn.execute(
        "INSERT INTO mcp_clients (id, name, config_path, server_access, created_at) VALUES (?1, ?2, ?3, '{}', ?4)",
        rusqlite::params![id, name, path, now],
    ).map_err(|e| e.to_string())?;

    Ok(McpClient {
        id,
        name,
        config_path: path,
        server_access: HashMap::new(),
        created_at: Some(now),
    })
}

#[tauri::command]
pub fn update_mcp_client_access(
    id: String,
    server_access: HashMap<String, bool>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let access_json = serde_json::to_string(&server_access).unwrap_or_else(|_| "{}".to_string());
    conn.execute(
        "UPDATE mcp_clients SET server_access = ?1 WHERE id = ?2",
        rusqlite::params![access_json, id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_mcp_client(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM mcp_clients WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Activity Logs ──

#[tauri::command]
pub fn get_activity_logs(date: String, db: State<'_, DbState>) -> Result<Vec<ActivityItem>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.server_id, COALESCE(s.name, a.server_id), a.request_type, a.status, a.latency_ms, a.recorded_at
             FROM activity_logs a LEFT JOIN mcp_servers s ON a.server_id = s.id
             WHERE a.recorded_at LIKE ?1
             ORDER BY a.recorded_at DESC LIMIT 200",
        )
        .map_err(|e| e.to_string())?;

    let items = stmt
        .query_map([format!("{}%", date)], |row| {
            Ok(ActivityItem {
                id: row.get(0)?,
                server_id: row.get(1)?,
                server_name: row.get(2)?,
                request_type: row.get(3)?,
                status: row.get(4)?,
                latency_ms: row.get(5)?,
                recorded_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(items)
}

#[tauri::command]
pub fn get_activity_heatmap(days: i64, db: State<'_, DbState>) -> Result<Vec<HeatmapDay>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT substr(recorded_at, 1, 10) as day, COUNT(*) as cnt
             FROM activity_logs
             WHERE recorded_at >= date('now', ?1)
             GROUP BY day ORDER BY day",
        )
        .map_err(|e| e.to_string())?;

    let offset = format!("-{} days", days);
    let heatmap = stmt
        .query_map([offset], |row| {
            Ok(HeatmapDay {
                date: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(heatmap)
}

// ── Workspaces ──

#[tauri::command]
pub fn get_workspaces(db: State<'_, DbState>) -> Result<Vec<Workspace>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, description, base_path, is_active, created_at FROM workspaces ORDER BY name")
        .map_err(|e| e.to_string())?;

    let workspaces = stmt
        .query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                base_path: row.get(3)?,
                is_active: row.get::<_, i32>(4)? == 1,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(workspaces)
}

#[tauri::command]
pub fn create_workspace(
    name: String,
    description: Option<String>,
    base_path: Option<String>,
    db: State<'_, DbState>,
) -> Result<Workspace, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let id = format!("ws-{}", chrono::Utc::now().timestamp_millis());
    let now = chrono::Utc::now().to_rfc3339();

    // Check if any workspaces exist, if not make this one active
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
        .unwrap_or(0);
    let is_active = count == 0;

    conn.execute(
        "INSERT INTO workspaces (id, name, description, base_path, is_active, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, name, description, base_path, is_active as i32, now],
    ).map_err(|e| e.to_string())?;

    Ok(Workspace {
        id,
        name,
        description,
        base_path,
        is_active,
        created_at: Some(now),
    })
}

#[tauri::command]
pub fn switch_workspace(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE workspaces SET is_active = 0", [])
        .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE workspaces SET is_active = 1 WHERE id = ?1",
        rusqlite::params![id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_workspace(
    id: String,
    name: String,
    description: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE workspaces SET name = ?1, description = ?2 WHERE id = ?3",
        rusqlite::params![name, description, id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_workspace(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    // Cannot delete active workspace
    let is_active: i32 = conn
        .query_row(
            "SELECT is_active FROM workspaces WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if is_active == 1 {
        return Err("Cannot delete active workspace".to_string());
    }

    conn.execute("DELETE FROM workspaces WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Custom Paths ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPath {
    pub tool_id: String,
    pub config_dir: Option<String>,
    pub mcp_config_path: Option<String>,
    pub skills_dir: Option<String>,
}

#[tauri::command]
pub fn get_custom_paths(db: State<'_, DbState>) -> Result<Vec<CustomPath>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT tool_id, config_dir, mcp_config_path, skills_dir FROM custom_paths")
        .map_err(|e| e.to_string())?;

    let paths = stmt
        .query_map([], |row| {
            Ok(CustomPath {
                tool_id: row.get(0)?,
                config_dir: row.get(1)?,
                mcp_config_path: row.get(2)?,
                skills_dir: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(paths)
}

#[tauri::command]
pub fn save_custom_path(
    tool_id: String,
    config_dir: Option<String>,
    mcp_config_path: Option<String>,
    skills_dir: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO custom_paths (tool_id, config_dir, mcp_config_path, skills_dir) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![tool_id, config_dir, mcp_config_path, skills_dir],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_custom_path(tool_id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM custom_paths WHERE tool_id = ?1", rusqlite::params![tool_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Config Profiles ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigProfile {
    pub id: String,
    pub name: String,
    pub tool_id: String,
    pub config_snapshot: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[tauri::command]
pub fn get_config_profiles(db: State<'_, DbState>) -> Result<Vec<ConfigProfile>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, tool_id, config_snapshot, created_at, updated_at FROM config_profiles ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;

    let profiles = stmt
        .query_map([], |row| {
            Ok(ConfigProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                tool_id: row.get(2)?,
                config_snapshot: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(profiles)
}

#[tauri::command]
pub fn save_config_profile(
    name: String,
    tool_id: String,
    config_snapshot: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        rusqlite::params![id, name, tool_id, config_snapshot, now],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn apply_config_profile(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (tool_id, snapshot): (String, String) = conn
        .query_row(
            "SELECT tool_id, config_snapshot FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    // Determine target config path
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let config_path = match tool_id.as_str() {
        "claude" => home.join(".claude").join("settings.json"),
        "codex" => home.join(".codex").join("config.toml"),
        "gemini" => home.join(".gemini").join("settings.json"),
        "cursor" => home.join(".cursor").join("mcp.json"),
        "windsurf" => home.join(".windsurf").join("mcp.json"),
        "opencode" => home.join(".opencode").join("opencode.json"),
        _ => return Err(format!("Unknown tool: {}", tool_id)),
    };

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    crate::utils::atomic_write_string(&config_path, &snapshot).map_err(|e| e.to_string())?;

    // Update timestamp
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE config_profiles SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    ).map_err(|e| e.to_string())?;

    crate::db::record_activity(&conn, &tool_id, "profile_switch", "success", None);
    Ok(())
}

#[tauri::command]
pub fn delete_config_profile(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM config_profiles WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Read a tool's current config file content (for saving as profile snapshot)
#[tauri::command]
pub fn read_tool_config(tool_id: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let config_path = match tool_id.as_str() {
        "claude" => home.join(".claude").join("settings.json"),
        "codex" => home.join(".codex").join("config.toml"),
        "gemini" => home.join(".gemini").join("settings.json"),
        "cursor" => home.join(".cursor").join("mcp.json"),
        "windsurf" => home.join(".windsurf").join("mcp.json"),
        "opencode" => home.join(".opencode").join("opencode.json"),
        _ => return Err(format!("Unknown tool: {}", tool_id)),
    };

    if !config_path.exists() {
        return Err(format!("Config file not found: {}", config_path.display()));
    }

    std::fs::read_to_string(&config_path).map_err(|e| e.to_string())
}
