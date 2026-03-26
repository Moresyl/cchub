use crate::db::DbState;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingImportedProjectRoot {
    pub project_root: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoRemapImportedProjectRootsResult {
    pub remapped_roots: usize,
    pub restored_files: usize,
    pub skipped_roots: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEnvironmentReport {
    pub tool_id: String,
    pub tool_name: String,
    pub cli_available: bool,
    pub cli_command: String,
    pub config_path: String,
    pub config_exists: bool,
    pub mcp_config_path: String,
    pub mcp_config_exists: bool,
    pub skills_dir: String,
    pub skills_dir_exists: bool,
    pub config_dir: String,
    pub config_dir_exists: bool,
    pub has_custom_config_dir: bool,
    pub has_custom_mcp_config_path: bool,
    pub has_custom_skills_dir: bool,
    pub manual_setup_kind: Option<String>,
    pub manual_setup_command: Option<String>,
    pub manual_setup_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapToolEnvironmentResult {
    pub created_dirs: usize,
    pub created_files: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastImportSummary {
    pub imported_at: String,
    pub db_rows_restored: usize,
    pub tool_configs_restored: usize,
    pub skills_restored: usize,
    pub full_files_restored: usize,
    pub pending_project_files: usize,
    pub safety_backup_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullRescanResult {
    pub mcp_servers: usize,
    pub skills: usize,
    pub hooks: usize,
    pub instruction_files: usize,
    pub workflows: usize,
    pub config_roots: usize,
    pub pending_project_roots: usize,
    pub tool_health_issues: usize,
    pub manual_setup_required: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairAllResult {
    pub remapped_roots: usize,
    pub restored_project_files: usize,
    pub skipped_remap_roots: usize,
    pub bootstrapped_tools: usize,
    pub created_dirs: usize,
    pub created_files: usize,
    pub bootstrap_notes: Vec<String>,
    pub rescan: FullRescanResult,
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
    base_path: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let previous_base_path: Option<String> = conn
        .query_row(
            "SELECT base_path FROM workspaces WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    let normalized_next_base_path = base_path
        .as_deref()
        .and_then(normalize_project_root_path)
        .map(str::to_string);

    conn.execute(
        "UPDATE workspaces SET name = ?1, description = ?2, base_path = ?3 WHERE id = ?4",
        rusqlite::params![name, description, base_path, id],
    ).map_err(|e| e.to_string())?;

    if let Some(next_base_path) = normalized_next_base_path.as_deref() {
        sync_known_project_root(&conn, previous_base_path.as_deref(), Some(next_base_path))?;

        if let Some(previous_base_path) = previous_base_path
            .as_deref()
            .and_then(normalize_project_root_path)
        {
            if !project_root_paths_match(previous_base_path, next_base_path) {
                let _ = apply_project_root_remap(
                    &conn,
                    previous_base_path,
                    next_base_path,
                )?;
            }
        }
    }

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
    pub source_type: Option<String>,
    pub source_key: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

fn tool_config_file_name(tool_id: &str) -> Result<&'static str, String> {
    match tool_id {
        "claude" => Ok("settings.json"),
        "codex" => Ok("config.toml"),
        "gemini" => Ok("settings.json"),
        "opencode" => Ok("opencode.json"),
        "openclaw" => Ok("openclaw.json"),
        _ => Err(format!("Unknown tool: {}", tool_id)),
    }
}

fn default_tool_config_dir(home: &std::path::Path, tool_id: &str) -> Result<PathBuf, String> {
    let dir = match tool_id {
        "claude" => ".claude",
        "codex" => ".codex",
        "gemini" => ".gemini",
        "opencode" => ".opencode",
        "openclaw" => ".openclaw",
        _ => return Err(format!("Unknown tool: {}", tool_id)),
    };
    Ok(home.join(dir))
}

fn resolve_tool_config_dir(conn: &rusqlite::Connection, tool_id: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let custom_dir: Option<String> = conn
        .query_row(
            "SELECT config_dir FROM custom_paths WHERE tool_id = ?1",
            rusqlite::params![tool_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(dir) = custom_dir.filter(|dir| !dir.trim().is_empty()) {
        return Ok(PathBuf::from(dir));
    }

    let custom_config_path: Option<String> = conn
        .query_row(
            "SELECT mcp_config_path FROM custom_paths WHERE tool_id = ?1",
            rusqlite::params![tool_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(path) = custom_config_path.filter(|path| !path.trim().is_empty()) {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            return Ok(parent.to_path_buf());
        }
    }

    default_tool_config_dir(&home, tool_id)
}

fn resolve_tool_config_path(conn: &rusqlite::Connection, tool_id: &str) -> Result<PathBuf, String> {
    Ok(resolve_tool_config_dir(conn, tool_id)?.join(tool_config_file_name(tool_id)?))
}

fn resolve_claude_paths(conn: &rusqlite::Connection) -> Result<(PathBuf, PathBuf), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let custom_dir: Option<String> = conn
        .query_row(
            "SELECT config_dir FROM custom_paths WHERE tool_id = 'claude'",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let settings_json = if let Some(dir) = custom_dir.filter(|dir| !dir.trim().is_empty()) {
        PathBuf::from(dir).join("settings.json")
    } else {
        home.join(".claude").join("settings.json")
    };

    let custom_mcp_path: Option<String> = conn
        .query_row(
            "SELECT mcp_config_path FROM custom_paths WHERE tool_id = 'claude'",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let claude_json = if let Some(path) = custom_mcp_path.filter(|path| !path.trim().is_empty()) {
        PathBuf::from(path)
    } else {
        home.join(".claude.json")
    };

    Ok((claude_json, settings_json))
}

fn resolve_tool_skills_dir(conn: &rusqlite::Connection, tool_id: &str) -> Result<PathBuf, String> {
    let custom_skills_dir: Option<String> = conn
        .query_row(
            "SELECT skills_dir FROM custom_paths WHERE tool_id = ?1",
            rusqlite::params![tool_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(dir) = custom_skills_dir.filter(|dir| !dir.trim().is_empty()) {
        return Ok(PathBuf::from(dir));
    }

    Ok(resolve_tool_config_dir(conn, tool_id)?.join("skills"))
}

fn tool_cli_command(tool_id: &str) -> &'static str {
    match tool_id {
        "claude" => "claude",
        "codex" => "codex",
        "gemini" => "gemini",
        "opencode" => "opencode",
        "openclaw" => "openclaw",
        _ => "",
    }
}

fn is_executable_file(path: &std::path::Path) -> bool {
    path.is_file()
}

fn cli_exists_in_path(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }

    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };

    let path_exts: Vec<String> = if cfg!(windows) {
        std::env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| vec![".EXE".to_string(), ".CMD".to_string(), ".BAT".to_string(), ".COM".to_string()])
    } else {
        Vec::new()
    };

    for dir in std::env::split_paths(&path_var) {
        let direct = dir.join(command);
        if is_executable_file(&direct) {
            return true;
        }

        if cfg!(windows) {
            for ext in &path_exts {
                let ext = ext.trim();
                if ext.is_empty() {
                    continue;
                }
                let normalized_ext = if ext.starts_with('.') {
                    ext.to_string()
                } else {
                    format!(".{}", ext)
                };
                let candidate = dir.join(format!("{command}{normalized_ext}"));
                if is_executable_file(&candidate) {
                    return true;
                }
            }
        }
    }

    false
}

fn write_default_file_if_missing(
    path: &std::path::Path,
    content: &str,
    created_files: &mut usize,
) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(path, content).map_err(|e| e.to_string())?;
    *created_files += 1;
    Ok(())
}

fn ensure_dir_exists(path: &std::path::Path, created_dirs: &mut usize) -> Result<(), String> {
    if path.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    *created_dirs += 1;
    Ok(())
}

fn bootstrap_tool_environment_from_conn(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<BootstrapToolEnvironmentResult, String> {
    let mut created_dirs = 0usize;
    let mut created_files = 0usize;
    let mut notes = Vec::new();

    let config_dir = resolve_tool_config_dir(conn, tool_id)?;
    ensure_dir_exists(&config_dir, &mut created_dirs)?;

    let skills_dir = resolve_tool_skills_dir(conn, tool_id)?;
    ensure_dir_exists(&skills_dir, &mut created_dirs)?;

    match tool_id {
        "claude" => {
            let (claude_json_path, settings_json_path) = resolve_claude_paths(conn)?;
            if let Some(parent) = claude_json_path.parent() {
                ensure_dir_exists(parent, &mut created_dirs)?;
            }
            if let Some(parent) = settings_json_path.parent() {
                ensure_dir_exists(parent, &mut created_dirs)?;
            }
            write_default_file_if_missing(&claude_json_path, "{}\n", &mut created_files)?;
            write_default_file_if_missing(&settings_json_path, "{}\n", &mut created_files)?;
        }
        "codex" => {
            write_default_file_if_missing(&config_dir.join("config.toml"), "", &mut created_files)?;
            write_default_file_if_missing(&config_dir.join("auth.json"), "{}\n", &mut created_files)?;
            notes.push("Codex CLI 仍需登录后 auth.json 才会真正可用".to_string());
        }
        "gemini" => {
            write_default_file_if_missing(&config_dir.join("settings.json"), "{}\n", &mut created_files)?;
            write_default_file_if_missing(
                &config_dir.join(".env"),
                "# Add GEMINI_API_KEY=...\n",
                &mut created_files,
            )?;
            notes.push("Gemini CLI 仍需在 .env 中填写 GEMINI_API_KEY".to_string());
        }
        "opencode" => {
            write_default_file_if_missing(&config_dir.join("opencode.json"), "{}\n", &mut created_files)?;
        }
        "openclaw" => {
            write_default_file_if_missing(&config_dir.join("openclaw.json"), "{}\n", &mut created_files)?;
        }
        _ => return Err(format!("Unknown tool: {}", tool_id)),
    }

    Ok(BootstrapToolEnvironmentResult {
        created_dirs,
        created_files,
        notes,
    })
}

fn json_file_has_content(path: &std::path::Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };

    match value {
        serde_json::Value::Object(map) => !map.is_empty(),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.trim().is_empty(),
        _ => true,
    }
}

fn gemini_env_has_api_key(path: &std::path::Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };

    content.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            return false;
        };

        key.trim() == "GEMINI_API_KEY"
            && !value.trim().is_empty()
            && value.trim() != "..."
    })
}

fn open_target_in_system(target: &str) -> Result<(), String> {
    if target.trim().is_empty() {
        return Err("Target is empty".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform".to_string())
}

fn set_json_app_setting<T: Serialize>(
    conn: &rusqlite::Connection,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let payload = serde_json::to_string(value).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, payload],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn get_json_app_setting<T: for<'de> Deserialize<'de>>(
    conn: &rusqlite::Connection,
    key: &str,
) -> Result<Option<T>, String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .ok();

    match raw {
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

fn candidate_home_dirs() -> Vec<PathBuf> {
    let mut homes = Vec::new();

    if let Some(home) = dirs::home_dir() {
        homes.push(home);
    }

    for key in ["USERPROFILE", "HOME"] {
        if let Ok(value) = std::env::var(key) {
            let path = PathBuf::from(value);
            if !homes.iter().any(|item| item == &path) {
                homes.push(path);
            }
        }
    }

    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        let home = PathBuf::from(format!("{}{}", drive, path));
        if !homes.iter().any(|item| item == &home) {
            homes.push(home);
        }
    }

    #[cfg(target_family = "unix")]
    {
        let mnt_root = PathBuf::from("/mnt");
        if mnt_root.exists() {
            if let Ok(drives) = std::fs::read_dir(&mnt_root) {
                for drive in drives.flatten() {
                    let users_dir = drive.path().join("Users");
                    if !users_dir.exists() {
                        continue;
                    }
                    if let Ok(users) = std::fs::read_dir(users_dir) {
                        for user in users.flatten() {
                            let home = user.path();
                            if !homes.iter().any(|item| item == &home) {
                                homes.push(home);
                            }
                        }
                    }
                }
            }
        }
    }

    homes
}

fn compatible_db_paths() -> Vec<PathBuf> {
    let compat_dir = [".cc", "switch"].join("-");
    let compat_db = ["cc", "switch.db"].join("-");

    candidate_home_dirs()
        .into_iter()
        .map(|home| home.join(&compat_dir).join(&compat_db))
        .filter(|path| path.exists())
        .collect()
}

fn current_profile_setting_key(tool_id: &str) -> String {
    format!("current_config_profile:{}", tool_id)
}

fn get_stored_current_profile_ids(conn: &rusqlite::Connection) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM app_settings WHERE key LIKE 'current_config_profile:%'")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut current = HashMap::new();
    for row in rows {
        let (key, value) = row.map_err(|e| e.to_string())?;
        if let Some(tool_id) = key.strip_prefix("current_config_profile:") {
            current.insert(tool_id.to_string(), value);
        }
    }

    Ok(current)
}

fn get_compatible_current_profile_ids() -> Result<HashMap<String, String>, String> {
    let mut current = HashMap::new();

    for db_path in compatible_db_paths() {
        let external = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;

        let mut stmt = external
            .prepare(
                "SELECT id, app_type
                 FROM providers
                 WHERE is_current = 1 AND app_type IN ('claude', 'codex', 'gemini')",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (provider_id, tool_id) = row.map_err(|e| e.to_string())?;
            current.insert(tool_id.clone(), format!("compat-{}-{}", tool_id, provider_id));
        }
    }

    Ok(current)
}

fn normalize_external_profile_snapshot(tool_id: &str, settings_config: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(settings_config).ok()?;

    match tool_id {
        "claude" | "codex" | "gemini" => serde_json::to_string_pretty(&value).ok(),
        _ => None,
    }
}

fn upsert_synced_profile(
    conn: &rusqlite::Connection,
    id: &str,
    name: &str,
    tool_id: &str,
    config_snapshot: &str,
    source_type: &str,
    source_key: Option<&str>,
    now: &str,
) -> Result<(), String> {
    let existing_source_type: Option<String> = conn
        .query_row(
            "SELECT source_type FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .ok();

    if existing_source_type.as_deref() == Some("manual") {
        return Ok(());
    }

    if existing_source_type.is_some() {
        conn.execute(
            "UPDATE config_profiles
             SET name = ?1, tool_id = ?2, config_snapshot = ?3, source_type = ?4, source_key = ?5, updated_at = ?6
             WHERE id = ?7",
            rusqlite::params![name, tool_id, config_snapshot, source_type, source_key, now, id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO config_profiles
             (id, name, tool_id, config_snapshot, source_type, source_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            rusqlite::params![id, name, tool_id, config_snapshot, source_type, source_key, now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn sync_profiles_from_compatible_databases(
    conn: &rusqlite::Connection,
    now: &str,
) -> Result<HashMap<String, usize>, String> {
    let mut counts = HashMap::new();
    let mut seen_ids = std::collections::HashSet::new();

    for db_path in compatible_db_paths() {
        let external = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;

        let mut stmt = external
            .prepare(
                "SELECT id, app_type, name, settings_config
                 FROM providers
                 WHERE app_type IN ('claude', 'codex', 'gemini')
                 ORDER BY app_type, name",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (provider_id, tool_id, name, settings_config) = row.map_err(|e| e.to_string())?;
            let Some(config_snapshot) = normalize_external_profile_snapshot(&tool_id, &settings_config) else {
                continue;
            };
            let id = format!("compat-{}-{}", tool_id, provider_id);
            let source_key = format!("{}#{}", db_path.display(), provider_id);

            upsert_synced_profile(
                conn,
                &id,
                &name,
                &tool_id,
                &config_snapshot,
                "compatible",
                Some(&source_key),
                now,
            )?;

            *counts.entry(tool_id).or_insert(0) += 1;
            seen_ids.insert(id);
        }
    }

    let mut stale_stmt = conn
        .prepare("SELECT id FROM config_profiles WHERE source_type = 'compatible'")
        .map_err(|e| e.to_string())?;
    let stale_ids: Vec<String> = stale_stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .filter(|id: &String| !seen_ids.contains(id))
        .collect();

    for id in stale_ids {
        conn.execute("DELETE FROM config_profiles WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;
    }

    Ok(counts)
}

fn sync_live_profiles(
    conn: &rusqlite::Connection,
    imported_counts: &HashMap<String, usize>,
    now: &str,
) -> Result<(), String> {
    for tool_id in ["claude", "codex", "gemini", "opencode", "openclaw"] {
        let id = format!("live-{}", tool_id);

        if imported_counts.get(tool_id).copied().unwrap_or(0) > 0 {
            conn.execute("DELETE FROM config_profiles WHERE id = ?1", rusqlite::params![id])
                .map_err(|e| e.to_string())?;
            continue;
        }

        match read_tool_snapshot(conn, tool_id) {
            Ok(config_snapshot) => {
                let name = format!("{} 当前配置", tool_id);
                upsert_synced_profile(
                    conn,
                    &id,
                    &name,
                    tool_id,
                    &config_snapshot,
                    "live",
                    Some(tool_id),
                    now,
                )?;
            }
            Err(_) => {
                conn.execute("DELETE FROM config_profiles WHERE id = ?1", rusqlite::params![id])
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

fn config_contents_match(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }

    match (
        serde_json::from_str::<serde_json::Value>(left),
        serde_json::from_str::<serde_json::Value>(right),
    ) {
        (Ok(mut a), Ok(mut b)) => {
            // Strip metadata keys used for claude profile splitting
            for key in &["__claude_json_keys__", "__settings_json_keys__"] {
                a.as_object_mut().map(|o| o.remove(*key));
                b.as_object_mut().map(|o| o.remove(*key));
            }
            a == b
        }
        _ => left.trim() == right.trim(),
    }
}

fn read_tool_snapshot(conn: &rusqlite::Connection, tool_id: &str) -> Result<String, String> {
    match tool_id {
        "codex" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            let auth_path = dir.join("auth.json");
            if !auth_path.exists() {
                return Err(format!("Config file not found: {}", auth_path.display()));
            }
            let auth: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&auth_path).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let config_path = dir.join("config.toml");
            let config = if config_path.exists() {
                std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?
            } else {
                String::new()
            };
            serde_json::to_string_pretty(&serde_json::json!({
                "auth": auth,
                "config": config,
            }))
            .map_err(|e| e.to_string())
        }
        "gemini" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            let env_path = dir.join(".env");
            if !env_path.exists() {
                return Err(format!("Config file not found: {}", env_path.display()));
            }
            let env_text = std::fs::read_to_string(&env_path).map_err(|e| e.to_string())?;
            let env: HashMap<String, String> = env_text.lines()
                .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .filter_map(|l| l.split_once('=').map(|(k,v)| (k.trim().to_string(), v.trim().to_string())))
                .collect();
            let settings_path = dir.join("settings.json");
            let config = if settings_path.exists() {
                serde_json::from_str::<serde_json::Value>(
                    &std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?
            } else {
                serde_json::json!({})
            };
            serde_json::to_string_pretty(&serde_json::json!({
                "env": env,
                "config": config,
            }))
            .map_err(|e| e.to_string())
        }
        "claude" => {
            let (claude_json, settings_json) = resolve_claude_paths(conn)?;

            let claude_json_obj: serde_json::Map<String, serde_json::Value> = if claude_json.exists() {
                std::fs::read_to_string(&claude_json).ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .and_then(|v| v.as_object().cloned())
                    .unwrap_or_default()
            } else { serde_json::Map::new() };

            let settings_json_obj: serde_json::Map<String, serde_json::Value> = if settings_json.exists() {
                std::fs::read_to_string(&settings_json).ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .and_then(|v| v.as_object().cloned())
                    .unwrap_or_default()
            } else { serde_json::Map::new() };

            if claude_json_obj.is_empty() && settings_json_obj.is_empty() {
                return Err("No Claude config found".to_string());
            }

            // Store both sources separately so apply can split them back
            let claude_json_keys: Vec<String> = claude_json_obj.keys().cloned().collect();
            let settings_json_keys: Vec<String> = settings_json_obj.keys().cloned().collect();

            let mut combined = claude_json_obj;
            for (k, v) in settings_json_obj {
                if !combined.contains_key(&k) { combined.insert(k, v); }
            }
            combined.insert("__claude_json_keys__".to_string(), serde_json::json!(claude_json_keys));
            combined.insert("__settings_json_keys__".to_string(), serde_json::json!(settings_json_keys));

            serde_json::to_string_pretty(&serde_json::Value::Object(combined)).map_err(|e| e.to_string())
        }
        _ => {
            let config_path = resolve_tool_config_path(conn, tool_id)?;
            if !config_path.exists() {
                return Err(format!("Config file not found: {}", config_path.display()));
            }
            std::fs::read_to_string(&config_path).map_err(|e| e.to_string())
        }
    }
}

fn apply_tool_snapshot(conn: &rusqlite::Connection, tool_id: &str, snapshot: &str) -> Result<(), String> {
    match tool_id {
        "codex" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let auth_path = dir.join("auth.json");
            let config_path = dir.join("config.toml");

            if let Ok(value) = serde_json::from_str::<serde_json::Value>(snapshot) {
                if let (Some(auth), Some(config)) = (value.get("auth"), value.get("config").and_then(|v| v.as_str())) {
                    let auth_text = serde_json::to_string_pretty(auth).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&auth_path, &auth_text).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&config_path, config).map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }

            crate::utils::atomic_write_string(&config_path, snapshot).map_err(|e| e.to_string())
        }
        "gemini" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let env_path = dir.join(".env");
            let settings_path = dir.join("settings.json");

            if let Ok(value) = serde_json::from_str::<serde_json::Value>(snapshot) {
                if let (Some(env), Some(config)) = (value.get("env").and_then(|v| v.as_object()), value.get("config")) {
                    let env_map: std::collections::HashMap<String, String> = env
                        .iter()
                        .filter_map(|(key, value)| value.as_str().map(|v| (key.clone(), v.to_string())))
                        .collect();
                    let env_text = env_map.iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let config_text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&env_path, &env_text).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&settings_path, &config_text).map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }

            crate::utils::atomic_write_string(&settings_path, snapshot).map_err(|e| e.to_string())
        }
        "claude" => {
            let (claude_json_path, settings_json_path) = resolve_claude_paths(conn)?;

            let snap: serde_json::Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
            let snap_obj = snap.as_object().ok_or("Invalid claude snapshot")?;

            // Determine which keys belong to which file
            let claude_json_keys: std::collections::HashSet<String> = snap_obj
                .get("__claude_json_keys__")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let settings_json_keys: std::collections::HashSet<String> = snap_obj
                .get("__settings_json_keys__")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            // Keys that should be preserved in settings.json during profile switch
            let preserve_keys: std::collections::HashSet<&str> = [
                "statusLine", "enabledPlugins", "mcpServers",
            ].iter().copied().collect();

            // Split snapshot fields back to their original files
            let mut claude_data = serde_json::Map::new();
            let mut settings_data = serde_json::Map::new();

            for (k, v) in snap_obj {
                if k == "__claude_json_keys__" || k == "__settings_json_keys__" {
                    continue;
                }
                if !claude_json_keys.is_empty() || !settings_json_keys.is_empty() {
                    // We have source metadata — use it
                    if claude_json_keys.contains(k) {
                        claude_data.insert(k.clone(), v.clone());
                    }
                    if settings_json_keys.contains(k) {
                        settings_data.insert(k.clone(), v.clone());
                    }
                    // Key in neither list (shouldn't happen) — try settings
                    if !claude_json_keys.contains(k) && !settings_json_keys.contains(k) {
                        settings_data.insert(k.clone(), v.clone());
                    }
                } else {
                    // Legacy snapshot without metadata — use known-settings heuristic
                    let settings_known = [
                        "permissions", "skipDangerousModePermissionPrompt",
                        "alwaysThinkingEnabled", "attribution", "autoUpdatesChannel",
                        "statusLine", "enabledPlugins", "mcpServers", "env",
                    ];
                    if settings_known.contains(&k.as_str()) {
                        settings_data.insert(k.clone(), v.clone());
                    } else {
                        claude_data.insert(k.clone(), v.clone());
                    }
                }
            }

            // Write .claude.json — merge with existing
            if !claude_data.is_empty() {
                let mut existing: serde_json::Map<String, serde_json::Value> = if claude_json_path.exists() {
                    std::fs::read_to_string(&claude_json_path).ok()
                        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                        .and_then(|v| v.as_object().cloned())
                        .unwrap_or_default()
                } else { serde_json::Map::new() };
                for (k, v) in claude_data {
                    existing.insert(k, v);
                }
                let text = serde_json::to_string_pretty(&serde_json::Value::Object(existing)).map_err(|e| e.to_string())?;
                crate::utils::atomic_write_string(&claude_json_path, &text).map_err(|e| e.to_string())?;
            }

            // Write settings.json — merge, preserving protected keys
            {
                let mut existing: serde_json::Map<String, serde_json::Value> = if settings_json_path.exists() {
                    std::fs::read_to_string(&settings_json_path).ok()
                        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                        .and_then(|v| v.as_object().cloned())
                        .unwrap_or_default()
                } else { serde_json::Map::new() };
                for (k, v) in settings_data {
                    if preserve_keys.contains(k.as_str()) {
                        // Don't overwrite preserved keys — keep current value
                        continue;
                    }
                    existing.insert(k, v);
                }
                std::fs::create_dir_all(settings_json_path.parent().unwrap()).map_err(|e| e.to_string())?;
                let text = serde_json::to_string_pretty(&serde_json::Value::Object(existing)).map_err(|e| e.to_string())?;
                crate::utils::atomic_write_string(&settings_json_path, &text).map_err(|e| e.to_string())?;
            }

            Ok(())
        }
        _ => {
            let config_path = resolve_tool_config_path(conn, tool_id)?;
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            crate::utils::atomic_write_string(&config_path, snapshot).map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub fn sync_config_profiles(db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let imported_counts = sync_profiles_from_compatible_databases(&conn, &now)?;
    sync_live_profiles(&conn, &imported_counts, &now)?;
    Ok(())
}

#[tauri::command]
pub fn get_config_profiles(db: State<'_, DbState>) -> Result<Vec<ConfigProfile>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, tool_id, config_snapshot, source_type, source_key, created_at, updated_at FROM config_profiles ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;

    let profiles = stmt
        .query_map([], |row| {
            Ok(ConfigProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                tool_id: row.get(2)?,
                config_snapshot: row.get(3)?,
                source_type: row.get(4)?,
                source_key: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
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
        "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, source_type, source_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'manual', NULL, ?5, ?5)",
        rusqlite::params![id, name, tool_id, config_snapshot, now],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn update_config_profile(
    id: String,
    name: String,
    config_snapshot: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let tool_id: String = conn
        .query_row(
            "SELECT tool_id FROM config_profiles WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    conn.execute(
        "UPDATE config_profiles SET name = ?1, config_snapshot = ?2, source_type = 'manual', source_key = NULL, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![name, config_snapshot, now, id],
    )
    .map_err(|e| e.to_string())?;

    let setting_key = current_profile_setting_key(&tool_id);
    let active_profile_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
            |row| row.get(0),
        )
        .ok();

    if active_profile_id.as_deref() == Some(id.as_str()) {
        apply_tool_snapshot(&conn, &tool_id, &config_snapshot)?;
    }

    Ok(())
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

    apply_tool_snapshot(&conn, &tool_id, &snapshot)?;

    // Update timestamp
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE config_profiles SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![current_profile_setting_key(&tool_id), id],
    ).map_err(|e| e.to_string())?;

    crate::db::record_activity(&conn, &tool_id, "profile_switch", "success", None);
    Ok(())
}

#[tauri::command]
pub fn delete_config_profile(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (tool_id, source_type): (String, Option<String>) = conn
        .query_row(
            "SELECT tool_id, source_type FROM config_profiles WHERE id = ?1",
            rusqlite::params![&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    if source_type.as_deref() != Some("manual") {
        return Err("Only manual profiles can be deleted".to_string());
    }

    conn.execute("DELETE FROM config_profiles WHERE id = ?1", rusqlite::params![&id])
        .map_err(|e| e.to_string())?;

    let setting_key = current_profile_setting_key(&tool_id);
    let stored_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![&setting_key],
            |row| row.get(0),
        )
        .ok();
    if stored_id.as_deref() == Some(id.as_str()) {
        conn.execute("DELETE FROM app_settings WHERE key = ?1", rusqlite::params![setting_key])
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_active_config_profile_ids(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, tool_id, config_snapshot, source_type, source_key, created_at, updated_at FROM config_profiles ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;
    let profiles: Vec<ConfigProfile> = stmt
        .query_map([], |row| {
            Ok(ConfigProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                tool_id: row.get(2)?,
                config_snapshot: row.get(3)?,
                source_type: row.get(4)?,
                source_key: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    let mut active_ids = Vec::new();
    let stored_current = get_stored_current_profile_ids(&conn)?;
    let compatible_current = get_compatible_current_profile_ids().unwrap_or_default();
    let mut cache: HashMap<String, Option<String>> = HashMap::new();
    let mut resolved_tools = std::collections::HashSet::new();

    for profile in &profiles {
        if resolved_tools.contains(&profile.tool_id) {
            continue;
        }

        let preferred_id = stored_current
            .get(&profile.tool_id)
            .or_else(|| compatible_current.get(&profile.tool_id));

        if let Some(preferred_id) = preferred_id {
            if profiles.iter().any(|item| item.tool_id == profile.tool_id && item.id == *preferred_id) {
                active_ids.push(preferred_id.clone());
                resolved_tools.insert(profile.tool_id.clone());
            }
        }
    }

    for profile in profiles {
        if resolved_tools.contains(&profile.tool_id) {
            continue;
        }

        if !cache.contains_key(&profile.tool_id) {
            let content = read_tool_snapshot(&conn, &profile.tool_id).ok();
            cache.insert(profile.tool_id.clone(), content);
        }

        if cache
            .get(&profile.tool_id)
            .and_then(|value| value.as_ref())
            .is_some_and(|value| config_contents_match(value, &profile.config_snapshot))
        {
            active_ids.push(profile.id);
            resolved_tools.insert(profile.tool_id.clone());
        }
    }

    Ok(active_ids)
}

// ── Proxy Settings ──

/// Set HTTP/HTTPS proxy for all network requests (persisted to database)
#[tauri::command]
pub fn set_proxy(proxy_url: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    if proxy_url.trim().is_empty() {
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("https_proxy");
        conn.execute("INSERT OR REPLACE INTO app_settings (key, value) VALUES ('proxy_url', '')", [])
            .map_err(|e| e.to_string())?;
    } else {
        let url = proxy_url.trim().to_string();
        std::env::set_var("HTTP_PROXY", &url);
        std::env::set_var("HTTPS_PROXY", &url);
        std::env::set_var("http_proxy", &url);
        std::env::set_var("https_proxy", &url);
        conn.execute("INSERT OR REPLACE INTO app_settings (key, value) VALUES ('proxy_url', ?1)", rusqlite::params![url])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Get current proxy setting
#[tauri::command]
pub fn get_proxy(db: State<'_, DbState>) -> String {
    // Read from database first (persisted), fallback to env
    if let Ok(conn) = db.0.lock() {
        if let Ok(proxy) = conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'proxy_url'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            if !proxy.is_empty() {
                return proxy;
            }
        }
    }
    std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .unwrap_or_default()
}

/// Open a native folder picker dialog and return the selected path
#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select folder")
        .pick_folder()
        .await;
    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Open a native file picker dialog and return the selected path
#[tauri::command]
pub async fn pick_file() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Select file")
        .add_filter("Config", &["json", "toml", "yaml", "yml"])
        .pick_file()
        .await;
    Ok(file.map(|f| f.path().to_string_lossy().to_string()))
}

/// Read a tool's current config file content
#[tauri::command]
pub fn read_tool_config(tool_id: String, db: State<'_, DbState>) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_tool_snapshot(&conn, &tool_id)
}

/// Get Claude Code permissions level (0=strict, 1=standard, 2=relaxed, 3=bypass)
#[tauri::command]
pub fn get_claude_permissions_level() -> Result<u32, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() { return Ok(0); }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let mode = settings.get("permissions")
        .and_then(|p| p.get("defaultMode"))
        .and_then(|m| m.as_str())
        .unwrap_or("");

    if mode == "bypassPermissions" {
        return Ok(3);
    }

    let allow = settings.get("permissions")
        .and_then(|p| p.get("allow"))
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    if allow.iter().any(|s| *s == "Bash(*)") && allow.iter().any(|s| *s == "Write(*)") {
        Ok(2)
    } else if allow.iter().any(|s| *s == "Read(*)") {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Set Claude Code permissions level (0=strict, 1=standard, 2=relaxed, 3=bypass)
#[tauri::command]
pub fn set_claude_permissions_level(level: u32) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");

    let mut settings: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };

    let (allow, mode, skip_prompt): (Vec<&str>, &str, bool) = match level {
        0 => (vec![], "normal", false),
        1 => (vec![
            "Read(*)", "Glob(*)", "Grep(*)", "WebSearch(*)",
        ], "normal", false),
        2 => (vec![
            "Read(*)", "Write(*)", "Edit(*)", "Glob(*)", "Grep(*)",
            "WebFetch(*)", "WebSearch(*)", "Agent(*)", "NotebookEdit(*)",
        ], "normal", false),
        3 => (vec![
            "Bash(*)", "Read(*)", "Write(*)", "Edit(*)", "Glob(*)", "Grep(*)",
            "WebFetch(*)", "WebSearch(*)", "Agent(*)", "NotebookEdit(*)",
            "Skill(*)", "mcp__*",
        ], "bypassPermissions", true),
        _ => return Err("Invalid level".to_string()),
    };

    let allow_arr: Vec<serde_json::Value> = allow.iter().map(|s| serde_json::json!(s)).collect();
    settings["permissions"] = serde_json::json!({
        "allow": allow_arr,
        "deny": [],
        "defaultMode": mode,
    });
    settings["skipDangerousModePermissionPrompt"] = serde_json::json!(skip_prompt);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get Claude Code auto-update channel
#[tauri::command]
pub fn get_claude_auto_update() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() { return Ok("latest".to_string()); }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(settings.get("autoUpdatesChannel").and_then(|v| v.as_str()).unwrap_or("latest").to_string())
}

/// Set Claude Code auto-update channel
#[tauri::command]
pub fn set_claude_auto_update(channel: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).map_err(|e| e.to_string())?
    } else { serde_json::json!({}) };
    if channel == "disabled" {
        settings.as_object_mut().map(|o| o.remove("autoUpdatesChannel"));
    } else {
        settings["autoUpdatesChannel"] = serde_json::json!(channel);
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get Codex CLI settings (approval_mode, reasoning_effort, disable_response_storage)
#[tauri::command]
pub fn get_codex_settings() -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".codex").join("config.toml");
    if !path.exists() {
        return Ok(serde_json::json!({ "approval_mode": "suggest", "reasoning_effort": "medium", "disable_response_storage": false }));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let doc: toml::Value = content.parse().map_err(|e: toml::de::Error| e.to_string())?;

    // Read approval mode from personality or dedicated field
    let personality = doc.get("personality").and_then(|v| v.as_str()).unwrap_or("pragmatic");
    let approval_mode = if personality == "full-auto" { "full-auto" }
        else if personality == "auto-edit" { "auto-edit" }
        else { "suggest" };

    let reasoning = doc.get("model_reasoning_effort").and_then(|v| v.as_str()).unwrap_or("medium");
    let disable_storage = doc.get("disable_response_storage").and_then(|v| v.as_bool()).unwrap_or(false);

    Ok(serde_json::json!({
        "approval_mode": approval_mode,
        "reasoning_effort": reasoning,
        "disable_response_storage": disable_storage,
    }))
}

/// Set a Codex CLI setting
#[tauri::command]
pub fn set_codex_setting(key: String, value: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".codex").join("config.toml");

    let content = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else { String::new() };

    let mut doc: toml_edit::DocumentMut = content.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;

    match key.as_str() {
        "approval_mode" => {
            // Codex doesn't have approval_mode directly, map to personality
            doc["personality"] = toml_edit::value(&value);
        }
        "reasoning_effort" => {
            doc["model_reasoning_effort"] = toml_edit::value(&value);
        }
        "disable_response_storage" => {
            doc["disable_response_storage"] = toml_edit::value(value == "true");
        }
        _ => return Err(format!("Unknown setting: {}", key)),
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &doc.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get Claude Code model setting
#[tauri::command]
pub fn get_claude_model() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() { return Ok("".to_string()); }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(settings.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string())
}

/// Set Claude Code model
#[tauri::command]
pub fn set_claude_model(model: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).map_err(|e| e.to_string())?
    } else { serde_json::json!({}) };
    settings["model"] = serde_json::json!(model);
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get Claude Code Tool Search (ENABLE_TOOL_SEARCH) status from settings.local.json
#[tauri::command]
pub fn get_claude_tool_search() -> Result<bool, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.local.json");
    if !path.exists() { return Ok(false); }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let enabled = settings.get("env")
        .and_then(|e| e.get("ENABLE_TOOL_SEARCH"))
        .and_then(|v| v.as_str())
        .map(|s| s == "true")
        .unwrap_or(false);
    Ok(enabled)
}

/// Set Claude Code Tool Search (ENABLE_TOOL_SEARCH) in settings.local.json
#[tauri::command]
pub fn set_claude_tool_search(enabled: bool) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.local.json");

    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).unwrap_or(serde_json::json!({}))
    } else { serde_json::json!({}) };

    if settings.get("env").is_none() {
        settings["env"] = serde_json::json!({});
    }
    if enabled {
        settings["env"]["ENABLE_TOOL_SEARCH"] = serde_json::json!("true");
    } else {
        if let Some(env) = settings.get_mut("env").and_then(|e| e.as_object_mut()) {
            env.remove("ENABLE_TOOL_SEARCH");
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

// ── StatusLine (claude-hud) ──

/// Check if claude-hud plugin is installed and return its status + config
#[tauri::command]
pub fn get_claude_hud_status() -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let cache_dir = home.join(".claude").join("plugins").join("cache").join("claude-hud").join("claude-hud");

    // Find installed version by looking for dist/index.js
    let mut installed = false;
    let mut version = String::new();
    let mut index_js_path = String::new();

    if cache_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let ver_dir = entry.path();
                let candidate = ver_dir.join("dist").join("index.js");
                if candidate.exists() {
                    installed = true;
                    version = entry.file_name().to_string_lossy().to_string();
                    index_js_path = candidate.to_string_lossy().to_string();
                    break;
                }
            }
        }
    }

    // Check if statusLine is enabled in settings.json
    let settings_path = home.join(".claude").join("settings.json");
    let statusline_enabled = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path).unwrap_or_default();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
        settings.get("statusLine").and_then(|s| s.get("command")).and_then(|c| c.as_str()).is_some()
    } else {
        false
    };

    // Read claude-hud config
    let hud_config_path = home.join(".claude").join("plugins").join("claude-hud").join("config.json");
    let hud_config = if hud_config_path.exists() {
        let content = std::fs::read_to_string(&hud_config_path).unwrap_or_default();
        serde_json::from_str::<serde_json::Value>(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    Ok(serde_json::json!({
        "installed": installed,
        "version": version,
        "indexJsPath": index_js_path,
        "statuslineEnabled": statusline_enabled,
        "hudConfig": hud_config,
    }))
}

/// Install claude-hud plugin by creating the necessary directory structure and downloading
#[tauri::command]
pub async fn install_claude_hud(db: State<'_, crate::db::DbState>) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    // Create plugin directory structure
    let version = "0.0.6";
    let dist_dir = home.join(".claude").join("plugins").join("cache")
        .join("claude-hud").join("claude-hud").join(version).join("dist");
    std::fs::create_dir_all(&dist_dir).map_err(|e| e.to_string())?;

    // Build HTTP client with proxy support
    let proxy_url = get_proxy(db);
    let client = if !proxy_url.is_empty() {
        let proxy = reqwest::Proxy::all(&proxy_url)
            .map_err(|e| format!("Invalid proxy: {}", e))?;
        reqwest::Client::builder().proxy(proxy).build()
            .map_err(|e| format!("Client build failed: {}", e))?
    } else {
        reqwest::Client::new()
    };

    // Try official npm registry first, fallback to China mirror
    let registries = [
        format!("https://registry.npmjs.org/claude-hud/-/claude-hud-{}.tgz", version),
        format!("https://registry.npmmirror.com/claude-hud/-/claude-hud-{}.tgz", version),
    ];

    let mut bytes = None;
    let mut last_err = String::new();
    for url in &registries {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.bytes().await {
                    Ok(b) => { bytes = Some(b); break; }
                    Err(e) => { last_err = format!("Read failed: {}", e); }
                }
            }
            Ok(resp) => { last_err = format!("HTTP {} from {}", resp.status(), url); }
            Err(e) => { last_err = format!("Download failed: {}", e); }
        }
    }
    let bytes = bytes.ok_or(format!("All registries failed: {}", last_err))?;

    // Extract tgz: decompress gzip then untar
    let gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(gz);
    let entries = archive.entries().map_err(|e| format!("Tar read failed: {}", e))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Tar entry error: {}", e))?;
        let entry_path = entry.path().map_err(|e| format!("Path error: {}", e))?.to_path_buf();
        let entry_str = entry_path.to_string_lossy().to_string();

        // npm tarballs have files under package/dist/
        if entry_str.starts_with("package/dist/") {
            let relative = entry_str.strip_prefix("package/dist/").unwrap_or(&entry_str);
            if relative.is_empty() { continue; }
            let target = dist_dir.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut file = std::fs::File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
        }
    }

    // Verify index.js exists
    let index_js = dist_dir.join("index.js");
    if !index_js.exists() {
        return Err("Installation failed: index.js not found after extraction".to_string());
    }

    // Create default hud config
    let hud_config_dir = home.join(".claude").join("plugins").join("claude-hud");
    std::fs::create_dir_all(&hud_config_dir).map_err(|e| e.to_string())?;
    let hud_config_path = hud_config_dir.join("config.json");
    if !hud_config_path.exists() {
        let default_config = serde_json::json!({
            "layout": "separators",
            "pathLevels": 2,
            "gitStatus": {
                "enabled": true,
                "showDirty": true,
                "showAheadBehind": false,
                "showFileStats": false
            },
            "display": {
                "showModel": true,
                "showContextBar": true,
                "showConfigCounts": true,
                "showDuration": true,
                "showUsage": true,
                "usageBarEnabled": true,
                "showTokenBreakdown": true,
                "showTools": true,
                "showAgents": true,
                "showTodos": true
            }
        });
        let config_str = serde_json::to_string_pretty(&default_config).map_err(|e| e.to_string())?;
        crate::utils::atomic_write_string(&hud_config_path, &config_str).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Enable or disable statusLine in settings.json
#[tauri::command]
pub fn set_claude_statusline(enabled: bool) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");

    let mut settings: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };

    if enabled {
        // Find the index.js path
        let cache_dir = home.join(".claude").join("plugins").join("cache").join("claude-hud").join("claude-hud");
        let mut index_path = String::new();
        if cache_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    let candidate = entry.path().join("dist").join("index.js");
                    if candidate.exists() {
                        // Use ~ relative path for cross-platform compatibility
                        let ver = entry.file_name().to_string_lossy().to_string();
                        index_path = format!("~/.claude/plugins/cache/claude-hud/claude-hud/{}/dist/index.js", ver);
                        break;
                    }
                }
            }
        }
        if index_path.is_empty() {
            return Err("claude-hud not installed".to_string());
        }

        settings["statusLine"] = serde_json::json!({
            "type": "command",
            "command": format!("node {}", index_path)
        });

        // Also enable the plugin
        if settings.get("enabledPlugins").is_none() {
            settings["enabledPlugins"] = serde_json::json!({});
        }
        settings["enabledPlugins"]["claude-hud@claude-hud"] = serde_json::json!(true);
    } else {
        if let Some(obj) = settings.as_object_mut() {
            obj.remove("statusLine");
        }
        if let Some(plugins) = settings.get_mut("enabledPlugins").and_then(|p| p.as_object_mut()) {
            plugins.remove("claude-hud@claude-hud");
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Update claude-hud config.json
#[tauri::command]
pub fn set_claude_hud_config(config: serde_json::Value) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let config_dir = home.join(".claude").join("plugins").join("claude-hud");
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let config_path = config_dir.join("config.json");
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&config_path, &content).map_err(|e| e.to_string())?;
    Ok(())
}

const SQL_BACKUP_MARKER: &str = "-- CCHub Database Backup (.sql)";

/// Escape a string value for SQL: replace ' with ''
fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}

fn path_is_within(path: &std::path::Path, root: &std::path::Path) -> bool {
    path.starts_with(root)
}

fn collect_backup_file_rows(
    base_path: &std::path::Path,
    root_key: &str,
    relative_prefix: &std::path::Path,
    rows: &mut Vec<(String, String, String)>,
) {
    let entries = match std::fs::read_dir(base_path) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let next_relative = relative_prefix.join(name);

        if path.is_dir() {
            collect_backup_file_rows(&path, root_key, &next_relative, rows);
            continue;
        }

        if !path.is_file() {
            continue;
        }

        if let Ok(bytes) = std::fs::read(&path) {
            let relative = next_relative.to_string_lossy().replace('\\', "/");
            let content_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            rows.push((root_key.to_string(), relative, content_base64));
        }
    }
}

fn collect_backup_entry_row(
    path: &std::path::Path,
    root_key: &str,
    relative_path: &std::path::Path,
    rows: &mut Vec<(String, String, String)>,
) {
    if !path.is_file() {
        return;
    }

    if let Ok(bytes) = std::fs::read(path) {
        let relative = relative_path.to_string_lossy().replace('\\', "/");
        let content_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        rows.push((root_key.to_string(), relative, content_base64));
    }
}

fn discover_project_roots(conn: &rusqlite::Connection) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    let mut push_root = |raw_path: String| {
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            return;
        }

        let key = trimmed.replace('\\', "/");
        if !seen.insert(key) {
            return;
        }

        let path = PathBuf::from(trimmed);
        if path.exists() {
            roots.push(path);
        }
    };

    if let Ok(mut stmt) = conn.prepare("SELECT base_path FROM workspaces WHERE base_path IS NOT NULL AND trim(base_path) != ''") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                push_root(row);
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare("SELECT project_path FROM hooks WHERE project_path IS NOT NULL AND trim(project_path) != ''") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                push_root(row);
            }
        }
    }

    let known_roots: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'known_project_roots'",
            [],
            |row| row.get(0),
        )
        .ok();
    if let Some(raw) = known_roots {
        if let Ok(paths) = serde_json::from_str::<Vec<String>>(&raw) {
            for path in paths {
                push_root(path);
            }
        }
    }

    roots
}

fn normalize_project_root_path(path: &str) -> Option<&str> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.trim_end_matches(['\\', '/']))
    }
}

fn project_root_paths_match(left: &str, right: &str) -> bool {
    normalize_project_root_path(left)
        .zip(normalize_project_root_path(right))
        .is_some_and(|(left, right)| left.replace('\\', "/").eq_ignore_ascii_case(&right.replace('\\', "/")))
}

fn sync_known_project_root(
    conn: &rusqlite::Connection,
    previous_path: Option<&str>,
    next_path: Option<&str>,
) -> Result<(), String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'known_project_roots'",
            [],
            |row| row.get(0),
        )
        .ok();

    let mut roots: Vec<String> = existing
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();

    if let Some(previous_path) = previous_path.and_then(normalize_project_root_path) {
        roots.retain(|value| !project_root_paths_match(value, previous_path));
    }

    if let Some(next_path) = next_path.and_then(normalize_project_root_path) {
        if !roots.iter().any(|value| project_root_paths_match(value, next_path)) {
            roots.push(next_path.to_string());
        }
    }

    let payload = serde_json::to_string(&roots).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('known_project_roots', ?1)",
        rusqlite::params![payload],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn restore_imported_project_root_snapshot(
    conn: &rusqlite::Connection,
    source_path: &str,
    target_path: &str,
) -> Result<usize, String> {
    let Some(source_root) = normalize_project_root_path(source_path) else {
        return Ok(0);
    };
    let Some(target_root) = normalize_project_root_path(target_path) else {
        return Ok(0);
    };

    let mut stmt = conn
        .prepare(
            "SELECT relative_path, content_base64
             FROM imported_project_files
             WHERE project_root = ?1
             ORDER BY relative_path",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![source_root], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let files: Vec<(String, String)> = rows.filter_map(|row| row.ok()).collect();
    if files.is_empty() {
        return Ok(0);
    }

    let target_root_path = PathBuf::from(target_root);
    let mut restored = 0usize;

    for (relative_path, content_base64) in &files {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(content_base64)
            .map_err(|e| e.to_string())?;
        let target_path =
            target_root_path.join(relative_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&target_path, bytes).map_err(|e| e.to_string())?;
        restored += 1;
    }

    if !project_root_paths_match(source_root, target_root) {
        conn.execute(
            "INSERT OR REPLACE INTO imported_project_files (project_root, relative_path, content_base64)
             SELECT ?1, relative_path, content_base64
             FROM imported_project_files
             WHERE project_root = ?2",
            rusqlite::params![target_root, source_root],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM imported_project_files WHERE project_root = ?1",
            rusqlite::params![source_root],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(restored)
}

fn store_imported_project_file(
    conn: &rusqlite::Connection,
    project_root: &str,
    relative_path: &str,
    content_base64: &str,
) -> Result<(), String> {
    let Some(project_root) = normalize_project_root_path(project_root) else {
        return Ok(());
    };

    conn.execute(
        "INSERT OR REPLACE INTO imported_project_files (project_root, relative_path, content_base64)
         VALUES (?1, ?2, ?3)",
        rusqlite::params![project_root, relative_path, content_base64],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn apply_project_root_remap(
    conn: &rusqlite::Connection,
    source_path: &str,
    target_path: &str,
) -> Result<usize, String> {
    let Some(source_root) = normalize_project_root_path(source_path) else {
        return Ok(0);
    };
    let Some(target_root) = normalize_project_root_path(target_path) else {
        return Ok(0);
    };

    if project_root_paths_match(source_root, target_root) {
        return Ok(0);
    }

    conn.execute(
        "UPDATE hooks SET project_path = ?1 WHERE project_path = ?2",
        rusqlite::params![target_root, source_root],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE workspaces SET base_path = ?1 WHERE base_path = ?2",
        rusqlite::params![target_root, source_root],
    )
    .map_err(|e| e.to_string())?;
    sync_known_project_root(conn, Some(source_root), Some(target_root))?;

    restore_imported_project_root_snapshot(conn, source_root, target_root)
}

fn get_pending_imported_project_roots_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<PendingImportedProjectRoot>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT project_root, COUNT(*) as file_count
             FROM imported_project_files
             GROUP BY project_root
             ORDER BY project_root",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PendingImportedProjectRoot {
                project_root: row.get(0)?,
                file_count: row.get::<_, i64>(1)? as usize,
            })
        })
        .map_err(|e| e.to_string())?;

    Ok(rows
        .filter_map(|row| row.ok())
        .filter(|item| !PathBuf::from(&item.project_root).exists())
        .collect())
}

fn project_root_match_key(path: &str) -> Option<String> {
    let normalized = normalize_project_root_path(path)?;
    let file_name = PathBuf::from(normalized).file_name()?.to_string_lossy().to_string();
    if file_name.trim().is_empty() {
        None
    } else {
        Some(file_name.to_ascii_lowercase())
    }
}

fn normalized_path_segments(path: &str) -> Vec<String> {
    normalize_project_root_path(path)
        .map(|value| {
            value
                .replace('\\', "/")
                .split('/')
                .filter(|segment| !segment.trim().is_empty())
                .map(|segment| segment.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn shared_trailing_segment_count(left: &str, right: &str) -> usize {
    let left_segments = normalized_path_segments(left);
    let right_segments = normalized_path_segments(right);
    let mut count = 0usize;

    for (left, right) in left_segments.iter().rev().zip(right_segments.iter().rev()) {
        if left == right {
            count += 1;
        } else {
            break;
        }
    }

    count
}

fn best_project_root_candidate<'a>(
    pending_path: &str,
    candidates: &'a [String],
) -> Option<&'a String> {
    let pending_key = project_root_match_key(pending_path)?;
    let mut scored: Vec<(&String, usize)> = candidates
        .iter()
        .filter(|candidate| project_root_match_key(candidate).as_deref() == Some(pending_key.as_str()))
        .map(|candidate| (candidate, shared_trailing_segment_count(pending_path, candidate)))
        .collect();

    if scored.is_empty() {
        return None;
    }

    scored.sort_by(|(left_path, left_score), (right_path, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_path.cmp(right_path))
    });

    let (best_path, best_score) = scored[0];
    if best_score == 0 {
        return None;
    }

    if scored.get(1).is_some_and(|(_, score)| *score == best_score) {
        return None;
    }

    Some(best_path)
}

fn build_tool_environment_report_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<ToolEnvironmentReport>, String> {
    let tools = crate::skills::tools::detect_tools();
    let mut reports = Vec::new();

    for tool in tools {
        let cli_command = tool_cli_command(&tool.id).to_string();
        let config_path = resolve_tool_config_path(conn, &tool.id)?.to_string_lossy().to_string();
        let mcp_config_path = if tool.id == "claude" {
            resolve_claude_paths(conn)?.0.to_string_lossy().to_string()
        } else {
            resolve_tool_config_path(conn, &tool.id)?.to_string_lossy().to_string()
        };
        let skills_dir = resolve_tool_skills_dir(conn, &tool.id)?.to_string_lossy().to_string();
        let config_dir = resolve_tool_config_dir(conn, &tool.id)?.to_string_lossy().to_string();

        let custom_row: Option<(Option<String>, Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT config_dir, mcp_config_path, skills_dir FROM custom_paths WHERE tool_id = ?1",
                rusqlite::params![&tool.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        let has_custom_config_dir = custom_row
            .as_ref()
            .and_then(|row| row.0.as_deref())
            .is_some_and(|value| !value.trim().is_empty());
        let has_custom_mcp_config_path = custom_row
            .as_ref()
            .and_then(|row| row.1.as_deref())
            .is_some_and(|value| !value.trim().is_empty());
        let has_custom_skills_dir = custom_row
            .as_ref()
            .and_then(|row| row.2.as_deref())
            .is_some_and(|value| !value.trim().is_empty());
        let mut manual_setup_kind = None;
        let mut manual_setup_command = None;
        let mut manual_setup_path = None;

        match tool.id.as_str() {
            "codex" => {
                let auth_path = PathBuf::from(&config_dir).join("auth.json");
                if !json_file_has_content(&auth_path) {
                    manual_setup_kind = Some("codex_login".to_string());
                    manual_setup_command = Some("codex".to_string());
                    manual_setup_path = Some(auth_path.to_string_lossy().to_string());
                }
            }
            "gemini" => {
                let env_path = PathBuf::from(&config_dir).join(".env");
                if !gemini_env_has_api_key(&env_path) {
                    manual_setup_kind = Some("gemini_api_key".to_string());
                    manual_setup_path = Some(env_path.to_string_lossy().to_string());
                }
            }
            _ => {}
        }

        reports.push(ToolEnvironmentReport {
            tool_id: tool.id,
            tool_name: tool.name,
            cli_available: cli_exists_in_path(&cli_command),
            cli_command,
            config_path: config_path.clone(),
            config_exists: PathBuf::from(&config_path).is_file(),
            mcp_config_path: mcp_config_path.clone(),
            mcp_config_exists: PathBuf::from(&mcp_config_path).is_file(),
            skills_dir: skills_dir.clone(),
            skills_dir_exists: PathBuf::from(&skills_dir).is_dir(),
            config_dir: config_dir.clone(),
            config_dir_exists: PathBuf::from(&config_dir).is_dir(),
            has_custom_config_dir,
            has_custom_mcp_config_path,
            has_custom_skills_dir,
            manual_setup_kind,
            manual_setup_command,
            manual_setup_path,
        });
    }

    Ok(reports)
}

fn refresh_mcp_servers_from_scan(conn: &rusqlite::Connection) -> Result<usize, String> {
    let scanned = crate::mcp::config::scan_all_mcp_servers();
    let now = chrono::Utc::now().to_rfc3339();

    for s in &scanned {
        let args_json = serde_json::to_string(&s.args).unwrap_or_else(|_| "[]".to_string());
        let env_json = serde_json::to_string(&s.env).unwrap_or_else(|_| "{}".to_string());

        let existing_status: Option<String> = conn
            .query_row(
                "SELECT status FROM mcp_servers WHERE id = ?1",
                rusqlite::params![s.name],
                |row| row.get(0),
            )
            .ok();

        let status = existing_status.unwrap_or_else(|| "active".to_string());

        conn.execute(
            "INSERT OR REPLACE INTO mcp_servers (id, name, command, args, env, transport, source, config_path, status, installed_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, COALESCE((SELECT installed_at FROM mcp_servers WHERE id = ?1), ?10), ?10)",
            rusqlite::params![s.name, s.name, s.command, args_json, env_json, s.transport, s.source, s.config_path, status, now],
        ).map_err(|e| e.to_string())?;
    }

    Ok(scanned.len())
}

fn run_full_rescan_from_conn(conn: &rusqlite::Connection) -> Result<FullRescanResult, String> {
    let mcp_servers = refresh_mcp_servers_from_scan(conn)?;
    let skills = crate::skills::scanner::scan_local_skills().len();
    let hooks = crate::hooks::manager::read_hooks_from_settings(conn).len();
    let instruction_files = crate::claude_md::manager::scan_claude_md_files(conn).len();
    let workflows = crate::workflows::scan_workflow_files().len();
    let config_roots = crate::commands::config_files_commands::get_config_roots()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|root| root.exists)
        .count();
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

fn auto_remap_imported_project_roots_from_conn(
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

        let Some(best_candidate) = best_project_root_candidate(&pending.project_root, candidates) else {
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

fn resolve_backup_root(conn: &rusqlite::Connection, root_key: &str) -> Result<PathBuf, String> {
    if root_key == "claude_mcp" {
        return Ok(resolve_claude_paths(conn)?.0);
    }

    if let Some(tool_id) = root_key.strip_prefix("tooldir:") {
        return resolve_tool_config_dir(conn, tool_id);
    }

    if let Some(tool_id) = root_key.strip_prefix("skillsdir:") {
        return resolve_tool_skills_dir(conn, tool_id);
    }

    if let Some(project_root) = root_key.strip_prefix("project:") {
        return Ok(PathBuf::from(project_root));
    }

    Err(format!("Unknown backup root: {}", root_key))
}

fn trim_utf8_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

fn validate_sql_backup_content(content: &str) -> Result<&str, String> {
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

fn configure_database_connection(
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

fn restore_proxy_env_from_conn(conn: &rusqlite::Connection) {
    let proxy = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'proxy_url'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .unwrap_or_default();

    if proxy.trim().is_empty() {
        for key in ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"] {
            std::env::remove_var(key);
        }
        return;
    }

    std::env::set_var("HTTP_PROXY", &proxy);
    std::env::set_var("HTTPS_PROXY", &proxy);
    std::env::set_var("http_proxy", &proxy);
    std::env::set_var("https_proxy", &proxy);
}

fn get_main_db_path(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    let mut stmt = conn
        .prepare("PRAGMA database_list")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows.flatten() {
        let (name, file) = row;
        if name == "main" && !file.trim().is_empty() {
            return Ok(PathBuf::from(file));
        }
    }

    Err("Cannot determine database path".to_string())
}

fn create_safety_db_backup(
    conn: &rusqlite::Connection,
    backup_path: &std::path::Path,
) -> Result<(), String> {
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    if backup_path.exists() {
        std::fs::remove_file(backup_path).map_err(|e| e.to_string())?;
    }

    let vacuum_sql = format!(
        "PRAGMA wal_checkpoint(TRUNCATE);\nVACUUM main INTO '{}';",
        sql_escape(&backup_path.to_string_lossy())
    );
    conn.execute_batch(&vacuum_sql).map_err(|e| e.to_string())
}

fn validate_imported_backup_tables(conn: &rusqlite::Connection) -> Result<(), String> {
    let backup_meta_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_backup_meta'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if backup_meta_exists == 0 {
        return Err("备份文件格式不正确，缺少 _backup_meta 表".to_string());
    }

    Ok(())
}

fn remove_db_sidecars(db_path: &std::path::Path) {
    let wal_path = db_path.with_extension(
        db_path
            .extension()
            .map(|ext| format!("{}-wal", ext.to_string_lossy()))
            .unwrap_or_else(|| "wal".to_string()),
    );
    let shm_path = db_path.with_extension(
        db_path
            .extension()
            .map(|ext| format!("{}-shm", ext.to_string_lossy()))
            .unwrap_or_else(|| "shm".to_string()),
    );

    let _ = std::fs::remove_file(wal_path);
    let _ = std::fs::remove_file(shm_path);
}

fn restore_imported_artifacts(
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

    if let Ok(mut stmt) = conn.prepare("SELECT tool_id, config_path, config_content FROM _tool_configs") {
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
                        crate::utils::atomic_write_string(&settings_json_path, &config_content).is_ok()
                    }
                    "claude" => {
                        let parsed = serde_json::from_str::<serde_json::Value>(&config_content).ok();
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
                            crate::utils::atomic_write_string(&claude_json_path, &config_content).is_ok()
                        }
                    }
                    "codex" | "gemini" | "opencode" | "openclaw" => {
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
                if crate::utils::atomic_write_string(&skills_dir.join(&name), &file_content).is_ok() {
                    skills_restored += 1;
                }
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare("SELECT root_key, relative_path, content_base64 FROM _backup_files ORDER BY id") {
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
                    store_imported_project_file(conn, project_root, &relative_path, &content_base64)?;
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
fn generate_sql_backup(conn: &rusqlite::Connection, home: &std::path::Path) -> String {
    let mut sql = String::new();

    // Header
    sql.push_str("-- ═══════════════════════════════════════════════════════\n");
    sql.push_str("-- CCHub Database Backup (.sql)\n");
    sql.push_str(&format!("-- Version: {}\n", env!("CARGO_PKG_VERSION")));
    sql.push_str(&format!("-- Created: {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
    sql.push_str("-- ═══════════════════════════════════════════════════════\n\n");

    // Schema (CREATE TABLE IF NOT EXISTS)
    sql.push_str("-- ── Schema ──\n\n");
    sql.push_str(&crate::db::schema::get_schema_sql());
    sql.push_str("\n");

    // Backup metadata table
    sql.push_str("CREATE TABLE IF NOT EXISTS _backup_meta (key TEXT PRIMARY KEY, value TEXT);\n");
    sql.push_str(&format!("INSERT OR REPLACE INTO _backup_meta VALUES ('version', '{}');\n", env!("CARGO_PKG_VERSION")));
    sql.push_str(&format!("INSERT OR REPLACE INTO _backup_meta VALUES ('created_at', '{}');\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));

    // Tool configs table
    sql.push_str("CREATE TABLE IF NOT EXISTS _tool_configs (tool_id TEXT PRIMARY KEY, config_path TEXT, config_content TEXT);\n");

    // Skill files table
    sql.push_str("CREATE TABLE IF NOT EXISTS _skill_files (id INTEGER PRIMARY KEY AUTOINCREMENT, tool_id TEXT, name TEXT, content TEXT);\n\n");
    sql.push_str("CREATE TABLE IF NOT EXISTS _backup_files (id INTEGER PRIMARY KEY AUTOINCREMENT, root_key TEXT, relative_path TEXT, content_base64 TEXT);\n\n");

    // Data dump for all 12 business tables
    sql.push_str("-- ── Data ──\n\n");
    let tables = ["mcp_servers", "plugins", "skills", "hooks", "activity_logs", "mcp_clients",
                   "workspaces", "custom_paths", "config_profiles", "app_settings",
                   "imported_project_files", "update_history", "metrics"];

    for table in tables {
        let query = format!("SELECT * FROM {}", table);
        if let Ok(mut stmt) = conn.prepare(&query) {
            let col_count = stmt.column_count();
            let col_names: Vec<String> = (0..col_count).map(|i| stmt.column_name(i).unwrap_or("").to_string()).collect();

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
                    sql.push_str(&format!("INSERT OR REPLACE INTO {} ({}) VALUES ({});\n",
                        table, col_names.join(", "), row.join(", ")));
                }
            }
            if has_rows {
                sql.push('\n');
            }
        }
    }

    // Tool config files
    sql.push_str("-- ── Tool Configs ──\n\n");
    let tool_ids = ["claude", "codex", "gemini", "opencode", "openclaw"];
    for tool_id in tool_ids {
        if let Ok(content) = read_tool_snapshot(conn, tool_id) {
            let config_path = match tool_id {
                "claude" => resolve_claude_paths(conn)
                    .map(|(claude_json, settings_json)| {
                        format!("{} | {}", claude_json.display(), settings_json.display())
                    })
                    .unwrap_or_else(|_| home.join(".claude").join("settings.json").display().to_string()),
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
                            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
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
            collect_backup_file_rows(root_path, root_key, std::path::Path::new(""), &mut backup_file_rows);
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
    let project_relative_dirs = [".claude", ".codex", ".gemini", ".opencode", ".openclaw"];

    for project_root in discover_project_roots(conn) {
        let root_key = format!("project:{}", project_root.to_string_lossy());

        for relative_file in project_relative_files {
            let relative_path = std::path::Path::new(relative_file);
            let absolute_path = project_root.join(relative_path);
            collect_backup_entry_row(&absolute_path, &root_key, relative_path, &mut backup_file_rows);
        }

        for relative_dir in project_relative_dirs {
            let relative_path = std::path::Path::new(relative_dir);
            let absolute_path = project_root.join(relative_path);
            if absolute_path.is_dir() {
                collect_backup_file_rows(&absolute_path, &root_key, relative_path, &mut backup_file_rows);
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

/// Export: generate .sql backup file
#[tauri::command]
pub async fn save_backup_to_file(db: State<'_, DbState>) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let sql_content = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        generate_sql_backup(&conn, &home)
    };

    let file = rfd::AsyncFileDialog::new()
        .set_title("导出备份")
        .set_file_name(&format!("cchub-backup-{}.sql", chrono::Local::now().format("%Y%m%d-%H%M%S")))
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
    let file_path = file.path().to_path_buf();
    let raw_content = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
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
            temp_conn.execute_batch(content).map_err(|e| e.to_string())?;
            crate::db::schema::run_migrations(&temp_conn).map_err(|e| e.to_string())?;
            validate_imported_backup_tables(&temp_conn)?;
        }

        let placeholder =
            rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?;
        let old_conn = std::mem::replace(&mut *conn, placeholder);
        drop(conn);

        if let Err((old_conn, err)) = old_conn.close() {
            let mut conn = db.0.lock().map_err(|e| e.to_string())?;
            *conn = old_conn;
            return Err(err.to_string());
        }

        temp_file
    };

    let import_result = (|| -> Result<(rusqlite::Connection, usize, usize, usize, usize, usize), String> {
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
        ) =
            restore_imported_artifacts(&reopened, restored_count)?;
        let now = chrono::Utc::now().to_rfc3339();
        let imported_counts = sync_profiles_from_compatible_databases(&reopened, &now)?;
        sync_live_profiles(&reopened, &imported_counts, &now)?;
        restore_proxy_env_from_conn(&reopened);

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
            drop(reopened_conn);
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
            restore_proxy_env_from_conn(&fallback);
            *conn = fallback;

            Err(err)
        }
    }
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
pub fn run_full_rescan(
    db: State<'_, DbState>,
) -> Result<FullRescanResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    run_full_rescan_from_conn(&conn)
}

#[tauri::command]
pub fn repair_all_migration_issues(
    db: State<'_, DbState>,
) -> Result<RepairAllResult, String> {
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
        assert_eq!(project_root_match_key("D:/work/foo-bar").as_deref(), Some("foo-bar"));
        assert_eq!(project_root_match_key("/tmp/demo/").as_deref(), Some("demo"));
        assert_eq!(project_root_match_key("   ").as_deref(), None);
    }

    #[test]
    fn shared_trailing_segments_counts_suffix_depth() {
        assert_eq!(
            shared_trailing_segment_count(
                "D:/old/workspace/acme/app",
                "E:/new/workspace/acme/app"
            ),
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
        let candidates = vec![
            "E:/new/a/app".to_string(),
            "F:/new/b/app".to_string(),
        ];

        let best = best_project_root_candidate("D:/old/c/app", &candidates)
            .map(|value| value.as_str());

        assert_eq!(best, None);
    }

    #[test]
    fn normalized_segments_ignore_empty_parts() {
        assert_eq!(
            normalized_path_segments("D:\\foo\\\\bar\\baz"),
            vec!["d:".to_string(), "foo".to_string(), "bar".to_string(), "baz".to_string()]
        );
    }
}
