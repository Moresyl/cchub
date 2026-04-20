use crate::copilot_auth::{self, CopilotAuthState};
use crate::db::DbState;
use crate::hermes;
use crate::utils::configure_background_command;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Component, PathBuf};
use tauri::{AppHandle, Manager, State};

fn log_command_timing(command: &str, started_at: std::time::Instant) {
    eprintln!(
        "[cchub][invoke] {command} completed in {}ms",
        started_at.elapsed().as_millis()
    );
}

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
#[serde(rename_all = "camelCase")]
pub struct ApplyConfigProfileResult {
    pub tool_id: String,
    pub profile_id: String,
    pub active_profile_ids: Vec<String>,
    pub applied_at: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedBackupFile {
    pub path: String,
    pub name: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub kind: String,
    pub can_restore: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPreferences {
    pub auto_backup_enabled: bool,
    pub retention_count: usize,
}

impl Default for BackupPreferences {
    fn default() -> Self {
        Self {
            auto_backup_enabled: false,
            retention_count: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPreferences {
    pub level: String,
}

impl Default for LogPreferences {
    fn default() -> Self {
        Self {
            level: "error".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileTargets {
    pub runtime_log_path: String,
    pub crash_log_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterEnvironmentState {
    pub disabled_by_env: bool,
    pub env_var_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProbeResult {
    pub profile_id: String,
    pub tool_id: String,
    pub provider_name: String,
    pub base_url: Option<String>,
    pub status: String,
    pub latency_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub checked_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStreamCheckResult {
    pub profile_id: String,
    pub tool_id: String,
    pub provider_name: String,
    pub base_url: Option<String>,
    pub status: String,
    pub latency_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub checked_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CommonConfigSnippet {
    pub hide_attribution: bool,
    pub enable_teammates: bool,
    pub effort_level_high: bool,
    pub enable_tool_search: bool,
    pub custom_values: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPingResult {
    pub profile_id: String,
    pub tool_id: String,
    pub provider_name: String,
    pub base_url: Option<String>,
    pub status: String,
    pub latency_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub checked_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeConfigToggles {
    pub hide_attribution: bool,
    pub enable_teammates: bool,
    pub max_thinking_tokens: bool,
    pub max_thinking_tokens_value: String,
    pub enable_tool_search: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTomlStructuredConfig {
    pub model_provider: String,
    pub provider_label: String,
    pub base_url: String,
    pub wire_api: String,
    pub model: String,
    pub reasoning_effort: String,
    pub personality: String,
    pub disable_response_storage: bool,
    pub model_context_window: String,
    pub model_auto_compact_token_limit: String,
    pub api_key: String,
    pub mcp_servers: Vec<String>,
    pub malformed_mcp_servers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawDailyMemoryEntry {
    pub path: String,
    pub file_name: String,
    pub source: String,
    pub project_name: Option<String>,
    pub modified_at: Option<String>,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub title: String,
    pub cwd: Option<String>,
    pub source_kind: String,
    pub source_backend: String,
    pub source_path: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub preview: String,
    pub message_count: usize,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub tokens_used: Option<u64>,
    pub search_hit_count: usize,
    pub can_resume: bool,
    pub can_delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    pub session: SessionSummary,
    pub entries: Vec<SessionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResumeResult {
    pub launched: bool,
    pub command: String,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeleteTarget {
    pub tool_id: String,
    pub session_id: String,
    pub source_path: String,
    pub source_backend: String,
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
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_mcp_client(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM mcp_clients WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Activity Logs ──

#[tauri::command]
pub fn get_activity_logs(
    date: String,
    db: State<'_, DbState>,
) -> Result<Vec<ActivityItem>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
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
    })();
    log_command_timing("get_activity_logs", started_at);
    result
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
    )
    .map_err(|e| e.to_string())?;
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
    )
    .map_err(|e| e.to_string())?;

    if let Some(next_base_path) = normalized_next_base_path.as_deref() {
        sync_known_project_root(&conn, previous_base_path.as_deref(), Some(next_base_path))?;

        if let Some(previous_base_path) = previous_base_path
            .as_deref()
            .and_then(normalize_project_root_path)
        {
            if !project_root_paths_match(previous_base_path, next_base_path) {
                let _ = apply_project_root_remap(&conn, previous_base_path, next_base_path)?;
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

    conn.execute(
        "DELETE FROM workspaces WHERE id = ?1",
        rusqlite::params![id],
    )
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
    conn.execute(
        "DELETE FROM custom_paths WHERE tool_id = ?1",
        rusqlite::params![tool_id],
    )
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
    pub sort_order: i64,
    pub source_type: Option<String>,
    pub source_key: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedConfigProfileInput {
    pub tool_id: String,
    pub config_snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigFragment {
    pub id: String,
    pub name: String,
    pub target_tools: Vec<String>,
    pub fields: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

fn tool_config_file_name(tool_id: &str) -> Result<&'static str, String> {
    match tool_id {
        "claude" => Ok("settings.json"),
        "codex" => Ok("config.toml"),
        "gemini" => Ok("settings.json"),
        "opencode" => Ok("opencode.json"),
        "openclaw" => Ok("openclaw.json"),
        "hermes" => Ok("config.yaml"),
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
        "hermes" => ".hermes",
        _ => return Err(format!("Unknown tool: {}", tool_id)),
    };
    Ok(home.join(dir))
}

fn resolve_tool_config_dir(conn: &rusqlite::Connection, tool_id: &str) -> Result<PathBuf, String> {
    if tool_id == "hermes" {
        return hermes::hermes_root(conn);
    }

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
    if tool_id == "hermes" {
        return hermes::config_path(conn);
    }
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
    if tool_id == "hermes" {
        return hermes::skills_dir(conn);
    }

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
        "hermes" => "hermes",
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
            .unwrap_or_else(|| {
                vec![
                    ".EXE".to_string(),
                    ".CMD".to_string(),
                    ".BAT".to_string(),
                    ".COM".to_string(),
                ]
            })
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

fn tool_label(tool_id: &str) -> &'static str {
    match tool_id {
        "claude" => "Claude",
        "codex" => "Codex",
        "gemini" => "Gemini",
        "opencode" => "OpenCode",
        "openclaw" => "OpenClaw",
        "hermes" => "Hermes",
        _ => "Session",
    }
}

fn tool_hidden_dir(tool_id: &str) -> Option<&'static str> {
    match tool_id {
        "claude" => Some(".claude"),
        "codex" => Some(".codex"),
        "gemini" => Some(".gemini"),
        "opencode" => Some(".opencode"),
        "openclaw" => Some(".openclaw"),
        "hermes" => Some(".hermes"),
        _ => None,
    }
}

fn format_unix_timestamp(value: i64) -> Option<String> {
    if value <= 0 {
        return None;
    }

    let (seconds, nanos) = if value > 10_000_000_000 {
        let seconds = value / 1000;
        let remainder = (value % 1000).unsigned_abs() as u32;
        (seconds, remainder.saturating_mul(1_000_000))
    } else {
        (value, 0)
    };

    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, nanos).map(|datetime| {
        datetime
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string()
    })
}

fn format_timestamp_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(parsed) = trimmed.parse::<i64>() {
        return format_unix_timestamp(parsed);
    }

    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(
            parsed
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string(),
        );
    }

    Some(trimmed.chars().take(19).collect())
}

fn truncate_session_text(text: &str, max_chars: usize) -> String {
    let condensed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if condensed.chars().count() <= max_chars {
        condensed
    } else {
        let mut result = condensed.chars().take(max_chars).collect::<String>();
        result.push_str("...");
        result
    }
}

fn count_query_hits(query: &str, values: &[String]) -> usize {
    if query.is_empty() {
        return 0;
    }

    values
        .iter()
        .filter(|value| value.to_lowercase().contains(query))
        .count()
}

#[derive(Debug, Default, Clone, Copy)]
struct SessionTokenTotals {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    has_usage: bool,
}

impl SessionTokenTotals {
    fn record(&mut self, input_tokens: Option<u64>, output_tokens: Option<u64>, total_tokens: Option<u64>) {
        let resolved_input = input_tokens.unwrap_or(0);
        let resolved_output = output_tokens.unwrap_or(0);
        let resolved_total = total_tokens.unwrap_or_else(|| resolved_input.saturating_add(resolved_output));

        if resolved_input == 0 && resolved_output == 0 && resolved_total == 0 {
            return;
        }

        self.input_tokens = self.input_tokens.saturating_add(resolved_input);
        self.output_tokens = self.output_tokens.saturating_add(resolved_output);
        self.total_tokens = self.total_tokens.saturating_add(resolved_total);
        self.has_usage = true;
    }

    fn input_option(self) -> Option<u64> {
        self.has_usage.then_some(self.input_tokens)
    }

    fn output_option(self) -> Option<u64> {
        self.has_usage.then_some(self.output_tokens)
    }

    fn total_option(self) -> Option<u64> {
        self.has_usage.then_some(self.total_tokens)
    }
}

fn read_token_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn object_usage_totals(map: &serde_json::Map<String, serde_json::Value>) -> Option<(Option<u64>, Option<u64>, Option<u64>)> {
    let input_tokens = [
        "input_tokens",
        "prompt_tokens",
        "inputTokenCount",
        "inputTokens",
    ]
    .iter()
    .find_map(|key| map.get(*key).and_then(read_token_u64));
    let output_tokens = [
        "output_tokens",
        "completion_tokens",
        "candidatesTokenCount",
        "outputTokenCount",
        "outputTokens",
    ]
    .iter()
    .find_map(|key| map.get(*key).and_then(read_token_u64));
    let total_tokens = [
        "total_tokens",
        "totalTokenCount",
        "totalTokens",
    ]
    .iter()
    .find_map(|key| map.get(*key).and_then(read_token_u64));

    (input_tokens.is_some() || output_tokens.is_some() || total_tokens.is_some())
        .then_some((input_tokens, output_tokens, total_tokens))
}

fn accumulate_token_usage_from_value(value: &serde_json::Value, totals: &mut SessionTokenTotals, depth: usize) {
    if depth > 8 {
        return;
    }

    match value {
        serde_json::Value::Object(map) => {
            if let Some((input_tokens, output_tokens, total_tokens)) = object_usage_totals(map) {
                totals.record(input_tokens, output_tokens, total_tokens);
                return;
            }

            for child in map.values() {
                accumulate_token_usage_from_value(child, totals, depth + 1);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                accumulate_token_usage_from_value(item, totals, depth + 1);
            }
        }
        _ => {}
    }
}

fn normalize_session_query(query: Option<String>) -> String {
    query.unwrap_or_default().trim().to_lowercase()
}

fn session_roots_for_tool(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(global_root) = resolve_tool_config_dir(conn, tool_id) {
        if global_root.exists() {
            let key = global_root.to_string_lossy().to_string();
            if seen.insert(key) {
                roots.push(global_root);
            }
        }
    }

    if let Some(hidden_dir) = tool_hidden_dir(tool_id) {
        for project_root in discover_project_roots(conn) {
            let session_root = project_root.join(hidden_dir);
            if !session_root.exists() {
                continue;
            }
            let key = session_root.to_string_lossy().to_string();
            if seen.insert(key) {
                roots.push(session_root);
            }
        }
    }

    Ok(roots)
}

fn is_session_candidate_path(
    tool_id: &str,
    path: &std::path::Path,
    base_dir: &std::path::Path,
) -> bool {
    if !path.is_file() {
        return false;
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let relative = match path.strip_prefix(base_dir) {
        Ok(relative) => relative.to_string_lossy().to_ascii_lowercase(),
        Err(_) => path.to_string_lossy().to_ascii_lowercase(),
    };

    let has_keyword = [
        "session",
        "sessions",
        "history",
        "conversation",
        "conversations",
        "thread",
        "threads",
        "chat",
        "rollout",
        "transcript",
        "project",
    ]
    .iter()
    .any(|keyword| relative.contains(keyword));

    match extension.as_deref() {
        Some("jsonl") => {
            if !has_keyword {
                return false;
            }
            // Skip Claude agent sub-sessions (e.g. agent-a54b9a9c979dbd77c.jsonl)
            if tool_id == "claude" {
                if let Some(stem) = path.file_stem().and_then(|v| v.to_str()) {
                    if stem.starts_with("agent-") {
                        return false;
                    }
                }
            }
            true
        }
        Some("sqlite" | "db") => has_keyword || tool_id == "opencode",
        _ => false,
    }
}

fn collect_session_candidate_files(
    tool_id: &str,
    current_dir: &std::path::Path,
    base_dir: &std::path::Path,
    jsonl_files: &mut Vec<PathBuf>,
    sqlite_files: &mut Vec<PathBuf>,
    depth: usize,
) {
    if depth > 5 {
        return;
    }

    let read_dir = match std::fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(
                dir_name.as_str(),
                ".git" | "node_modules" | "dist" | "target"
            ) {
                continue;
            }
            collect_session_candidate_files(
                tool_id,
                &path,
                base_dir,
                jsonl_files,
                sqlite_files,
                depth + 1,
            );
            continue;
        }

        if !is_session_candidate_path(tool_id, &path, base_dir) {
            continue;
        }

        match path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("jsonl") => jsonl_files.push(path),
            Some("sqlite" | "db") => sqlite_files.push(path),
            _ => {}
        }
    }
}

fn preferred_texts_from_value(value: &serde_json::Value, texts: &mut Vec<String>, depth: usize) {
    if depth > 4 || texts.len() >= 8 {
        return;
    }

    match value {
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                texts.push(trimmed.to_string());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter().take(8) {
                preferred_texts_from_value(item, texts, depth + 1);
                if texts.len() >= 8 {
                    break;
                }
            }
        }
        serde_json::Value::Object(map) => {
            for key in [
                "text", "message", "content", "preview", "prompt", "output", "title",
            ] {
                if let Some(child) = map.get(key) {
                    preferred_texts_from_value(child, texts, depth + 1);
                    if texts.len() >= 8 {
                        return;
                    }
                }
            }
            for key in ["payload", "items", "messages", "data"] {
                if let Some(child) = map.get(key) {
                    preferred_texts_from_value(child, texts, depth + 1);
                    if texts.len() >= 8 {
                        return;
                    }
                }
            }
        }
        _ => {}
    }
}

fn read_session_token_totals_from_jsonl(path: &std::path::Path) -> SessionTokenTotals {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return SessionTokenTotals::default(),
    };

    let mut totals = SessionTokenTotals::default();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        accumulate_token_usage_from_value(&value, &mut totals, 0);
    }

    totals
}

/// Resolve the full path to a CLI tool executable (returned WITHOUT quotes).
/// On Windows, checks npm global bin first, then `where`.
/// Falls back to the bare command name if nothing is found.
fn resolve_cli_path(cmd: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        // 1) npm global bin — most Node.js CLI tools live here
        if let Ok(appdata) = std::env::var("APPDATA") {
            let npm_cmd = std::path::PathBuf::from(&appdata)
                .join("npm")
                .join(format!("{cmd}.cmd"));
            if npm_cmd.exists() {
                return npm_cmd.to_string_lossy().to_string();
            }
        }
        // 2) `where` — may return system shims (e.g. C:\Windows\claude.exe)
        let mut process = std::process::Command::new("where");
        configure_background_command(&mut process);
        if let Ok(output) = process.arg(cmd).output() {
            if output.status.success() {
                let mut fallback = None;
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let p = line.trim();
                    if p.is_empty() {
                        continue;
                    }
                    let path = std::path::PathBuf::from(p);
                    if !path.exists() {
                        continue;
                    }
                    let lower = p.to_ascii_lowercase();
                    let is_windows_alias = lower.starts_with("c:\\windows\\")
                        && (lower.ends_with(&format!("\\{cmd}.exe"))
                            || lower.ends_with(&format!("\\{cmd}.cmd")));
                    let is_cmd_wrapper = lower.ends_with(".cmd")
                        || lower.ends_with(".bat")
                        || lower.ends_with(".ps1");

                    if !is_windows_alias && is_cmd_wrapper {
                        return p.to_string();
                    }
                    if !is_windows_alias && fallback.is_none() {
                        fallback = Some(p.to_string());
                    }
                }
                if let Some(path) = fallback {
                    return path;
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = std::process::Command::new("which").arg(cmd).output() {
            if output.status.success() {
                if let Some(line) = String::from_utf8_lossy(&output.stdout).lines().next() {
                    let p = line.trim();
                    if !p.is_empty() {
                        return p.to_string();
                    }
                }
            }
        }
    }
    cmd.to_string()
}

/// Quote a CLI path for embedding in a shell command string.
/// Only adds quotes when the path contains spaces.
fn shell_quote_cli(path: &str) -> String {
    if path.contains(' ') {
        format!("\"{}\"", path)
    } else {
        path.to_string()
    }
}

fn codex_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("codex"));
    format!("{cli} resume {session_id}")
}

fn claude_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("claude"));
    format!("{cli} --resume {session_id}")
}

fn gemini_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("gemini"));
    format!("{cli} --resume {session_id}")
}

fn opencode_resume_command(session_id: &str) -> String {
    let cli = shell_quote_cli(&resolve_cli_path("opencode"));
    format!("{cli} session resume {session_id}")
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn resolve_openclaw_session_key(
    source_path: Option<&str>,
    session_id: &str,
) -> Result<String, String> {
    let source_path = source_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Missing OpenClaw session source path".to_string())?;
    let source = PathBuf::from(source_path);
    let index_path = source
        .parent()
        .ok_or_else(|| format!("Invalid OpenClaw session path: {source_path}"))?
        .join("sessions.json");
    let content = std::fs::read_to_string(&index_path).map_err(|e| {
        format!(
            "Failed to read OpenClaw sessions index {}: {e}",
            index_path.display()
        )
    })?;
    let parsed: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        format!(
            "Failed to parse OpenClaw sessions index {}: {e}",
            index_path.display()
        )
    })?;
    let obj = parsed.as_object().ok_or_else(|| {
        format!(
            "OpenClaw sessions index is not a JSON object: {}",
            index_path.display()
        )
    })?;

    for (session_key, entry) in obj {
        let same_id = entry.get("sessionId").and_then(|value| value.as_str()) == Some(session_id);
        let same_file = entry
            .get("sessionFile")
            .and_then(|value| value.as_str())
            .map(|value| PathBuf::from(value) == source)
            .unwrap_or(false);
        if same_id || same_file {
            return Ok(session_key.clone());
        }
    }

    Err(format!(
        "OpenClaw session key not found for session {session_id} in {}",
        index_path.display()
    ))
}

fn openclaw_resume_command(source_path: Option<&str>, session_id: &str) -> Result<String, String> {
    let session_key = resolve_openclaw_session_key(source_path, session_id)?;
    Ok(format!(
        "openclaw tui --session {}",
        shell_single_quote(&session_key)
    ))
}

fn tool_supports_session_resume(tool_id: &str) -> bool {
    match tool_id {
        "codex" => cli_exists_in_path("codex"),
        "claude" => cli_exists_in_path("claude"),
        "gemini" => cli_exists_in_path("gemini"),
        "opencode" => cli_exists_in_path("opencode"),
        "openclaw" => cli_exists_in_path("openclaw"),
        _ => false,
    }
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
            write_default_file_if_missing(
                &config_dir.join("auth.json"),
                "{}\n",
                &mut created_files,
            )?;
            notes.push("Codex CLI 仍需登录后 auth.json 才会真正可用".to_string());
        }
        "gemini" => {
            write_default_file_if_missing(
                &config_dir.join("settings.json"),
                "{}\n",
                &mut created_files,
            )?;
            write_default_file_if_missing(
                &config_dir.join(".env"),
                "# Add GEMINI_API_KEY=...\n",
                &mut created_files,
            )?;
            notes.push("Gemini CLI 仍需在 .env 中填写 GEMINI_API_KEY".to_string());
        }
        "opencode" => {
            write_default_file_if_missing(
                &config_dir.join("opencode.json"),
                "{}\n",
                &mut created_files,
            )?;
        }
        "openclaw" => {
            write_default_file_if_missing(
                &config_dir.join("openclaw.json"),
                "{}\n",
                &mut created_files,
            )?;
        }
        "hermes" => {
            write_default_file_if_missing(
                &config_dir.join("config.yaml"),
                "model:\n  provider: openrouter\n  default: anthropic/claude-sonnet-4.6\n  base_url: https://openrouter.ai/api/v1\n",
                &mut created_files,
            )?;
            write_default_file_if_missing(
                &config_dir.join(".env"),
                "# Add OPENROUTER_API_KEY=...\n",
                &mut created_files,
            )?;
            notes.push("Hermes 仅支持 Linux / macOS / WSL2；Windows 请把根目录覆盖指向 WSL2 内的 ~/.hermes".to_string());
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

        key.trim() == "GEMINI_API_KEY" && !value.trim().is_empty() && value.trim() != "..."
    })
}

fn is_external_target(target: &str) -> bool {
    let trimmed = target.trim();
    trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with("tel:")
}

fn existing_open_target(target: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(target);
    if path.exists() {
        return Ok(path);
    }

    let mut current = path.parent().map(|parent| parent.to_path_buf());
    while let Some(candidate) = current {
        if candidate.exists() {
            return Ok(candidate);
        }
        current = candidate.parent().map(|parent| parent.to_path_buf());
    }

    Err(format!("Path not found: {}", target))
}

fn open_target_in_system(target: &str) -> Result<(), String> {
    if target.trim().is_empty() {
        return Err("Target is empty".to_string());
    }

    if is_external_target(target) {
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
    }

    let resolved_target = existing_open_target(target)?;
    let resolved_text = resolved_target.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        if resolved_target.is_file() {
            std::process::Command::new("explorer")
                .args(["/select,", &resolved_text])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            std::process::Command::new("explorer")
                .arg(&resolved_text)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if resolved_target.is_file() {
            std::process::Command::new("open")
                .args(["-R", &resolved_text])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            std::process::Command::new("open")
                .arg(&resolved_text)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&resolved_text)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform".to_string())
}

pub(crate) fn set_json_app_setting<T: Serialize>(
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

pub(crate) fn get_json_app_setting<T: for<'de> Deserialize<'de>>(
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

fn set_text_app_setting(conn: &rusqlite::Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn get_text_app_setting(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>, String> {
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .ok())
}

const MANAGED_APP_IDS: [&str; 6] = ["claude", "codex", "gemini", "opencode", "openclaw", "hermes"];
const VISIBLE_APPS_SETTING_KEY: &str = "visible_apps";
const WINDOW_PREFERENCES_SETTING_KEY: &str = "window_preferences";
const COMMON_CONFIG_SNIPPETS_SETTING_KEY: &str = "common_config_snippets";
const WELCOME_COMPLETED_SETTING_KEY: &str = "welcome_completed";

fn is_common_config_tool(tool_id: &str) -> bool {
    matches!(tool_id, "claude" | "codex" | "gemini")
}

fn normalize_integer_like(value: &str) -> Option<i64> {
    let normalized = value.trim().replace(['_', ',', ' '], "");
    if normalized.is_empty() {
        return None;
    }
    normalized.parse::<i64>().ok()
}

fn normalized_non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_common_config_snippet(mut snippet: CommonConfigSnippet) -> CommonConfigSnippet {
    let mut normalized = HashMap::new();
    for (key, value) in snippet.custom_values {
        let Some(key) = normalized_non_empty(&key) else {
            continue;
        };
        let Some(value) = normalized_non_empty(&value) else {
            continue;
        };
        normalized.insert(key, value);
    }
    snippet.custom_values = normalized;
    snippet
}

fn common_config_snippet_has_payload(snippet: &CommonConfigSnippet) -> bool {
    snippet.hide_attribution
        || snippet.enable_teammates
        || snippet.effort_level_high
        || snippet.enable_tool_search
        || !snippet.custom_values.is_empty()
}

fn load_common_config_snippets_from_conn(
    conn: &rusqlite::Connection,
) -> Result<HashMap<String, CommonConfigSnippet>, String> {
    Ok(get_json_app_setting(conn, COMMON_CONFIG_SNIPPETS_SETTING_KEY)?.unwrap_or_default())
}

fn read_common_config_snippet_from_conn(
    conn: &rusqlite::Connection,
    tool_id: &str,
) -> Result<CommonConfigSnippet, String> {
    if !is_common_config_tool(tool_id) {
        return Ok(CommonConfigSnippet::default());
    }

    let snippets = load_common_config_snippets_from_conn(conn)?;
    Ok(snippets
        .get(tool_id)
        .cloned()
        .map(normalize_common_config_snippet)
        .unwrap_or_default())
}

fn write_common_config_snippet_to_conn(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snippet: CommonConfigSnippet,
) -> Result<CommonConfigSnippet, String> {
    if !is_common_config_tool(tool_id) {
        return Err(format!(
            "Common Config Snippet is not supported for tool: {tool_id}"
        ));
    }

    let mut snippets = load_common_config_snippets_from_conn(conn)?;
    let normalized = normalize_common_config_snippet(snippet);
    if common_config_snippet_has_payload(&normalized) {
        snippets.insert(tool_id.to_string(), normalized.clone());
    } else {
        snippets.remove(tool_id);
    }
    set_json_app_setting(conn, COMMON_CONFIG_SNIPPETS_SETTING_KEY, &snippets)?;
    Ok(normalized)
}

fn resolve_claude_settings_local_path(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    let (_, settings_json_path) = resolve_claude_paths(conn)?;
    let parent = settings_json_path
        .parent()
        .ok_or_else(|| "Invalid Claude settings path".to_string())?;
    Ok(parent.join("settings.local.json"))
}

fn read_json_file_or_default(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn write_json_file_pretty(path: &std::path::Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(path, &content).map_err(|e| e.to_string())
}

fn read_claude_config_toggles_from_conn(
    conn: &rusqlite::Connection,
) -> Result<ClaudeConfigToggles, String> {
    let path = resolve_claude_settings_local_path(conn)?;
    let settings = read_json_file_or_default(&path)?;
    let env = settings
        .get("env")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();

    let truthy = |key: &str| {
        env.get(key)
            .and_then(|value| value.as_str())
            .map(|value| matches!(value, "1" | "true" | "TRUE" | "True"))
            .unwrap_or(false)
    };

    let max_thinking_tokens_value = env
        .get("CLAUDE_CODE_MAX_THINKING_TOKENS")
        .and_then(|value| value.as_str())
        .unwrap_or("32000")
        .to_string();

    Ok(ClaudeConfigToggles {
        hide_attribution: truthy("ANTHROPIC_HIDE_ATTRIBUTION"),
        enable_teammates: truthy("CLAUDE_CODE_ENABLE_TEAMMATES"),
        max_thinking_tokens: env
            .get("CLAUDE_CODE_MAX_THINKING_TOKENS")
            .and_then(|value| value.as_str())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        max_thinking_tokens_value,
        enable_tool_search: truthy("ENABLE_TOOL_SEARCH"),
    })
}

fn write_claude_config_toggle_to_conn(
    conn: &rusqlite::Connection,
    key: &str,
    enabled: bool,
) -> Result<ClaudeConfigToggles, String> {
    let path = resolve_claude_settings_local_path(conn)?;
    let mut settings = read_json_file_or_default(&path)?;

    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    if settings.get("env").is_none() || !settings.get("env").is_some_and(|value| value.is_object())
    {
        settings["env"] = serde_json::json!({});
    }

    let env = settings
        .get_mut("env")
        .and_then(|value| value.as_object_mut())
        .ok_or_else(|| "Claude settings.local env must be an object".to_string())?;

    match key {
        "hideAttribution" => {
            if enabled {
                env.insert(
                    "ANTHROPIC_HIDE_ATTRIBUTION".to_string(),
                    serde_json::json!("true"),
                );
            } else {
                env.remove("ANTHROPIC_HIDE_ATTRIBUTION");
            }
        }
        "enableTeammates" => {
            if enabled {
                env.insert(
                    "CLAUDE_CODE_ENABLE_TEAMMATES".to_string(),
                    serde_json::json!("true"),
                );
            } else {
                env.remove("CLAUDE_CODE_ENABLE_TEAMMATES");
            }
        }
        "maxThinkingTokens" => {
            if enabled {
                env.insert(
                    "CLAUDE_CODE_MAX_THINKING_TOKENS".to_string(),
                    serde_json::json!("32000"),
                );
            } else {
                env.remove("CLAUDE_CODE_MAX_THINKING_TOKENS");
            }
        }
        "enableToolSearch" => {
            if enabled {
                env.insert("ENABLE_TOOL_SEARCH".to_string(), serde_json::json!("true"));
            } else {
                env.remove("ENABLE_TOOL_SEARCH");
            }
        }
        _ => {
            return Err(format!("Unknown Claude config toggle: {key}"));
        }
    }

    if env.is_empty() {
        settings.as_object_mut().map(|value| value.remove("env"));
    }

    write_json_file_pretty(&path, &settings)?;
    read_claude_config_toggles_from_conn(conn)
}

fn resolve_codex_structured_paths(
    conn: &rusqlite::Connection,
    path: Option<String>,
) -> Result<(PathBuf, PathBuf), String> {
    let config_path = match path.and_then(|value| normalized_non_empty(&value)) {
        Some(path) => PathBuf::from(path),
        None => resolve_tool_config_dir(conn, "codex")?.join("config.toml"),
    };

    if config_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        != "config.toml"
    {
        return Err(format!(
            "Codex structured editing only supports config.toml: {}",
            config_path.display()
        ));
    }

    let dir = config_path
        .parent()
        .ok_or_else(|| "Invalid Codex config.toml path".to_string())?;
    Ok((config_path.clone(), dir.join("auth.json")))
}
const PREFERRED_TERMINAL_SETTING_KEY: &str = "preferred_terminal";
const BACKUP_PREFERENCES_SETTING_KEY: &str = "backup_preferences";
const LOG_PREFERENCES_SETTING_KEY: &str = "log_preferences";
const PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY: &str = "provider_config_fragments";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPreferences {
    pub launch_at_login: bool,
    pub launch_hidden: bool,
    pub close_to_tray: bool,
    pub lightweight_mode: bool,
}

impl Default for WindowPreferences {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            launch_hidden: false,
            close_to_tray: true,
            lightweight_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalOption {
    pub id: String,
    pub label: String,
    pub command: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalPreferences {
    pub platform: String,
    pub selected_terminal: String,
    pub options: Vec<TerminalOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConflict {
    pub id: String,
    pub kind: String,
    pub variables: Vec<String>,
    pub affected_apps: Vec<String>,
}

fn default_visible_apps() -> Vec<String> {
    MANAGED_APP_IDS.iter().map(|id| (*id).to_string()).collect()
}

fn normalize_visible_apps(visible_apps: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for app_id in visible_apps {
        let trimmed = app_id.trim();
        if MANAGED_APP_IDS.contains(&trimmed) && seen.insert(trimmed.to_string()) {
            normalized.push(trimmed.to_string());
        }
    }

    if normalized.is_empty() {
        normalized.push("claude".to_string());
    }

    normalized
}

fn read_backup_preferences_from_conn(conn: &rusqlite::Connection) -> BackupPreferences {
    let mut preferences: BackupPreferences =
        get_json_app_setting(conn, BACKUP_PREFERENCES_SETTING_KEY)
            .ok()
            .flatten()
            .unwrap_or_default();
    if preferences.retention_count == 0 {
        preferences.retention_count = BackupPreferences::default().retention_count;
    }
    preferences
}

fn normalize_log_level(level: &str) -> String {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" | "warn" | "info" | "debug" | "trace" => level.trim().to_ascii_lowercase(),
        _ => "error".to_string(),
    }
}

pub fn read_log_preferences_from_conn(conn: &rusqlite::Connection) -> LogPreferences {
    let mut preferences: LogPreferences = get_json_app_setting(conn, LOG_PREFERENCES_SETTING_KEY)
        .ok()
        .flatten()
        .unwrap_or_default();
    preferences.level = normalize_log_level(&preferences.level);
    preferences
}

pub fn apply_log_preferences(preferences: &LogPreferences) {
    let level = normalize_log_level(&preferences.level);
    std::env::set_var("CCHUB_LOG_LEVEL", &level);
    std::env::set_var("RUST_LOG", &level);
    std::env::set_var(
        "RUST_BACKTRACE",
        if matches!(level.as_str(), "debug" | "trace") {
            "full"
        } else {
            "1"
        },
    );
}

fn build_log_file_targets() -> LogFileTargets {
    LogFileTargets {
        runtime_log_path: crate::utils::runtime_log_path()
            .to_string_lossy()
            .to_string(),
        crash_log_path: crate::utils::crash_log_path().to_string_lossy().to_string(),
    }
}

fn read_disable_auto_updater_env() -> Option<String> {
    std::env::var("DISABLE_AUTOUPDATER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn updater_environment_state() -> UpdaterEnvironmentState {
    let env_var_value = read_disable_auto_updater_env();
    let normalized = env_var_value
        .as_deref()
        .map(|value| value.to_ascii_lowercase());

    UpdaterEnvironmentState {
        disabled_by_env: matches!(normalized.as_deref(), Some("1" | "true" | "yes" | "on")),
        env_var_value,
    }
}

fn log_level_for_provider_status(status: &str) -> &'static str {
    match status {
        "error" => "warn",
        "healthy" | "reachable" | "fast" | "medium" | "slow" => "info",
        _ => "debug",
    }
}

fn log_provider_result(
    kind: &str,
    tool_id: &str,
    provider_name: &str,
    base_url: Option<&str>,
    status: &str,
    message: &str,
) {
    let target = base_url.unwrap_or("n/a");
    crate::utils::append_runtime_log(
        log_level_for_provider_status(status),
        "providers",
        &format!("{kind} [{tool_id}] {provider_name} -> {target} [{status}] {message}"),
    );
}

pub fn read_window_preferences_from_conn(conn: &rusqlite::Connection) -> WindowPreferences {
    get_json_app_setting(conn, WINDOW_PREFERENCES_SETTING_KEY)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn current_platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

#[cfg(target_os = "macos")]
fn macos_app_exists(name: &str) -> bool {
    let mut candidates = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("Applications"));
    }

    candidates
        .into_iter()
        .any(|base| base.join(format!("{name}.app")).exists())
}

fn terminal_options_for_current_platform() -> Vec<TerminalOption> {
    #[cfg(target_os = "windows")]
    {
        return vec![
            TerminalOption {
                id: "windows-terminal".to_string(),
                label: "Windows Terminal".to_string(),
                command: "wt".to_string(),
                installed: cli_exists_in_path("wt"),
            },
            TerminalOption {
                id: "powershell".to_string(),
                label: "PowerShell".to_string(),
                command: "powershell".to_string(),
                installed: cli_exists_in_path("powershell"),
            },
            TerminalOption {
                id: "cmd".to_string(),
                label: "Command Prompt".to_string(),
                command: "cmd".to_string(),
                installed: cli_exists_in_path("cmd"),
            },
        ];
    }

    #[cfg(target_os = "macos")]
    {
        return vec![
            TerminalOption {
                id: "terminal".to_string(),
                label: "Terminal".to_string(),
                command: "open -a Terminal".to_string(),
                installed: macos_app_exists("Terminal"),
            },
            TerminalOption {
                id: "iterm2".to_string(),
                label: "iTerm".to_string(),
                command: "open -a iTerm".to_string(),
                installed: macos_app_exists("iTerm"),
            },
            TerminalOption {
                id: "warp".to_string(),
                label: "Warp".to_string(),
                command: "open -a Warp".to_string(),
                installed: macos_app_exists("Warp"),
            },
            TerminalOption {
                id: "ghostty".to_string(),
                label: "Ghostty".to_string(),
                command: "open -a Ghostty".to_string(),
                installed: macos_app_exists("Ghostty"),
            },
            TerminalOption {
                id: "kaku".to_string(),
                label: "Kaku".to_string(),
                command: "open -a Kaku".to_string(),
                installed: macos_app_exists("Kaku"),
            },
            TerminalOption {
                id: "kitty".to_string(),
                label: "Kitty".to_string(),
                command: "kitty".to_string(),
                installed: cli_exists_in_path("kitty"),
            },
            TerminalOption {
                id: "alacritty".to_string(),
                label: "Alacritty".to_string(),
                command: "alacritty".to_string(),
                installed: cli_exists_in_path("alacritty"),
            },
        ];
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            TerminalOption {
                id: "gnome-terminal".to_string(),
                label: "GNOME Terminal".to_string(),
                command: "gnome-terminal".to_string(),
                installed: cli_exists_in_path("gnome-terminal"),
            },
            TerminalOption {
                id: "konsole".to_string(),
                label: "Konsole".to_string(),
                command: "konsole".to_string(),
                installed: cli_exists_in_path("konsole"),
            },
            TerminalOption {
                id: "xterm".to_string(),
                label: "xterm".to_string(),
                command: "xterm".to_string(),
                installed: cli_exists_in_path("xterm"),
            },
            TerminalOption {
                id: "kitty".to_string(),
                label: "Kitty".to_string(),
                command: "kitty".to_string(),
                installed: cli_exists_in_path("kitty"),
            },
            TerminalOption {
                id: "alacritty".to_string(),
                label: "Alacritty".to_string(),
                command: "alacritty".to_string(),
                installed: cli_exists_in_path("alacritty"),
            },
            TerminalOption {
                id: "wezterm".to_string(),
                label: "WezTerm".to_string(),
                command: "wezterm".to_string(),
                installed: cli_exists_in_path("wezterm"),
            },
        ]
    }
}

fn read_terminal_preferences_from_conn(
    conn: &rusqlite::Connection,
) -> Result<TerminalPreferences, String> {
    let options = terminal_options_for_current_platform();
    let stored = get_text_app_setting(conn, PREFERRED_TERMINAL_SETTING_KEY)?;

    let selected_terminal = stored
        .filter(|terminal_id| options.iter().any(|option| option.id == *terminal_id))
        .or_else(|| {
            options
                .iter()
                .find(|option| option.installed)
                .map(|option| option.id.clone())
        })
        .or_else(|| options.first().map(|option| option.id.clone()))
        .unwrap_or_default();

    Ok(TerminalPreferences {
        platform: current_platform_name().to_string(),
        selected_terminal,
        options,
    })
}

#[allow(dead_code)]
fn shell_quote_single(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'"'"'"#))
}

fn normalize_terminal_target(path: Option<String>) -> Result<PathBuf, String> {
    let base = match path.filter(|value| !value.trim().is_empty()) {
        Some(path) => PathBuf::from(path),
        None => dirs::home_dir().ok_or("Cannot find home directory")?,
    };

    if base.is_dir() {
        return Ok(base);
    }

    if base.is_file() {
        return base
            .parent()
            .map(|parent| parent.to_path_buf())
            .ok_or_else(|| "Cannot determine file parent directory".to_string());
    }

    Err(format!("Path does not exist: {}", base.display()))
}

fn launch_preferred_terminal_impl(
    preferences: &TerminalPreferences,
    target_dir: &std::path::Path,
    shell_command: Option<&str>,
) -> Result<bool, String> {
    let target_text = target_dir.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        // Use raw_arg to bypass Rust's msvcrt arg escaping which causes
        // quote-nesting issues with cmd.exe / wt on Windows.
        use std::os::windows::process::CommandExt;

        if let Some(command) = shell_command {
            match preferences.selected_terminal.as_str() {
                "windows-terminal" => {
                    std::process::Command::new("wt")
                        .raw_arg(format!("-d \"{}\" cmd.exe /K {}", target_text, command))
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "powershell" => {
                    let ps_cmd = format!(
                        "Set-Location -LiteralPath '{}'; {}",
                        target_text.replace('\'', "''"),
                        command,
                    );
                    std::process::Command::new("powershell")
                        .raw_arg(format!("-NoExit -Command \"{}\"", ps_cmd))
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "cmd" => {
                    std::process::Command::new("cmd.exe")
                        .raw_arg(format!("/K cd /d \"{}\" && {}", target_text, command))
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                _ => {
                    return Err(format!(
                        "Unsupported terminal: {}",
                        preferences.selected_terminal
                    ))
                }
            }
            return Ok(true);
        }

        match preferences.selected_terminal.as_str() {
            "windows-terminal" => {
                std::process::Command::new("wt")
                    .raw_arg(format!("-d \"{}\"", target_text))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "powershell" => {
                let ps_cmd = format!(
                    "Set-Location -LiteralPath '{}'",
                    target_text.replace('\'', "''")
                );
                std::process::Command::new("powershell")
                    .raw_arg(format!("-NoExit -Command \"{}\"", ps_cmd))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "cmd" => {
                std::process::Command::new("cmd.exe")
                    .raw_arg(format!("/K cd /d \"{}\"", target_text))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                return Err(format!(
                    "Unsupported terminal: {}",
                    preferences.selected_terminal
                ))
            }
        }
        return Ok(true);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(command) = shell_command {
            let shell_line = format!(
                "cd {} && {} ; exec bash",
                shell_quote_single(&target_text),
                command,
            );
            match preferences.selected_terminal.as_str() {
                "kitty" => {
                    std::process::Command::new("kitty")
                        .args(["--directory", &target_text, "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(true);
                }
                "alacritty" => {
                    std::process::Command::new("alacritty")
                        .args([
                            "--working-directory",
                            &target_text,
                            "-e",
                            "bash",
                            "-lc",
                            &shell_line,
                        ])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(true);
                }
                "terminal" => {
                    std::process::Command::new("open")
                        .args(["-a", "Terminal", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                "iterm2" => {
                    std::process::Command::new("open")
                        .args(["-a", "iTerm", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                "warp" => {
                    std::process::Command::new("open")
                        .args(["-a", "Warp", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                "ghostty" => {
                    std::process::Command::new("open")
                        .args(["-a", "Ghostty", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                "kaku" => {
                    std::process::Command::new("open")
                        .args(["-a", "Kaku", &target_text])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                _ => {
                    return Err(format!(
                        "Unsupported terminal: {}",
                        preferences.selected_terminal
                    ))
                }
            }
        }

        match preferences.selected_terminal.as_str() {
            "terminal" => {
                std::process::Command::new("open")
                    .args(["-a", "Terminal", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "iterm2" => {
                std::process::Command::new("open")
                    .args(["-a", "iTerm", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "warp" => {
                std::process::Command::new("open")
                    .args(["-a", "Warp", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "ghostty" => {
                std::process::Command::new("open")
                    .args(["-a", "Ghostty", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "kaku" => {
                std::process::Command::new("open")
                    .args(["-a", "Kaku", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "kitty" => {
                std::process::Command::new("kitty")
                    .args(["--directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "alacritty" => {
                std::process::Command::new("alacritty")
                    .args(["--working-directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                return Err(format!(
                    "Unsupported terminal: {}",
                    preferences.selected_terminal
                ))
            }
        }
        return Ok(true);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(command) = shell_command {
            let shell_line = format!(
                "cd {} && {} ; exec bash",
                shell_quote_single(&target_text),
                command,
            );
            match preferences.selected_terminal.as_str() {
                "gnome-terminal" => {
                    std::process::Command::new("gnome-terminal")
                        .args([
                            "--working-directory",
                            &target_text,
                            "--",
                            "bash",
                            "-lc",
                            &shell_line,
                        ])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "konsole" => {
                    std::process::Command::new("konsole")
                        .args(["--workdir", &target_text, "-e", "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "xterm" => {
                    std::process::Command::new("xterm")
                        .args(["-e", "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "kitty" => {
                    std::process::Command::new("kitty")
                        .args(["--directory", &target_text, "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "alacritty" => {
                    std::process::Command::new("alacritty")
                        .args([
                            "--working-directory",
                            &target_text,
                            "-e",
                            "bash",
                            "-lc",
                            &shell_line,
                        ])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                "wezterm" => {
                    std::process::Command::new("wezterm")
                        .args(["start", "--cwd", &target_text, "bash", "-lc", &shell_line])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                }
                _ => {
                    return Err(format!(
                        "Unsupported terminal: {}",
                        preferences.selected_terminal
                    ))
                }
            }
            return Ok(true);
        }

        match preferences.selected_terminal.as_str() {
            "gnome-terminal" => {
                std::process::Command::new("gnome-terminal")
                    .args(["--working-directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "konsole" => {
                std::process::Command::new("konsole")
                    .args(["--workdir", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "xterm" => {
                std::process::Command::new("xterm")
                    .args([
                        "-e",
                        "bash",
                        "-lc",
                        &format!("cd {} && exec bash", shell_quote_single(&target_text)),
                    ])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "kitty" => {
                std::process::Command::new("kitty")
                    .args(["--directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "alacritty" => {
                std::process::Command::new("alacritty")
                    .args(["--working-directory", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "wezterm" => {
                std::process::Command::new("wezterm")
                    .args(["start", "--cwd", &target_text])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                return Err(format!(
                    "Unsupported terminal: {}",
                    preferences.selected_terminal
                ))
            }
        }
        return Ok(true);
    }

    #[allow(unreachable_code)]
    Ok(false)
}

fn build_session_resume_command(
    tool_id: &str,
    session_id: &str,
    source_path: Option<&str>,
) -> Result<String, String> {
    match tool_id {
        "codex" => Ok(codex_resume_command(session_id)),
        "claude" => Ok(claude_resume_command(session_id)),
        "gemini" => Ok(gemini_resume_command(session_id)),
        "opencode" => Ok(opencode_resume_command(session_id)),
        "openclaw" => openclaw_resume_command(source_path, session_id),
        _ => Err(format!("Session restore is not supported for {tool_id}")),
    }
}

#[cfg(target_os = "windows")]
fn autostart_entry_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA is not set".to_string())?;
    Ok(PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join("CCHub.cmd"))
}

#[cfg(target_os = "windows")]
fn autostart_entry_content(exe: &std::path::Path) -> String {
    format!("@echo off\r\nstart \"\" \"{}\"\r\n", exe.display())
}

#[cfg(target_os = "macos")]
fn autostart_entry_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join("com.cchub.app.plist"))
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(target_os = "macos")]
fn autostart_entry_content(exe: &std::path::Path) -> String {
    let exe = xml_escape(&exe.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.cchub.app</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn autostart_entry_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir().ok_or("Cannot find config directory")?;
    Ok(config_dir.join("autostart").join("com.cchub.app.desktop"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn autostart_entry_content(exe: &std::path::Path) -> String {
    let escaped = exe.to_string_lossy().replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=CCHub\nExec=\"{escaped}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

fn sync_launch_at_login(enabled: bool) -> Result<(), String> {
    let path = autostart_entry_path()?;

    if !enabled {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &autostart_entry_content(&exe))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn scan_environment_conflicts() -> Vec<EnvironmentConflict> {
    let env_groups = [
        (
            "claude",
            vec![
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "ANTHROPIC_BASE_URL",
                "ANTHROPIC_MODEL",
            ],
        ),
        (
            "codex",
            vec![
                "OPENAI_API_KEY",
                "OPENAI_BASE_URL",
                "OPENAI_ORG_ID",
                "OPENAI_MODEL",
            ],
        ),
        (
            "gemini",
            vec![
                "GEMINI_API_KEY",
                "GOOGLE_API_KEY",
                "GOOGLE_GEMINI_BASE_URL",
                "GEMINI_MODEL",
            ],
        ),
    ];

    let mut conflicts = Vec::new();
    let mut apps_with_overrides = Vec::new();
    let mut all_variables = Vec::new();

    for (app_id, keys) in env_groups {
        let variables: Vec<String> = keys
            .into_iter()
            .filter(|key| {
                std::env::var(key)
                    .ok()
                    .is_some_and(|value| !value.trim().is_empty())
            })
            .map(str::to_string)
            .collect();

        if variables.is_empty() {
            continue;
        }

        all_variables.extend(variables.iter().cloned());
        apps_with_overrides.push(app_id.to_string());
        conflicts.push(EnvironmentConflict {
            id: format!("{app_id}_env_override"),
            kind: "tool_override".to_string(),
            variables,
            affected_apps: vec![app_id.to_string()],
        });
    }

    if apps_with_overrides.len() >= 2 {
        conflicts.insert(
            0,
            EnvironmentConflict {
                id: "shared_env_overrides".to_string(),
                kind: "multi_tool_override".to_string(),
                variables: all_variables,
                affected_apps: apps_with_overrides,
            },
        );
    }

    conflicts
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

fn next_profile_sort_order(conn: &rusqlite::Connection, tool_id: &str) -> i64 {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM config_profiles WHERE tool_id = ?1",
        rusqlite::params![tool_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub(crate) fn ensure_official_config_profiles_seeded(
    conn: &rusqlite::Connection,
) -> Result<(), String> {
    let seeded_at = chrono::Utc::now().to_rfc3339();
    let codex_config = [
        r#"model_provider = "custom""#,
        r#"model = "gpt-5.4""#,
        r#"model_reasoning_effort = "high""#,
        "disable_response_storage = true",
        "",
        "[model_providers.custom]",
        r#"name = "custom""#,
        r#"base_url = "https://api.openai.com/v1""#,
        r#"wire_api = "responses""#,
        "requires_openai_auth = true",
    ]
    .join("\n");

    let seeds = vec![
        (
            "claude",
            "Claude Official",
            serde_json::json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
                },
                "includeCoAuthoredBy": false,
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://www.anthropic.com/api",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "codex",
            "OpenAI Official",
            serde_json::json!({
                "auth": {},
                "config": codex_config,
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://platform.openai.com/",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "gemini",
            "Google Official",
            serde_json::json!({
                "env": {
                    "GOOGLE_GEMINI_BASE_URL": "https://generativelanguage.googleapis.com/v1beta",
                    "GEMINI_MODEL": "gemini-2.5-pro",
                },
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://ai.google.dev/",
                    "seededAt": seeded_at,
                },
                "config": {},
            })
            .to_string(),
        ),
        (
            "openclaw",
            "Anthropic Direct",
            serde_json::json!({
                "baseUrl": "https://api.anthropic.com",
                "apiKey": "",
                "api": "anthropic-messages",
                "models": [{
                    "id": "claude-sonnet-4-5",
                    "name": "claude-sonnet-4-5",
                }],
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://www.anthropic.com/api",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "opencode",
            "OpenAI Responses",
            serde_json::json!({
                "npm": "@ai-sdk/openai",
                "name": "custom",
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://platform.openai.com/",
                    "seededAt": seeded_at,
                },
                "options": {
                    "baseURL": "https://api.openai.com/v1",
                    "apiKey": "",
                },
                "models": {
                    "gpt-5.4": {
                        "name": "gpt-5.4",
                    },
                },
            })
            .to_string(),
        ),
    ];

    for (tool_id, name, config_snapshot) in seeds {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM config_profiles WHERE tool_id = ?1 AND name = ?2",
                rusqlite::params![tool_id, name],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let next_sort_order = next_profile_sort_order(conn, tool_id);
        conn.execute(
            "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'manual', NULL, ?6, ?6)",
            rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn clear_active_profile_if_selected(
    conn: &rusqlite::Connection,
    tool_id: &str,
    profile_id: &str,
) -> Result<(), String> {
    let setting_key = current_profile_setting_key(tool_id);
    let stored_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![&setting_key],
            |row| row.get(0),
        )
        .ok();
    if stored_id.as_deref() == Some(profile_id) {
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn apply_snapshot_if_profile_active(
    conn: &rusqlite::Connection,
    profile_id: &str,
    tool_id: &str,
    config_snapshot: &str,
) -> Result<(), String> {
    let setting_key = current_profile_setting_key(tool_id);
    let active_profile_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
            |row| row.get(0),
        )
        .ok();

    if active_profile_id.as_deref() == Some(profile_id) {
        apply_tool_snapshot(conn, tool_id, config_snapshot)?;
    }

    Ok(())
}

fn delete_profile_record(
    conn: &rusqlite::Connection,
    profile_id: &str,
    tool_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM config_profiles WHERE id = ?1",
        rusqlite::params![profile_id],
    )
    .map_err(|e| e.to_string())?;
    clear_active_profile_if_selected(conn, tool_id, profile_id)?;
    Ok(())
}

fn get_stored_current_profile_ids(
    conn: &rusqlite::Connection,
) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM app_settings WHERE key LIKE 'current_config_profile:%'")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
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

fn normalize_provider_fragment_target_tools(target_tools: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    target_tools
        .into_iter()
        .map(|tool_id| tool_id.trim().to_string())
        .filter(|tool_id| !tool_id.is_empty())
        .filter(|tool_id| MANAGED_APP_IDS.contains(&tool_id.as_str()))
        .filter(|tool_id| seen.insert(tool_id.clone()))
        .collect()
}

fn read_provider_config_fragments_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<ProviderConfigFragment>, String> {
    let mut fragments = get_json_app_setting::<Vec<ProviderConfigFragment>>(
        conn,
        PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY,
    )?
    .unwrap_or_default();

    for fragment in &mut fragments {
        fragment.name = fragment.name.trim().to_string();
        fragment.target_tools =
            normalize_provider_fragment_target_tools(fragment.target_tools.clone());
    }

    fragments.retain(|fragment| {
        !fragment.id.trim().is_empty()
            && !fragment.name.is_empty()
            && !fragment.target_tools.is_empty()
            && fragment.fields.is_object()
    });

    fragments.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(fragments)
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
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (provider_id, tool_id) = row.map_err(|e| e.to_string())?;
            current.insert(
                tool_id.clone(),
                format!("compat-{}-{}", tool_id, provider_id),
            );
        }
    }

    Ok(current)
}

fn read_all_config_profiles_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<ConfigProfile>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, tool_id, config_snapshot, COALESCE(sort_order, 0), source_type, source_key, created_at, updated_at
             FROM config_profiles
             ORDER BY tool_id ASC, COALESCE(sort_order, 0) ASC, updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let profiles = stmt
        .query_map([], |row| {
            Ok(ConfigProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                tool_id: row.get(2)?,
                config_snapshot: row.get(3)?,
                sort_order: row.get(4)?,
                source_type: row.get(5)?,
                source_key: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    Ok(profiles)
}

fn get_active_config_profile_ids_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<String>, String> {
    let profiles = read_all_config_profiles_from_conn(conn)?;
    let mut active_ids = Vec::new();
    let stored_current = get_stored_current_profile_ids(conn)?;
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
            if profiles
                .iter()
                .any(|item| item.tool_id == profile.tool_id && item.id == *preferred_id)
            {
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
            let content = read_tool_snapshot(conn, &profile.tool_id).ok();
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

pub fn read_config_profiles_for_tray(
    conn: &rusqlite::Connection,
) -> Result<Vec<ConfigProfile>, String> {
    read_all_config_profiles_from_conn(conn)
}

pub fn read_active_config_profile_ids_for_tray(
    conn: &rusqlite::Connection,
) -> Result<Vec<String>, String> {
    get_active_config_profile_ids_from_conn(conn)
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
        let next_sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM config_profiles WHERE tool_id = ?1",
                rusqlite::params![tool_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO config_profiles
             (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, source_type, source_key, now],
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
            let Some(config_snapshot) =
                normalize_external_profile_snapshot(&tool_id, &settings_config)
            else {
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
        conn.execute(
            "DELETE FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(counts)
}

fn sync_live_profiles(
    conn: &rusqlite::Connection,
    imported_counts: &HashMap<String, usize>,
    now: &str,
) -> Result<(), String> {
    for tool_id in ["claude", "codex", "gemini", "opencode", "openclaw", "hermes"] {
        let id = format!("live-{}", tool_id);

        if imported_counts.get(tool_id).copied().unwrap_or(0) > 0 {
            conn.execute(
                "DELETE FROM config_profiles WHERE id = ?1",
                rusqlite::params![id],
            )
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
                conn.execute(
                    "DELETE FROM config_profiles WHERE id = ?1",
                    rusqlite::params![id],
                )
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
            let env: HashMap<String, String> = env_text
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .filter_map(|l| {
                    l.split_once('=')
                        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                })
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
        "hermes" => hermes::snapshot::read_snapshot(conn),
        "claude" => {
            let (claude_json, settings_json) = resolve_claude_paths(conn)?;

            let claude_json_obj: serde_json::Map<String, serde_json::Value> =
                if claude_json.exists() {
                    std::fs::read_to_string(&claude_json)
                        .ok()
                        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                        .and_then(|v| v.as_object().cloned())
                        .unwrap_or_default()
                } else {
                    serde_json::Map::new()
                };

            let settings_json_obj: serde_json::Map<String, serde_json::Value> =
                if settings_json.exists() {
                    std::fs::read_to_string(&settings_json)
                        .ok()
                        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                        .and_then(|v| v.as_object().cloned())
                        .unwrap_or_default()
                } else {
                    serde_json::Map::new()
                };

            if claude_json_obj.is_empty() && settings_json_obj.is_empty() {
                return Err("No Claude config found".to_string());
            }

            // Store both sources separately so apply can split them back
            let claude_json_keys: Vec<String> = claude_json_obj.keys().cloned().collect();
            let settings_json_keys: Vec<String> = settings_json_obj.keys().cloned().collect();

            let mut combined = claude_json_obj;
            for (k, v) in settings_json_obj {
                if !combined.contains_key(&k) {
                    combined.insert(k, v);
                }
            }
            combined.insert(
                "__claude_json_keys__".to_string(),
                serde_json::json!(claude_json_keys),
            );
            combined.insert(
                "__settings_json_keys__".to_string(),
                serde_json::json!(settings_json_keys),
            );

            serde_json::to_string_pretty(&serde_json::Value::Object(combined))
                .map_err(|e| e.to_string())
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

fn apply_tool_snapshot(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
) -> Result<(), String> {
    apply_tool_snapshot_with_options(conn, tool_id, snapshot, false)
}

/// Codex TOML keys that are managed by the Tools page / ConfigFiles page and should survive
/// startup reapply of the active profile. On explicit profile switch we intentionally overwrite
/// them, but on unattended startup reapply (whose only real job is to rewrite the proxy base_url)
/// we want the user's last-known values to persist across restarts.
const CODEX_USER_MANAGED_KEYS: &[&str] = &[
    "personality",
    "model_reasoning_effort",
    "disable_response_storage",
    "model_context_window",
    "model_auto_compact_token_limit",
];

/// Overlay the Tools-page-managed codex fields from the existing config.toml on disk onto
/// the snapshot's inline config TOML. Returns the updated snapshot JSON string.
fn overlay_codex_user_fields_into_snapshot(
    snapshot_json: &str,
    existing_config_path: &std::path::Path,
) -> String {
    if !existing_config_path.exists() {
        return snapshot_json.to_string();
    }
    let Ok(existing_text) = std::fs::read_to_string(existing_config_path) else {
        return snapshot_json.to_string();
    };
    let Ok(existing_doc) = existing_text.parse::<toml_edit::DocumentMut>() else {
        return snapshot_json.to_string();
    };
    let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(snapshot_json) else {
        return snapshot_json.to_string();
    };
    let Some(obj) = parsed.as_object_mut() else {
        return snapshot_json.to_string();
    };
    let Some(config_text) = obj.get("config").and_then(|v| v.as_str()).map(str::to_string) else {
        return snapshot_json.to_string();
    };
    let Ok(mut snapshot_doc) = config_text.parse::<toml_edit::DocumentMut>() else {
        return snapshot_json.to_string();
    };

    for key in CODEX_USER_MANAGED_KEYS {
        if let Some(existing_value) = existing_doc.get(*key) {
            snapshot_doc[*key] = existing_value.clone();
        }
    }

    obj.insert(
        "config".to_string(),
        serde_json::Value::String(snapshot_doc.to_string()),
    );
    serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| snapshot_json.to_string())
}

fn apply_tool_snapshot_with_options(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
    preserve_user_edits: bool,
) -> Result<(), String> {
    let effective_snapshot = crate::provider_proxy::materialize_tool_snapshot_for_runtime(
        conn,
        tool_id,
        snapshot,
    )?;

    match tool_id {
        "codex" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let auth_path = dir.join("auth.json");
            let config_path = dir.join("config.toml");

            let snapshot_to_apply = if preserve_user_edits {
                overlay_codex_user_fields_into_snapshot(&effective_snapshot, &config_path)
            } else {
                effective_snapshot.clone()
            };

            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&snapshot_to_apply) {
                if let (Some(auth), Some(config)) = (
                    value.get("auth"),
                    value.get("config").and_then(|v| v.as_str()),
                ) {
                    let auth_text =
                        serde_json::to_string_pretty(auth).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&auth_path, &auth_text)
                        .map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&config_path, config)
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }

            crate::utils::atomic_write_string(&config_path, &snapshot_to_apply)
                .map_err(|e| e.to_string())
        }
        "gemini" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let env_path = dir.join(".env");
            let settings_path = dir.join("settings.json");

            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&effective_snapshot) {
                if let (Some(env), Some(config)) = (
                    value.get("env").and_then(|v| v.as_object()),
                    value.get("config"),
                ) {
                    let env_map: std::collections::HashMap<String, String> = env
                        .iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|v| (key.clone(), v.to_string()))
                        })
                        .collect();
                    let env_text = env_map
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let config_text =
                        serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&env_path, &env_text)
                        .map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&settings_path, &config_text)
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }

            crate::utils::atomic_write_string(&settings_path, &effective_snapshot)
                .map_err(|e| e.to_string())
        }
        "claude" => {
            let (claude_json_path, settings_json_path) = resolve_claude_paths(conn)?;

            let snap: serde_json::Value =
                serde_json::from_str(&effective_snapshot).map_err(|e| e.to_string())?;
            let snap_obj = snap.as_object().ok_or("Invalid claude snapshot")?;

            // Determine which keys belong to which file
            let claude_json_keys: std::collections::HashSet<String> = snap_obj
                .get("__claude_json_keys__")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let settings_json_keys: std::collections::HashSet<String> = snap_obj
                .get("__settings_json_keys__")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            // Keys that should be preserved in settings.json during profile switch
            let preserve_keys: std::collections::HashSet<&str> =
                ["statusLine", "enabledPlugins", "mcpServers"]
                    .iter()
                    .copied()
                    .collect();

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
                        "permissions",
                        "skipDangerousModePermissionPrompt",
                        "alwaysThinkingEnabled",
                        "attribution",
                        "autoUpdatesChannel",
                        "statusLine",
                        "enabledPlugins",
                        "mcpServers",
                        "env",
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
                let mut existing: serde_json::Map<String, serde_json::Value> =
                    if claude_json_path.exists() {
                        std::fs::read_to_string(&claude_json_path)
                            .ok()
                            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                            .and_then(|v| v.as_object().cloned())
                            .unwrap_or_default()
                    } else {
                        serde_json::Map::new()
                    };
                for (k, v) in claude_data {
                    existing.insert(k, v);
                }
                let text = serde_json::to_string_pretty(&serde_json::Value::Object(existing))
                    .map_err(|e| e.to_string())?;
                crate::utils::atomic_write_string(&claude_json_path, &text)
                    .map_err(|e| e.to_string())?;
            }

            // Write settings.json — merge, preserving protected keys
            {
                let mut existing: serde_json::Map<String, serde_json::Value> =
                    if settings_json_path.exists() {
                        std::fs::read_to_string(&settings_json_path)
                            .ok()
                            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                            .and_then(|v| v.as_object().cloned())
                            .unwrap_or_default()
                    } else {
                        serde_json::Map::new()
                    };
                for (k, v) in settings_data {
                    if preserve_keys.contains(k.as_str()) {
                        // Don't overwrite preserved keys — keep current value
                        continue;
                    }
                    existing.insert(k, v);
                }
                std::fs::create_dir_all(settings_json_path.parent().unwrap())
                    .map_err(|e| e.to_string())?;
                let text = serde_json::to_string_pretty(&serde_json::Value::Object(existing))
                    .map_err(|e| e.to_string())?;
                crate::utils::atomic_write_string(&settings_json_path, &text)
                    .map_err(|e| e.to_string())?;
            }

            Ok(())
        }
        "hermes" => hermes::snapshot::apply_snapshot(conn, &effective_snapshot).map(|_| ()),
        _ => {
            let config_path = resolve_tool_config_path(conn, tool_id)?;
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            crate::utils::atomic_write_string(&config_path, &effective_snapshot)
                .map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub fn sync_config_profiles(db: State<'_, DbState>) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        let imported_counts = sync_profiles_from_compatible_databases(&conn, &now)?;
        sync_live_profiles(&conn, &imported_counts, &now)?;
        Ok(())
    })();
    log_command_timing("sync_config_profiles", started_at);
    result
}

#[tauri::command]
pub fn get_config_profiles(db: State<'_, DbState>) -> Result<Vec<ConfigProfile>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_all_config_profiles_from_conn(&conn)
    })();
    log_command_timing("get_config_profiles", started_at);
    result
}

#[tauri::command]
pub fn get_provider_config_fragments(
    db: State<'_, DbState>,
) -> Result<Vec<ProviderConfigFragment>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_provider_config_fragments_from_conn(&conn)
    })();
    log_command_timing("get_provider_config_fragments", started_at);
    result
}

#[tauri::command]
pub fn save_provider_config_fragment(
    id: Option<String>,
    name: String,
    target_tools: Vec<String>,
    fields: serde_json::Value,
    db: State<'_, DbState>,
) -> Result<ProviderConfigFragment, String> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err("Fragment name is required".to_string());
    }
    if !fields.is_object() {
        return Err("Fragment fields must be a JSON object".to_string());
    }

    let normalized_tools = normalize_provider_fragment_target_tools(target_tools);
    if normalized_tools.is_empty() {
        return Err("At least one target app is required".to_string());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut fragments = read_provider_config_fragments_from_conn(&conn)?;
    let now = chrono::Utc::now().to_rfc3339();
    let next_id = id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let saved = if let Some(existing) = fragments.iter_mut().find(|fragment| fragment.id == next_id)
    {
        existing.name = trimmed_name.to_string();
        existing.target_tools = normalized_tools.clone();
        existing.fields = fields.clone();
        existing.updated_at = now.clone();
        existing.clone()
    } else {
        let fragment = ProviderConfigFragment {
            id: next_id.clone(),
            name: trimmed_name.to_string(),
            target_tools: normalized_tools.clone(),
            fields: fields.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        fragments.push(fragment.clone());
        fragment
    };

    fragments.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    set_json_app_setting(&conn, PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY, &fragments)?;
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!(
            "Saved provider config fragment {} for apps {}",
            saved.id,
            saved.target_tools.join(",")
        ),
    );

    Ok(saved)
}

#[tauri::command]
pub fn delete_provider_config_fragment(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut fragments = read_provider_config_fragments_from_conn(&conn)?;
    let initial_len = fragments.len();
    fragments.retain(|fragment| fragment.id != id);
    if fragments.len() == initial_len {
        return Err("Provider fragment not found".to_string());
    }

    set_json_app_setting(&conn, PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY, &fragments)?;
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!("Deleted provider config fragment {id}"),
    );
    Ok(())
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
    let next_sort_order = next_profile_sort_order(&conn, &tool_id);

    conn.execute(
        "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'manual', NULL, ?6, ?6)",
        rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, now],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn save_shared_config_profiles(
    name: String,
    profiles: Vec<SharedConfigProfileInput>,
    group_key: Option<String>,
    replace_profile_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    if profiles.is_empty() {
        return Err("At least one target tool is required".to_string());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let shared_group_key = group_key.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut existing_by_tool: HashMap<String, (String, Option<String>)> = HashMap::new();
    let mut stale_manual_replace: Option<(String, String)> = None;

    {
        let mut stmt = conn
            .prepare(
                "SELECT id, tool_id, source_type
                 FROM config_profiles
                 WHERE source_type = 'shared' AND source_key = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![&shared_group_key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (id, tool_id, source_type) = row.map_err(|e| e.to_string())?;
            existing_by_tool.insert(tool_id, (id, source_type));
        }
    }

    if let Some(profile_id) = replace_profile_id.as_ref() {
        let existing = conn
            .query_row(
                "SELECT tool_id, source_type
                 FROM config_profiles
                 WHERE id = ?1",
                rusqlite::params![profile_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .ok();

        if let Some((tool_id, source_type)) = existing {
            if source_type.as_deref() != Some("shared") && !existing_by_tool.contains_key(&tool_id)
            {
                if profiles.iter().any(|item| item.tool_id == tool_id) {
                    existing_by_tool.insert(tool_id, (profile_id.clone(), source_type));
                } else {
                    stale_manual_replace = Some((tool_id, profile_id.clone()));
                }
            }
        }
    }

    for profile in &profiles {
        if let Some((existing_id, _)) = existing_by_tool.remove(&profile.tool_id) {
            conn.execute(
                "UPDATE config_profiles
                 SET name = ?1, tool_id = ?2, config_snapshot = ?3, source_type = 'shared', source_key = ?4, updated_at = ?5
                 WHERE id = ?6",
                rusqlite::params![
                    &name,
                    &profile.tool_id,
                    &profile.config_snapshot,
                    &shared_group_key,
                    &now,
                    &existing_id
                ],
            )
            .map_err(|e| e.to_string())?;
            apply_snapshot_if_profile_active(
                &conn,
                &existing_id,
                &profile.tool_id,
                &profile.config_snapshot,
            )?;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            let next_sort_order = next_profile_sort_order(&conn, &profile.tool_id);
            conn.execute(
                "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'shared', ?6, ?7, ?7)",
                rusqlite::params![
                    id,
                    &name,
                    &profile.tool_id,
                    &profile.config_snapshot,
                    next_sort_order,
                    &shared_group_key,
                    &now
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    if let Some((tool_id, profile_id)) = stale_manual_replace {
        delete_profile_record(&conn, &profile_id, &tool_id)?;
    }

    for (tool_id, (profile_id, _)) in existing_by_tool {
        delete_profile_record(&conn, &profile_id, &tool_id)?;
    }

    Ok(shared_group_key)
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

pub fn apply_config_profile_from_conn(
    conn: &rusqlite::Connection,
    id: &str,
    preserve_user_edits: bool,
) -> Result<(String, String), String> {
    let (tool_id, snapshot): (String, String) = conn
        .query_row(
            "SELECT tool_id, config_snapshot FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    apply_tool_snapshot_with_options(conn, &tool_id, &snapshot, preserve_user_edits)?;

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE config_profiles SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![current_profile_setting_key(&tool_id), id],
    )
    .map_err(|e| e.to_string())?;

    crate::db::record_activity(conn, &tool_id, "profile_switch", "success", None);
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!("Applied profile {id} for tool {tool_id}"),
    );
    Ok((tool_id, snapshot))
}

#[tauri::command]
pub fn apply_config_profile(
    id: String,
    db: State<'_, DbState>,
) -> Result<ApplyConfigProfileResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (tool_id, _) = apply_config_profile_from_conn(&conn, &id, false)?;
    let active_profile_ids = get_active_config_profile_ids_from_conn(&conn)?;
    Ok(ApplyConfigProfileResult {
        tool_id,
        profile_id: id,
        active_profile_ids,
        applied_at: chrono::Utc::now().to_rfc3339(),
    })
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

    delete_profile_record(&conn, &id, &tool_id)?;

    Ok(())
}

#[tauri::command]
pub fn delete_config_profile_group(
    source_key: String,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, tool_id
             FROM config_profiles
             WHERE source_type = 'shared' AND source_key = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![&source_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(row.map_err(|e| e.to_string())?);
    }

    if profiles.is_empty() {
        return Err("Shared profile group not found".to_string());
    }

    for (profile_id, tool_id) in &profiles {
        delete_profile_record(&conn, profile_id, tool_id)?;
    }

    Ok(profiles.len())
}

#[tauri::command]
pub fn reorder_config_profiles(
    tool_id: String,
    ordered_ids: Vec<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    for (index, profile_id) in ordered_ids.iter().enumerate() {
        let belongs_to_tool: Option<String> = conn
            .query_row(
                "SELECT tool_id FROM config_profiles WHERE id = ?1",
                rusqlite::params![profile_id],
                |row| row.get(0),
            )
            .ok();

        if belongs_to_tool.as_deref() != Some(tool_id.as_str()) {
            return Err(format!(
                "Profile does not belong to tool {tool_id}: {profile_id}"
            ));
        }

        conn.execute(
            "UPDATE config_profiles SET sort_order = ?1 WHERE id = ?2",
            rusqlite::params![index as i64, profile_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_active_config_profile_ids(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        get_active_config_profile_ids_from_conn(&conn)
    })();
    log_command_timing("get_active_config_profile_ids", started_at);
    result
}

#[tauri::command]
pub fn refresh_tray_provider_menu(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::refresh_tray_menu(&app_handle).map_err(|e| e.to_string())
}

fn parse_toml_assignment(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(key) {
            return None;
        }
        let (_, raw_value) = trimmed.split_once('=')?;
        let value = raw_value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn parse_toml_section_assignment(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed.trim_matches(['[', ']']) == section;
            continue;
        }
        if !in_section || !trimmed.starts_with(key) {
            continue;
        }
        let (_, raw_value) = trimmed.split_once('=')?;
        let value = raw_value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            return None;
        }
        return Some(value.to_string());
    }
    None
}

fn read_codex_structured_config_from_content(
    content: &str,
    api_key: String,
) -> CodexTomlStructuredConfig {
    let model_provider =
        parse_toml_assignment(content, "model_provider").unwrap_or_else(|| "custom".to_string());
    let provider_section = format!("model_providers.{model_provider}");

    let mcp_servers = content
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|doc| {
            doc.get("mcp_servers")
                .and_then(|item| item.as_table())
                .map(|table| {
                    table
                        .iter()
                        .map(|(key, _)| key.to_string())
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or_default();

    let malformed_mcp_servers = content
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|doc| doc.get("mcp_servers").map(|item| !item.is_table()))
        .unwrap_or(false);

    CodexTomlStructuredConfig {
        model_provider: model_provider.clone(),
        provider_label: parse_toml_section_assignment(content, &provider_section, "name")
            .unwrap_or_else(|| model_provider.clone()),
        base_url: parse_toml_section_assignment(content, &provider_section, "base_url")
            .unwrap_or_default(),
        wire_api: parse_toml_section_assignment(content, &provider_section, "wire_api")
            .unwrap_or_else(|| "responses".to_string()),
        model: parse_toml_assignment(content, "model").unwrap_or_default(),
        reasoning_effort: parse_toml_assignment(content, "model_reasoning_effort")
            .unwrap_or_else(|| "medium".to_string()),
        personality: parse_toml_assignment(content, "personality")
            .unwrap_or_else(|| "pragmatic".to_string()),
        disable_response_storage: parse_toml_assignment(content, "disable_response_storage")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        model_context_window: parse_toml_assignment(content, "model_context_window")
            .unwrap_or_default(),
        model_auto_compact_token_limit: parse_toml_assignment(
            content,
            "model_auto_compact_token_limit",
        )
        .unwrap_or_default(),
        api_key,
        mcp_servers,
        malformed_mcp_servers,
    }
}

fn write_codex_structured_config_to_text(
    raw_toml: &str,
    config: &CodexTomlStructuredConfig,
) -> String {
    let mut doc = raw_toml
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());

    let provider_name =
        normalized_non_empty(&config.model_provider).unwrap_or_else(|| "custom".to_string());
    let provider_label =
        normalized_non_empty(&config.provider_label).unwrap_or_else(|| provider_name.clone());
    let wire_api =
        normalized_non_empty(&config.wire_api).unwrap_or_else(|| "responses".to_string());
    let reasoning_effort =
        normalized_non_empty(&config.reasoning_effort).unwrap_or_else(|| "medium".to_string());
    let personality =
        normalized_non_empty(&config.personality).unwrap_or_else(|| "pragmatic".to_string());

    doc["model_provider"] = toml_edit::value(provider_name.clone());
    doc["model"] = toml_edit::value(config.model.trim());
    doc["model_reasoning_effort"] = toml_edit::value(reasoning_effort);
    doc["personality"] = toml_edit::value(personality);
    doc["disable_response_storage"] = toml_edit::value(config.disable_response_storage);

    if let Some(context_window) = normalize_integer_like(&config.model_context_window) {
        doc["model_context_window"] = toml_edit::value(context_window);
    } else {
        doc.as_table_mut().remove("model_context_window");
    }

    if let Some(compact_limit) = normalize_integer_like(&config.model_auto_compact_token_limit) {
        doc["model_auto_compact_token_limit"] = toml_edit::value(compact_limit);
    } else {
        doc.as_table_mut().remove("model_auto_compact_token_limit");
    }

    doc["model_providers"][provider_name.as_str()]["name"] = toml_edit::value(provider_label);
    doc["model_providers"][provider_name.as_str()]["base_url"] =
        toml_edit::value(config.base_url.trim());
    doc["model_providers"][provider_name.as_str()]["wire_api"] = toml_edit::value(wire_api);
    doc["model_providers"][provider_name.as_str()]["requires_openai_auth"] = toml_edit::value(true);

    let malformed_mcp_servers = doc
        .get("mcp_servers")
        .map(|item| !item.is_table())
        .unwrap_or(false);
    if malformed_mcp_servers {
        doc.as_table_mut().remove("mcp_servers");
    }
    if doc.get("mcp_servers").is_none() {
        doc["mcp_servers"] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    doc.to_string()
}

fn apply_common_config_to_claude_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Claude snapshot".to_string())?;

    if snippet.hide_attribution {
        obj.insert(
            "attribution".to_string(),
            serde_json::json!({ "commit": "", "pr": "" }),
        );
    }
    if snippet.effort_level_high {
        obj.insert(
            "effortLevel".to_string(),
            serde_json::Value::String("high".to_string()),
        );
    }

    let env = obj
        .entry("env")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Claude env must be an object".to_string())?;
    if snippet.enable_teammates {
        env.insert(
            "CLAUDE_CODE_ENABLE_TEAMMATES".to_string(),
            serde_json::json!("true"),
        );
        env.insert(
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".to_string(),
            serde_json::json!("1"),
        );
    }
    if snippet.enable_tool_search {
        env.insert("ENABLE_TOOL_SEARCH".to_string(), serde_json::json!("true"));
    }
    for (key, value) in &snippet.custom_values {
        env.insert(key.clone(), serde_json::json!(value));
    }

    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

fn apply_common_config_to_codex_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Codex snapshot".to_string())?;
    let current_config = obj
        .get("config")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let current_config = current_config.to_string();
    let current_api_key = obj
        .get("auth")
        .and_then(|value| value.get("OPENAI_API_KEY"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let mut structured =
        read_codex_structured_config_from_content(&current_config, current_api_key);
    if snippet.effort_level_high {
        structured.reasoning_effort = "high".to_string();
    }
    for (key, value) in &snippet.custom_values {
        if key == "model_auto_compact_token_limit" {
            structured.model_auto_compact_token_limit = value.clone();
        }
    }
    let mut next_toml = write_codex_structured_config_to_text(&current_config, &structured);
    for (key, value) in &snippet.custom_values {
        if key == "model_auto_compact_token_limit" {
            continue;
        }
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            continue;
        }
        let mut doc = next_toml
            .parse::<toml_edit::DocumentMut>()
            .unwrap_or_else(|_| toml_edit::DocumentMut::new());
        if let Some(integer) = normalize_integer_like(value) {
            doc[normalized_key] = toml_edit::value(integer);
        } else if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
            doc[normalized_key] = toml_edit::value(value.eq_ignore_ascii_case("true"));
        } else {
            doc[normalized_key] = toml_edit::value(value.as_str());
        }
        next_toml = doc.to_string();
    }
    obj.insert("config".to_string(), serde_json::Value::String(next_toml));
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

fn apply_common_config_to_gemini_snapshot(
    snapshot: &str,
    snippet: &CommonConfigSnippet,
) -> Result<String, String> {
    let mut parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Invalid Gemini snapshot".to_string())?;
    let env = obj
        .entry("env")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Gemini env must be an object".to_string())?;
    for (key, value) in &snippet.custom_values {
        env.insert(key.clone(), serde_json::json!(value));
    }
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

#[allow(dead_code)]
fn apply_common_config_snippet_to_snapshot(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
) -> Result<String, String> {
    let snippet = read_common_config_snippet_from_conn(conn, tool_id)?;
    if !common_config_snippet_has_payload(&snippet) {
        return Ok(snapshot.to_string());
    }

    match tool_id {
        "claude" => apply_common_config_to_claude_snapshot(snapshot, &snippet),
        "codex" => apply_common_config_to_codex_snapshot(snapshot, &snippet),
        "gemini" => apply_common_config_to_gemini_snapshot(snapshot, &snippet),
        _ => Ok(snapshot.to_string()),
    }
}

fn join_api_endpoint(base_url: &str, suffix: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed_base = base_url.trim().trim_end_matches('/');
    let trimmed_suffix = suffix.trim_start_matches('/');
    if trimmed_base.ends_with(trimmed_suffix) {
        trimmed_base.to_string()
    } else {
        format!("{trimmed_base}/{trimmed_suffix}")
    }
}

fn build_claude_messages_endpoint(base_url: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/messages") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/messages")
    } else {
        format!("{trimmed}/v1/messages")
    }
}

fn build_gemini_stream_endpoint(base_url: &str, model: &str, use_full_url: bool) -> String {
    if use_full_url {
        return base_url.trim().to_string();
    }
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.contains(":streamGenerateContent") {
        trimmed.to_string()
    } else if trimmed.ends_with(&format!("/models/{model}")) {
        format!("{trimmed}:streamGenerateContent?alt=sse")
    } else {
        format!("{trimmed}/models/{model}:streamGenerateContent?alt=sse")
    }
}

struct StreamCheckRequestSpec {
    endpoint: String,
    headers: Vec<(String, String)>,
    body: serde_json::Value,
}

fn build_provider_probe_client(conn: &rusqlite::Connection) -> Result<reqwest::Client, String> {
    let proxy_url = get_text_app_setting(conn, "proxy_url")?.unwrap_or_default();
    let mut builder = reqwest::Client::builder()
        .user_agent("CCHub Provider Probe")
        .timeout(std::time::Duration::from_secs(10));

    if !proxy_url.trim().is_empty() {
        let proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| format!("Invalid proxy: {e}"))?;
        builder = builder.proxy(proxy);
    }

    builder.build().map_err(|e| e.to_string())
}

fn extract_profile_metadata(
    parsed: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    parsed
        .get("metadata")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default()
}

fn extract_provider_type_from_snapshot(parsed: &serde_json::Value) -> Option<String> {
    extract_profile_metadata(parsed)
        .get("providerType")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn extract_use_full_url_from_snapshot(parsed: &serde_json::Value) -> bool {
    extract_profile_metadata(parsed)
        .get("useFullUrl")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn extract_copilot_account_id_from_snapshot(parsed: &serde_json::Value) -> Option<String> {
    let metadata = extract_profile_metadata(parsed);
    metadata
        .get("authBinding")
        .and_then(|value| {
            value
                .get("authProvider")
                .and_then(|item| item.as_str())
                .map(|provider| (value, provider))
        })
        .and_then(|(value, provider)| {
            if provider == "github_copilot" {
                value
                    .get("accountId")
                    .and_then(|item| item.as_str())
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
            } else {
                None
            }
        })
        .or_else(|| {
            metadata
                .get("githubAccountId")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn build_openai_chat_endpoint(
    base_url: &str,
    provider_type: Option<&str>,
    use_full_url: bool,
) -> String {
    if provider_type == Some("github_copilot") {
        join_api_endpoint(base_url, "chat/completions", use_full_url)
    } else {
        join_api_endpoint(base_url, "v1/chat/completions", use_full_url)
    }
}

async fn resolve_copilot_headers(
    app_handle: &AppHandle,
    parsed: &serde_json::Value,
) -> Result<Vec<(String, String)>, String> {
    let account_id = extract_copilot_account_id_from_snapshot(parsed);
    let manager = app_handle.state::<CopilotAuthState>().0.clone();
    let token = manager
        .get_valid_token_for_account(account_id.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    Ok(copilot_auth::copilot_request_headers(&token))
}

async fn extract_probe_target(
    app_handle: &AppHandle,
    profile: &ConfigProfile,
) -> Result<(Option<String>, Vec<(String, String)>), String> {
    let parsed: serde_json::Value =
        serde_json::from_str(&profile.config_snapshot).map_err(|e| e.to_string())?;
    let provider_type = extract_provider_type_from_snapshot(&parsed);
    let use_full_url = extract_use_full_url_from_snapshot(&parsed);

    match profile.tool_id.as_str() {
        "claude" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                base_url
            } else if provider_type.as_deref() == Some("github_copilot") {
                base_url.map(|value| join_api_endpoint(&value, "models", false))
            } else {
                base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let headers = if provider_type.as_deref() == Some("github_copilot") {
                resolve_copilot_headers(app_handle, &parsed).await?
            } else {
                let mut headers = Vec::new();
                if let Some(token) = env
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .or_else(|| env.get("ANTHROPIC_API_KEY"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let api_format = env
                        .get("ANTHROPIC_API_FORMAT")
                        .and_then(|value| value.as_str())
                        .unwrap_or("anthropic");
                    if api_format == "anthropic" {
                        headers.push(("x-api-key".to_string(), token.to_string()));
                        headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
                    } else {
                        headers.push(("authorization".to_string(), format!("Bearer {token}")));
                    }
                }
                headers
            };
            Ok((base_url, headers))
        }
        "codex" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let explicit_base_url = parse_toml_assignment(config, "base_url");
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.openai.com/v1".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("auth")
                .and_then(|value| value.get("OPENAI_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
        }
        "gemini" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url
                    .or_else(|| Some("https://generativelanguage.googleapis.com/v1beta".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = env
                .get("GEMINI_API_KEY")
                .or_else(|| env.get("GOOGLE_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("x-goog-api-key".to_string(), token.to_string()));
            }
            Ok((base_url, headers))
        }
        "openclaw" => {
            let explicit_base_url = parsed
                .get("baseUrl")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
        }
        "hermes" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let model = config
                .get("model")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let provider = model
                .get("provider")
                .and_then(|value| value.as_str())
                .unwrap_or("custom");
            let explicit_base_url = model
                .get("base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url
                    .or_else(|| hermes::providers::default_base_url_for_provider(provider).map(str::to_string))
            };
            let env_key = parsed
                .get("metadata")
                .and_then(|value| value.get("hermesApiKeyEnv"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| hermes::providers::default_env_key_for_provider(provider).map(str::to_string));
            let mut headers = Vec::new();
            if let Some(token) = env_key
                .as_deref()
                .and_then(|key| env.get(key))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if provider == "gemini" {
                    headers.push(("x-goog-api-key".to_string(), token.to_string()));
                } else if provider == "anthropic" {
                    headers.push(("x-api-key".to_string(), token.to_string()));
                    headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
                } else {
                    headers.push(("authorization".to_string(), format!("Bearer {token}")));
                }
            }
            Ok((base_url, headers))
        }
        "opencode" => {
            let explicit_base_url = parsed
                .get("options")
                .and_then(|value| value.get("baseURL"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url
            } else {
                explicit_base_url.or_else(|| Some("https://api.anthropic.com".to_string()))
            };
            let mut headers = Vec::new();
            if let Some(token) = parsed
                .get("options")
                .and_then(|value| value.get("apiKey"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
            Ok((base_url, headers))
        }
        _ => Ok((None, Vec::new())),
    }
}

fn classify_provider_latency_status(latency_ms: u64) -> String {
    if latency_ms < 200 {
        "fast".to_string()
    } else if latency_ms <= 500 {
        "medium".to_string()
    } else {
        "slow".to_string()
    }
}

async fn extract_stream_check_request(
    app_handle: &AppHandle,
    profile: &ConfigProfile,
) -> Result<StreamCheckRequestSpec, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(&profile.config_snapshot).map_err(|e| e.to_string())?;
    let provider_type = extract_provider_type_from_snapshot(&parsed);
    let use_full_url = extract_use_full_url_from_snapshot(&parsed);

    match profile.tool_id.as_str() {
        "claude" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Claude base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string())
            };
            let model = env
                .get("ANTHROPIC_MODEL")
                .or_else(|| env.get("ANTHROPIC_DEFAULT_SONNET_MODEL"))
                .or_else(|| env.get("ANTHROPIC_REASONING_MODEL"))
                .or_else(|| env.get("ANTHROPIC_DEFAULT_HAIKU_MODEL"))
                .or_else(|| env.get("ANTHROPIC_DEFAULT_OPUS_MODEL"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("claude-sonnet-4-5");
            let api_format = env
                .get("ANTHROPIC_API_FORMAT")
                .and_then(|value| value.as_str())
                .unwrap_or("anthropic");

            if provider_type.as_deref() == Some("github_copilot") || api_format == "openai_chat" {
                let headers = if provider_type.as_deref() == Some("github_copilot") {
                    resolve_copilot_headers(app_handle, &parsed).await?
                } else {
                    let token = env
                        .get("ANTHROPIC_AUTH_TOKEN")
                        .or_else(|| env.get("ANTHROPIC_API_KEY"))
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| "No Claude API token configured".to_string())?;
                    vec![("authorization".to_string(), format!("Bearer {token}"))]
                };
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_openai_chat_endpoint(
                        &base_url,
                        provider_type.as_deref(),
                        use_full_url,
                    ),
                    headers,
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                });
            }

            if api_format == "openai_responses" {
                let token = env
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .or_else(|| env.get("ANTHROPIC_API_KEY"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "No Claude API token configured".to_string())?;
                return Ok(StreamCheckRequestSpec {
                    endpoint: join_api_endpoint(&base_url, "v1/responses", use_full_url),
                    headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                });
            }

            let token = env
                .get("ANTHROPIC_AUTH_TOKEN")
                .or_else(|| env.get("ANTHROPIC_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Claude API token configured".to_string())?;

            Ok(StreamCheckRequestSpec {
                endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                headers: vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ],
                body: serde_json::json!({
                    "model": model,
                    "max_tokens": 16,
                    "stream": true,
                    "messages": [
                        { "role": "user", "content": "Reply with OK." }
                    ],
                }),
            })
        }
        "codex" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let token = parsed
                .get("auth")
                .and_then(|value| value.get("OPENAI_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Codex OPENAI_API_KEY configured".to_string())?;
            let explicit_base_url = parse_toml_assignment(config, "base_url");
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Codex base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            };
            let wire_api = parse_toml_assignment(config, "wire_api")
                .unwrap_or_else(|| "responses".to_string());
            let model =
                parse_toml_assignment(config, "model").unwrap_or_else(|| "gpt-5.4".to_string());
            let (endpoint, body) = if wire_api == "chat" {
                (
                    join_api_endpoint(&base_url, "chat/completions", use_full_url),
                    serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                )
            } else {
                (
                    join_api_endpoint(&base_url, "responses", use_full_url),
                    serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                )
            };

            Ok(StreamCheckRequestSpec {
                endpoint,
                headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                body,
            })
        }
        "gemini" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let token = env
                .get("GEMINI_API_KEY")
                .or_else(|| env.get("GOOGLE_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No Gemini API key configured".to_string())?;
            let explicit_base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Gemini base URL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| {
                    "https://generativelanguage.googleapis.com/v1beta".to_string()
                })
            };
            let model = env
                .get("GEMINI_MODEL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gemini-2.5-flash");

            Ok(StreamCheckRequestSpec {
                endpoint: build_gemini_stream_endpoint(&base_url, model, use_full_url),
                headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                body: serde_json::json!({
                    "contents": [
                        {
                            "role": "user",
                            "parts": [{ "text": "Reply with OK." }]
                        }
                    ],
                    "generationConfig": {
                        "maxOutputTokens": 16
                    }
                }),
            })
        }
        "openclaw" => {
            let api = parsed
                .get("api")
                .and_then(|value| value.as_str())
                .unwrap_or("openai-completions");
            let explicit_base_url = parsed
                .get("baseUrl")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No OpenClaw baseUrl configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string())
            };
            let api_key = parsed
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let model = parsed
                .get("models")
                .and_then(|value| value.as_array())
                .and_then(|models| models.first())
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gpt-5.4");

            match api {
                "openai-responses" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: join_api_endpoint(&base_url, "responses", use_full_url),
                        headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                        body: serde_json::json!({
                            "model": model,
                            "stream": true,
                            "max_output_tokens": 16,
                            "input": "Reply with OK.",
                        }),
                    })
                }
                "anthropic-messages" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                        headers: vec![
                            ("x-api-key".to_string(), token),
                            ("anthropic-version".to_string(), "2023-06-01".to_string()),
                        ],
                        body: serde_json::json!({
                            "model": model,
                            "max_tokens": 16,
                            "stream": true,
                            "messages": [
                                { "role": "user", "content": "Reply with OK." }
                            ],
                        }),
                    })
                }
                "google-generative-ai" => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: build_gemini_stream_endpoint(&base_url, model, use_full_url),
                        headers: vec![("x-goog-api-key".to_string(), token)],
                        body: serde_json::json!({
                            "contents": [
                                {
                                    "role": "user",
                                    "parts": [{ "text": "Reply with OK." }]
                                }
                            ],
                            "generationConfig": {
                                "maxOutputTokens": 16
                            }
                        }),
                    })
                }
                "bedrock-converse-stream" => Err("AWS Bedrock ConverseStream requires SigV4 signing and is not yet supported for stream checks".to_string()),
                _ => {
                    let token = api_key.ok_or_else(|| "No OpenClaw API key configured".to_string())?;
                    Ok(StreamCheckRequestSpec {
                        endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                        headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                        body: serde_json::json!({
                            "model": model,
                            "stream": true,
                            "max_tokens": 16,
                            "messages": [
                                { "role": "user", "content": "Reply with OK." }
                            ],
                        }),
                    })
                }
            }
        }
        "hermes" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let model = config
                .get("model")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let provider = model
                .get("provider")
                .and_then(|value| value.as_str())
                .unwrap_or("custom");
            let explicit_base_url = model
                .get("base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No Hermes base_url configured".to_string())?
            } else {
                explicit_base_url
                    .or_else(|| hermes::providers::default_base_url_for_provider(provider).map(str::to_string))
                    .ok_or_else(|| "No Hermes base_url configured".to_string())?
            };
            let model_id = model
                .get("default")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gpt-5.4");
            let env_key = parsed
                .get("metadata")
                .and_then(|value| value.get("hermesApiKeyEnv"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| hermes::providers::default_env_key_for_provider(provider).map(str::to_string))
                .ok_or_else(|| "No Hermes API key env configured".to_string())?;
            let token = env
                .get(&env_key)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("No Hermes API key configured in {env_key}"))?;

            if provider == "gemini" {
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_gemini_stream_endpoint(&base_url, model_id, use_full_url),
                    headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                    body: serde_json::json!({
                        "contents": [{ "role": "user", "parts": [{ "text": "Reply with OK." }] }],
                        "generationConfig": { "maxOutputTokens": 16 },
                    }),
                });
            }

            if provider == "anthropic" {
                return Ok(StreamCheckRequestSpec {
                    endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                    headers: vec![
                        ("x-api-key".to_string(), token.to_string()),
                        ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ],
                    body: serde_json::json!({
                        "model": model_id,
                        "max_tokens": 16,
                        "stream": true,
                        "messages": [{ "role": "user", "content": "Reply with OK." }],
                    }),
                });
            }

            Ok(StreamCheckRequestSpec {
                endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                body: serde_json::json!({
                    "model": model_id,
                    "stream": true,
                    "max_tokens": 16,
                    "messages": [{ "role": "user", "content": "Reply with OK." }],
                }),
            })
        }
        "opencode" => {
            let npm = parsed
                .get("npm")
                .and_then(|value| value.as_str())
                .unwrap_or("@ai-sdk/openai-compatible");
            let options = parsed
                .get("options")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let explicit_base_url = options
                .get("baseURL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let base_url = if use_full_url {
                explicit_base_url.ok_or_else(|| "No OpenCode baseURL configured".to_string())?
            } else {
                explicit_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            };
            let token = options
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "No OpenCode API key configured".to_string())?;
            let model = parsed
                .get("models")
                .and_then(|value| value.as_object())
                .and_then(|value| value.keys().next().cloned())
                .unwrap_or_else(|| "gpt-5.4".to_string());

            if npm.contains("anthropic") {
                Ok(StreamCheckRequestSpec {
                    endpoint: build_claude_messages_endpoint(&base_url, use_full_url),
                    headers: vec![
                        ("x-api-key".to_string(), token.to_string()),
                        ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ],
                    body: serde_json::json!({
                        "model": model,
                        "max_tokens": 16,
                        "stream": true,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                })
            } else if npm.contains("google") {
                Ok(StreamCheckRequestSpec {
                    endpoint: build_gemini_stream_endpoint(&base_url, &model, use_full_url),
                    headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                    body: serde_json::json!({
                        "contents": [
                            {
                                "role": "user",
                                "parts": [{ "text": "Reply with OK." }]
                            }
                        ],
                        "generationConfig": {
                            "maxOutputTokens": 16
                        }
                    }),
                })
            } else if npm == "@ai-sdk/openai" {
                Ok(StreamCheckRequestSpec {
                    endpoint: join_api_endpoint(&base_url, "responses", use_full_url),
                    headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_output_tokens": 16,
                        "input": "Reply with OK.",
                    }),
                })
            } else {
                Ok(StreamCheckRequestSpec {
                    endpoint: join_api_endpoint(&base_url, "chat/completions", use_full_url),
                    headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                    body: serde_json::json!({
                        "model": model,
                        "stream": true,
                        "max_tokens": 16,
                        "messages": [
                            { "role": "user", "content": "Reply with OK." }
                        ],
                    }),
                })
            }
        }
        _ => Err("Stream check is not supported for this profile".to_string()),
    }
}

#[tauri::command]
pub async fn ping_provider_endpoint(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderPingResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let (base_url, headers) = match extract_probe_target(&app_handle, &profile).await {
        Ok(value) => value,
        Err(message) => {
            let result = ProviderPingResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: "error".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
            log_provider_result(
                "ping",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };

    let Some(base_url) = base_url else {
        let result = ProviderPingResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: None,
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: "No base URL configured for latency ping".to_string(),
        };
        log_provider_result(
            "ping",
            &result.tool_id,
            &result.provider_name,
            result.base_url.as_deref(),
            &result.status,
            &result.message,
        );
        return Ok(result);
    };

    let send_request = |method: reqwest::Method| {
        let client = client.clone();
        let base_url = base_url.clone();
        let headers = headers.clone();
        async move {
            let started_at = std::time::Instant::now();
            let mut request = client.request(method, &base_url);
            for (name, value) in headers {
                request = request.header(&name, value);
            }
            request
                .send()
                .await
                .map(|response| (response, started_at.elapsed().as_millis() as u64))
        }
    };

    let mut response_result = send_request(reqwest::Method::HEAD).await;
    let should_fallback_to_get = matches!(
        response_result,
        Ok((ref response, _))
            if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
                || response.status() == reqwest::StatusCode::NOT_IMPLEMENTED
    );
    if should_fallback_to_get {
        response_result = send_request(reqwest::Method::GET).await;
    }

    let result = match response_result {
        Ok((response, latency_ms)) => {
            let http_status = response.status().as_u16();
            ProviderPingResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: Some(base_url),
                status: classify_provider_latency_status(latency_ms),
                latency_ms: Some(latency_ms),
                http_status: Some(http_status),
                checked_at,
                message: format!("Endpoint responded with HTTP {http_status}"),
            }
        }
        Err(error) => ProviderPingResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: Some(base_url),
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: error.to_string(),
        },
    };

    log_provider_result(
        "ping",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
}

#[tauri::command]
pub async fn probe_config_profile(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderProbeResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let (base_url, headers) = match extract_probe_target(&app_handle, &profile).await {
        Ok(value) => value,
        Err(message) => {
            let result = ProviderProbeResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: "unconfigured".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
            log_provider_result(
                "probe",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };

    let result = if let Some(base_url) = base_url {
        let started_at = std::time::Instant::now();
        let mut request = client.get(&base_url);
        for (name, value) in headers {
            request = request.header(&name, value);
        }

        match request.send().await {
            Ok(response) => {
                let latency_ms = started_at.elapsed().as_millis() as u64;
                let http_status = response.status().as_u16();
                let status = if response.status().is_success() {
                    "healthy"
                } else if response.status().is_client_error() || response.status().is_server_error()
                {
                    "reachable"
                } else {
                    "unknown"
                };

                ProviderProbeResult {
                    profile_id: profile.id,
                    tool_id: profile.tool_id,
                    provider_name: profile.name,
                    base_url: Some(base_url),
                    status: status.to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message: format!("Endpoint responded with HTTP {http_status}"),
                }
            }
            Err(error) => ProviderProbeResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: Some(base_url),
                status: "error".to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message: error.to_string(),
            },
        }
    } else {
        ProviderProbeResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: None,
            status: "unconfigured".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: "No base URL configured for probing".to_string(),
        }
    };

    log_provider_result(
        "probe",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
}

#[tauri::command]
pub async fn stream_check_config_profile(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<ProviderStreamCheckResult, String> {
    let (profile, client) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| format!("Profile not found: {id}"))?;
        let client = build_provider_probe_client(&conn)?;
        (profile, client)
    };

    let checked_at = chrono::Utc::now().to_rfc3339();
    let request = match extract_stream_check_request(&app_handle, &profile).await {
        Ok(request) => request,
        Err(message) => {
            let status =
                if message.contains("not yet supported") || message.contains("not supported") {
                    "unsupported"
                } else {
                    "unconfigured"
                };
            let result = ProviderStreamCheckResult {
                profile_id: profile.id,
                tool_id: profile.tool_id,
                provider_name: profile.name,
                base_url: None,
                status: status.to_string(),
                latency_ms: None,
                http_status: None,
                checked_at,
                message,
            };
            log_provider_result(
                "stream-check",
                &result.tool_id,
                &result.provider_name,
                result.base_url.as_deref(),
                &result.status,
                &result.message,
            );
            return Ok(result);
        }
    };
    let StreamCheckRequestSpec {
        endpoint,
        headers,
        body,
    } = request;

    let started_at = std::time::Instant::now();
    let mut request_builder = client
        .post(&endpoint)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream, application/json");
    for (name, value) in headers {
        request_builder = request_builder.header(&name, value);
    }

    let result = match request_builder.json(&body).send().await {
        Ok(mut response) => {
            let latency_ms = started_at.elapsed().as_millis() as u64;
            let http_status = response.status().as_u16();

            if !response.status().is_success() {
                let detail = response.text().await.unwrap_or_default();
                ProviderStreamCheckResult {
                    profile_id: profile.id,
                    tool_id: profile.tool_id,
                    provider_name: profile.name,
                    base_url: Some(endpoint.clone()),
                    status: "reachable".to_string(),
                    latency_ms: Some(latency_ms),
                    http_status: Some(http_status),
                    checked_at,
                    message: if detail.trim().is_empty() {
                        format!("Endpoint responded with HTTP {http_status}")
                    } else {
                        format!(
                            "HTTP {http_status}: {}",
                            detail.chars().take(160).collect::<String>()
                        )
                    },
                }
            } else {
                match tokio::time::timeout(std::time::Duration::from_secs(15), response.chunk()).await {
                    Ok(Ok(Some(chunk))) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "healthy".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: format!("Received first stream chunk ({} bytes)", chunk.len()),
                    },
                    Ok(Ok(None)) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "reachable".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: "Stream endpoint closed without returning chunks".to_string(),
                    },
                    Ok(Err(error)) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "error".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: error.to_string(),
                    },
                    Err(_) => ProviderStreamCheckResult {
                        profile_id: profile.id,
                        tool_id: profile.tool_id,
                        provider_name: profile.name,
                        base_url: Some(endpoint.clone()),
                        status: "reachable".to_string(),
                        latency_ms: Some(latency_ms),
                        http_status: Some(http_status),
                        checked_at,
                        message: "Connected successfully but did not receive a stream chunk within 15 seconds".to_string(),
                    },
                }
            }
        }
        Err(error) => ProviderStreamCheckResult {
            profile_id: profile.id,
            tool_id: profile.tool_id,
            provider_name: profile.name,
            base_url: Some(endpoint),
            status: "error".to_string(),
            latency_ms: None,
            http_status: None,
            checked_at,
            message: error.to_string(),
        },
    };

    log_provider_result(
        "stream-check",
        &result.tool_id,
        &result.provider_name,
        result.base_url.as_deref(),
        &result.status,
        &result.message,
    );
    Ok(result)
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
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('proxy_url', '')",
            [],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let url = proxy_url.trim().to_string();
        std::env::set_var("HTTP_PROXY", &url);
        std::env::set_var("HTTPS_PROXY", &url);
        std::env::set_var("http_proxy", &url);
        std::env::set_var("https_proxy", &url);
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('proxy_url', ?1)",
            rusqlite::params![url],
        )
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

#[tauri::command]
pub fn get_visible_apps(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let stored = get_json_app_setting::<Vec<String>>(&conn, VISIBLE_APPS_SETTING_KEY)?;
    Ok(stored
        .map(normalize_visible_apps)
        .unwrap_or_else(default_visible_apps))
}

#[tauri::command]
pub fn set_visible_apps(
    visible_apps: Vec<String>,
    db: State<'_, DbState>,
) -> Result<Vec<String>, String> {
    let normalized = normalize_visible_apps(visible_apps);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, VISIBLE_APPS_SETTING_KEY, &normalized)?;
    Ok(normalized)
}

#[tauri::command]
pub fn get_welcome_completed(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(get_json_app_setting::<bool>(&conn, WELCOME_COMPLETED_SETTING_KEY)?.unwrap_or(false))
}

#[tauri::command]
pub fn set_welcome_completed(
    completed: bool,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, WELCOME_COMPLETED_SETTING_KEY, &completed)?;
    Ok(completed)
}

#[tauri::command]
pub fn get_hermes_root_override(db: State<'_, DbState>) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    hermes::read_root_override(&conn)
}

#[tauri::command]
pub fn set_hermes_root_override(
    value: Option<String>,
    db: State<'_, DbState>,
) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    hermes::write_root_override(&conn, value.as_deref())
}

#[tauri::command]
pub fn get_window_preferences(db: State<'_, DbState>) -> Result<WindowPreferences, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_window_preferences_from_conn(&conn))
}

#[tauri::command]
pub fn get_log_preferences(db: State<'_, DbState>) -> Result<LogPreferences, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_log_preferences_from_conn(&conn))
}

#[tauri::command]
pub fn set_log_preferences(
    preferences: LogPreferences,
    db: State<'_, DbState>,
) -> Result<LogPreferences, String> {
    let sanitized = LogPreferences {
        level: normalize_log_level(&preferences.level),
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, LOG_PREFERENCES_SETTING_KEY, &sanitized)?;
    apply_log_preferences(&sanitized);
    crate::utils::append_runtime_log(
        "info",
        "settings",
        &format!("Log level changed to {}", sanitized.level),
    );
    Ok(sanitized)
}

#[tauri::command]
pub fn get_log_file_targets() -> LogFileTargets {
    build_log_file_targets()
}

#[tauri::command]
pub fn get_updater_environment_state() -> UpdaterEnvironmentState {
    updater_environment_state()
}

#[tauri::command]
pub fn set_window_preferences(
    preferences: WindowPreferences,
    db: State<'_, DbState>,
) -> Result<WindowPreferences, String> {
    sync_launch_at_login(preferences.launch_at_login)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    set_json_app_setting(&conn, WINDOW_PREFERENCES_SETTING_KEY, &preferences)?;
    Ok(preferences)
}

#[tauri::command]
pub fn get_terminal_preferences(db: State<'_, DbState>) -> Result<TerminalPreferences, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_terminal_preferences_from_conn(&conn)
}

#[tauri::command]
pub fn set_preferred_terminal(
    terminal_id: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let preferences = read_terminal_preferences_from_conn(&conn)?;
    if !preferences
        .options
        .iter()
        .any(|option| option.id == terminal_id)
    {
        return Err(format!("Unsupported terminal: {terminal_id}"));
    }
    set_text_app_setting(&conn, PREFERRED_TERMINAL_SETTING_KEY, &terminal_id)?;
    Ok(terminal_id)
}

#[tauri::command]
pub fn open_in_preferred_terminal(
    path: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let preferences = read_terminal_preferences_from_conn(&conn)?;
    drop(conn);
    let target_dir = normalize_terminal_target(path)?;
    launch_preferred_terminal_impl(&preferences, &target_dir, None).map(|_| ())
}

#[tauri::command]
pub fn resume_session_in_preferred_terminal(
    tool_id: String,
    session_id: String,
    cwd: Option<String>,
    source_path: Option<String>,
    db: State<'_, DbState>,
) -> Result<SessionResumeResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let preferences = read_terminal_preferences_from_conn(&conn)?;
    drop(conn);

    let command = build_session_resume_command(&tool_id, &session_id, source_path.as_deref())?;
    let target_dir = normalize_terminal_target(cwd)?;
    let launched = launch_preferred_terminal_impl(&preferences, &target_dir, Some(&command))?;

    Ok(SessionResumeResult {
        launched,
        command,
        cwd: Some(target_dir.to_string_lossy().to_string()),
    })
}

#[tauri::command]
pub fn get_environment_conflicts() -> Result<Vec<EnvironmentConflict>, String> {
    Ok(scan_environment_conflicts())
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

#[tauri::command]
pub fn search_openclaw_daily_memory(
    query: Option<String>,
    limit: Option<usize>,
    db: State<'_, DbState>,
) -> Result<Vec<OpenClawDailyMemoryEntry>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let query = query.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    let max_results = limit.unwrap_or(30).clamp(1, 100);
    let mut entries = Vec::new();
    let mut scanned_roots = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        let global_dir = home.join(".openclaw");
        if global_dir.exists() && scanned_roots.insert(global_dir.to_string_lossy().to_string()) {
            collect_openclaw_daily_memory_files(
                &global_dir,
                &global_dir,
                "global",
                None,
                query.as_deref(),
                &mut entries,
                0,
            );
        }
    }

    for project_root in discover_project_roots(&conn) {
        let project_name = project_root
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| project_root.to_string_lossy().to_string());
        let memory_root = project_root.join(".openclaw");
        if !memory_root.exists() {
            continue;
        }
        let key = memory_root.to_string_lossy().to_string();
        if !scanned_roots.insert(key) {
            continue;
        }
        collect_openclaw_daily_memory_files(
            &memory_root,
            &memory_root,
            "project",
            Some(&project_name),
            query.as_deref(),
            &mut entries,
            0,
        );
    }

    entries.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    entries.truncate(max_results);
    Ok(entries)
}

#[tauri::command]
pub fn read_openclaw_daily_memory_content(
    path: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let path_buf = std::path::PathBuf::from(&path);
    if !is_valid_openclaw_daily_memory_path(&path_buf, &conn) {
        return Err("Invalid OpenClaw Daily Memory path".to_string());
    }
    std::fs::read_to_string(path_buf).map_err(|e| e.to_string())
}

fn load_codex_history_index(root: &std::path::Path) -> HashMap<String, Vec<String>> {
    let mut index = HashMap::new();
    let history_path = root.join("history.jsonl");
    let file = match std::fs::File::open(history_path) {
        Ok(file) => file,
        Err(_) => return index,
    };

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(session_id) = value.get("session_id").and_then(|item| item.as_str()) else {
            continue;
        };
        let Some(text) = value.get("text").and_then(|item| item.as_str()) else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        index
            .entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(trimmed.to_string());
    }

    index
}

fn codex_state_databases(root: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(read_dir) = std::fs::read_dir(root) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !file_name.starts_with("state_") || !file_name.ends_with(".sqlite") {
                continue;
            }
            if seen.insert(path.to_string_lossy().to_string()) {
                paths.push(path);
            }
        }
    }

    let fallback = root.join("state.sqlite");
    if fallback.exists() && seen.insert(fallback.to_string_lossy().to_string()) {
        paths.push(fallback);
    }

    paths.sort();
    paths.reverse();
    paths
}

fn scan_codex_sessions(
    conn: &rusqlite::Connection,
    query: &str,
) -> Result<Vec<SessionSummary>, String> {
    let root = resolve_tool_config_dir(conn, "codex")?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let history_index = load_codex_history_index(&root);
    let mut sessions = Vec::new();
    let mut seen_ids = HashSet::new();

    for db_path in codex_state_databases(&root) {
        let external = match rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(conn) => conn,
            Err(_) => continue,
        };

        let mut stmt = match external.prepare(
            "SELECT id, rollout_path, created_at, updated_at, cwd, title, first_user_message
             FROM threads
             ORDER BY updated_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let rows = match stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => continue,
        };

        for row in rows {
            let (id, rollout_path, created_at_raw, updated_at_raw, cwd, title, first_user_message) =
                row.map_err(|e| e.to_string())?;
            if !seen_ids.insert(id.clone()) {
                continue;
            }

            let rollout_file_path = {
                let path = PathBuf::from(&rollout_path);
                if path.is_absolute() {
                    path
                } else {
                    root.join(&rollout_path)
                }
            };
            let token_totals = read_session_token_totals_from_jsonl(&rollout_file_path);

            let history_items = history_index.get(&id).cloned().unwrap_or_default();
            let preview_source = history_items
                .last()
                .cloned()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    let trimmed = first_user_message.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .unwrap_or_else(|| id.clone());
            let preview = truncate_session_text(&preview_source, 180);
            let search_values = vec![
                title.clone(),
                preview.clone(),
                cwd.clone(),
                first_user_message.clone(),
            ];
            let search_hit_count = count_query_hits(query, &search_values);
            if !query.is_empty() && search_hit_count == 0 {
                continue;
            }

            sessions.push(SessionSummary {
                id: id.clone(),
                tool_id: "codex".to_string(),
                tool_name: "Codex".to_string(),
                title: if title.trim().is_empty() {
                    let trimmed_first_user = first_user_message.trim();
                    if trimmed_first_user.is_empty() {
                        id.clone()
                    } else {
                        truncate_session_text(trimmed_first_user, 80)
                    }
                } else {
                    title
                },
                cwd: (!cwd.trim().is_empty()).then_some(cwd),
                source_kind: "codex_jsonl".to_string(),
                source_backend: "jsonl".to_string(),
                source_path: rollout_path,
                created_at: format_unix_timestamp(created_at_raw),
                updated_at: format_unix_timestamp(updated_at_raw),
                preview,
                message_count: history_items.len(),
                input_tokens: token_totals.input_option(),
                output_tokens: token_totals.output_option(),
                tokens_used: token_totals.total_option(),
                search_hit_count,
                can_resume: tool_supports_session_resume("codex"),
                can_delete: true,
            });
        }
    }

    if !sessions.is_empty() {
        return Ok(sessions);
    }

    scan_generic_tool_sessions(conn, "codex", query)
}

fn parse_generic_jsonl_session_summary(
    tool_id: &str,
    path: &std::path::Path,
    query: &str,
) -> Option<SessionSummary> {
    let file = std::fs::File::open(path).ok()?;
    let metadata = std::fs::metadata(path).ok();
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();

    let mut session_id = file_stem.clone();
    let mut title: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut first_message_summary: Option<String> = None;
    let mut created_at: Option<String> = metadata
        .as_ref()
        .and_then(|value| value.created().ok())
        .map(format_local_datetime);
    let mut updated_at: Option<String> = metadata
        .as_ref()
        .and_then(|value| value.modified().ok())
        .map(format_local_datetime);
    let mut preview: Option<String> = None;
    let mut message_count = 0usize;
    let mut token_totals = SessionTokenTotals::default();

    for (line_index, line) in BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        accumulate_token_usage_from_value(&value, &mut token_totals, 0);

        if line_index >= 120 {
            continue;
        }

        if let Some(found_id) = value
            .get("sessionId")
            .or_else(|| value.get("session_id"))
            .and_then(|item| item.as_str())
        {
            if !found_id.trim().is_empty() {
                session_id = found_id.trim().to_string();
            }
        } else if value.get("type").and_then(|item| item.as_str()) == Some("session_meta") {
            if let Some(found_id) = value
                .get("payload")
                .and_then(|item| item.get("id"))
                .and_then(|item| item.as_str())
            {
                if !found_id.trim().is_empty() {
                    session_id = found_id.trim().to_string();
                }
            }
        }

        if title.is_none() {
            title = value
                .get("title")
                .and_then(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .or_else(|| {
                    value
                        .get("payload")
                        .and_then(|item| item.get("title"))
                        .and_then(|item| item.as_str())
                        .map(|item| item.trim().to_string())
                        .filter(|item| !item.is_empty())
                });
        }

        if cwd.is_none() {
            cwd = value
                .get("cwd")
                .and_then(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .or_else(|| {
                    value
                        .get("payload")
                        .and_then(|item| item.get("cwd"))
                        .and_then(|item| item.as_str())
                        .map(|item| item.trim().to_string())
                        .filter(|item| !item.is_empty())
                });
        }

        if let Some(timestamp) = value.get("timestamp").and_then(|item| item.as_str()) {
            let formatted = format_timestamp_text(timestamp);
            if created_at.is_none() {
                created_at = formatted.clone();
            }
            updated_at = formatted.or(updated_at);
        } else if let Some(ts) = value.get("ts").and_then(|item| item.as_i64()) {
            let formatted = format_unix_timestamp(ts);
            if created_at.is_none() {
                created_at = formatted.clone();
            }
            updated_at = formatted.or(updated_at);
        }

        let mut texts = Vec::new();
        preferred_texts_from_value(&value, &mut texts, 0);
        if let Some(text) = texts.into_iter().find(|item| !item.trim().is_empty()) {
            message_count += 1;
            if preview.is_none() {
                preview = Some(truncate_session_text(&text, 180));
            }
            if first_message_summary.is_none() {
                first_message_summary = Some(truncate_session_text(&text, 80));
            }
        }
    }

    let title = title
        .or(first_message_summary)
        .unwrap_or_else(|| session_id.clone());
    let preview = preview.unwrap_or_else(|| title.clone());
    let search_values = vec![
        title.clone(),
        preview.clone(),
        cwd.clone().unwrap_or_default(),
        session_id.clone(),
    ];
    let search_hit_count = count_query_hits(query, &search_values);
    if !query.is_empty() && search_hit_count == 0 {
        return None;
    }

    Some(SessionSummary {
        id: session_id,
        tool_id: tool_id.to_string(),
        tool_name: tool_label(tool_id).to_string(),
        title,
        cwd,
        source_kind: format!("{tool_id}_jsonl"),
        source_backend: "jsonl".to_string(),
        source_path: path.to_string_lossy().to_string(),
        created_at,
        updated_at,
        preview,
        message_count,
        input_tokens: token_totals.input_option(),
        output_tokens: token_totals.output_option(),
        tokens_used: token_totals.total_option(),
        search_hit_count,
        can_resume: tool_supports_session_resume(tool_id),
        can_delete: true,
    })
}

fn sqlite_table_columns(
    conn: &rusqlite::Connection,
    table_name: &str,
) -> Result<HashSet<String>, String> {
    let sql = format!("PRAGMA table_info({table_name})");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;

    let mut columns = HashSet::new();
    for row in rows {
        columns.insert(row.map_err(|e| e.to_string())?.to_ascii_lowercase());
    }
    Ok(columns)
}

fn select_sqlite_expr(columns: &HashSet<String>, names: &[&str], fallback: &str) -> String {
    for name in names {
        if columns.contains(&name.to_ascii_lowercase()) {
            return format!("CAST({name} AS TEXT)");
        }
    }
    fallback.to_string()
}

fn scan_generic_sqlite_sessions(
    tool_id: &str,
    db_path: &std::path::Path,
    query: &str,
) -> Vec<SessionSummary> {
    let external = match rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(conn) => conn,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    let mut seen_ids = HashSet::new();

    for table_name in ["threads", "sessions", "conversations"] {
        let columns = match sqlite_table_columns(&external, table_name) {
            Ok(columns) if !columns.is_empty() => columns,
            _ => continue,
        };

        let id_column = if columns.contains("id") {
            "id"
        } else if columns.contains("session_id") {
            "session_id"
        } else if columns.contains("thread_id") {
            "thread_id"
        } else {
            continue;
        };
        let title_expr = select_sqlite_expr(&columns, &["title", "name"], "''");
        let cwd_expr = select_sqlite_expr(
            &columns,
            &["cwd", "working_directory", "project_path"],
            "NULL",
        );
        let created_expr = select_sqlite_expr(
            &columns,
            &["created_at", "created_ts", "timestamp", "ts"],
            "NULL",
        );
        let updated_expr = select_sqlite_expr(
            &columns,
            &[
                "updated_at",
                "updated_ts",
                "last_updated_at",
                "timestamp",
                "ts",
            ],
            "NULL",
        );
        let sort_column = if columns.contains("updated_at") {
            "updated_at"
        } else if columns.contains("timestamp") {
            "timestamp"
        } else if columns.contains("created_at") {
            "created_at"
        } else {
            "rowid"
        };

        let sql = format!(
            "SELECT CAST({id_column} AS TEXT), {title_expr}, {cwd_expr}, {created_expr}, {updated_expr}
             FROM {table_name}
             ORDER BY {sort_column} DESC
             LIMIT 200"
        );
        let mut stmt = match external.prepare(&sql) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let rows = match stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => continue,
        };

        for row in rows.flatten() {
            let (id, title_raw, cwd_raw, created_raw, updated_raw) = row;
            if !seen_ids.insert(id.clone()) {
                continue;
            }
            let title = title_raw
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{table_name} {id}"));
            let cwd = cwd_raw
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let preview = cwd
                .as_ref()
                .map(|value| truncate_session_text(value, 180))
                .unwrap_or_else(|| truncate_session_text(&title, 180));
            let search_values = vec![
                title.clone(),
                preview.clone(),
                cwd.clone().unwrap_or_default(),
            ];
            let search_hit_count = count_query_hits(query, &search_values);
            if !query.is_empty() && search_hit_count == 0 {
                continue;
            }

            sessions.push(SessionSummary {
                id,
                tool_id: tool_id.to_string(),
                tool_name: tool_label(tool_id).to_string(),
                title,
                cwd,
                source_kind: format!("{tool_id}_sqlite"),
                source_backend: "sqlite".to_string(),
                source_path: db_path.to_string_lossy().to_string(),
                created_at: created_raw.as_deref().and_then(format_timestamp_text),
                updated_at: updated_raw.as_deref().and_then(format_timestamp_text),
                preview,
                message_count: 0,
                input_tokens: None,
                output_tokens: None,
                tokens_used: None,
                search_hit_count,
                can_resume: tool_supports_session_resume(tool_id),
                can_delete: false,
            });
        }
    }

    sessions
}

fn scan_generic_tool_sessions(
    conn: &rusqlite::Connection,
    tool_id: &str,
    query: &str,
) -> Result<Vec<SessionSummary>, String> {
    let mut jsonl_files = Vec::new();
    let mut sqlite_files = Vec::new();
    let mut seen_jsonl = HashSet::new();
    let mut seen_sqlite = HashSet::new();

    for root in session_roots_for_tool(conn, tool_id)? {
        collect_session_candidate_files(
            tool_id,
            &root,
            &root,
            &mut jsonl_files,
            &mut sqlite_files,
            0,
        );
    }

    let mut sessions = Vec::new();
    for path in jsonl_files {
        let key = path.to_string_lossy().to_string();
        if !seen_jsonl.insert(key) {
            continue;
        }
        if let Some(summary) = parse_generic_jsonl_session_summary(tool_id, &path, query) {
            sessions.push(summary);
        }
    }

    for path in sqlite_files {
        let key = path.to_string_lossy().to_string();
        if !seen_sqlite.insert(key) {
            continue;
        }
        sessions.extend(scan_generic_sqlite_sessions(tool_id, &path, query));
    }

    Ok(sessions)
}

fn scan_sessions_from_conn(
    conn: &rusqlite::Connection,
    tool_id: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SessionSummary>, String> {
    let query = normalize_session_query(query);
    let max_results = limit.unwrap_or(200).clamp(1, 500);
    let requested_tool = tool_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let tool_ids: Vec<&str> = match requested_tool {
        Some("claude") => vec!["claude"],
        Some("codex") => vec!["codex"],
        Some("gemini") => vec!["gemini"],
        Some("opencode") => vec!["opencode"],
        Some("openclaw") => vec!["openclaw"],
        Some("hermes") => vec!["hermes"],
        _ => vec!["claude", "codex", "gemini", "opencode", "openclaw", "hermes"],
    };

    let mut sessions = Vec::new();
    for tool in tool_ids {
        if tool == "codex" {
            sessions.extend(scan_codex_sessions(conn, &query)?);
        } else {
            sessions.extend(scan_generic_tool_sessions(conn, tool, &query)?);
        }
    }

    sessions.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
    });
    sessions.truncate(max_results);
    Ok(sessions)
}

fn codex_message_content(content: Option<&serde_json::Value>) -> String {
    let mut texts = Vec::new();
    if let Some(content) = content {
        preferred_texts_from_value(content, &mut texts, 0);
    }
    texts.join("\n\n")
}

fn parse_codex_session_entries(path: &std::path::Path) -> Result<Vec<SessionEntry>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();

    for (index, line) in BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let timestamp = value
            .get("timestamp")
            .and_then(|item| item.as_str())
            .and_then(format_timestamp_text);
        let item_type = value
            .get("type")
            .and_then(|item| item.as_str())
            .unwrap_or_default();

        match item_type {
            "response_item" => {
                let Some(payload) = value.get("payload") else {
                    continue;
                };
                let payload_type = payload
                    .get("type")
                    .and_then(|item| item.as_str())
                    .unwrap_or_default();
                match payload_type {
                    "message" => {
                        let role = payload
                            .get("role")
                            .and_then(|item| item.as_str())
                            .unwrap_or("assistant");
                        if matches!(role, "developer" | "system") {
                            continue;
                        }
                        let content = codex_message_content(payload.get("content"));
                        if content.trim().is_empty() {
                            continue;
                        }
                        entries.push(SessionEntry {
                            id: format!("entry-{index}"),
                            kind: role.to_string(),
                            title: match role {
                                "user" => "User".to_string(),
                                "assistant" => "Assistant".to_string(),
                                _ => role.to_string(),
                            },
                            content,
                            timestamp,
                        });
                    }
                    "function_call" => {
                        let name = payload
                            .get("name")
                            .and_then(|item| item.as_str())
                            .unwrap_or("tool");
                        let content = payload
                            .get("arguments")
                            .and_then(|item| item.as_str())
                            .unwrap_or("")
                            .to_string();
                        entries.push(SessionEntry {
                            id: format!("entry-{index}"),
                            kind: "tool_call".to_string(),
                            title: format!("Call {name}"),
                            content,
                            timestamp,
                        });
                    }
                    "function_call_output" => {
                        let content = payload
                            .get("output")
                            .and_then(|item| item.as_str())
                            .unwrap_or("")
                            .to_string();
                        if content.trim().is_empty() {
                            continue;
                        }
                        entries.push(SessionEntry {
                            id: format!("entry-{index}"),
                            kind: "tool_output".to_string(),
                            title: "Tool Output".to_string(),
                            content,
                            timestamp,
                        });
                    }
                    "reasoning" => {
                        let mut texts = Vec::new();
                        if let Some(summary) = payload.get("summary") {
                            preferred_texts_from_value(summary, &mut texts, 0);
                        }
                        if texts.is_empty() {
                            continue;
                        }
                        entries.push(SessionEntry {
                            id: format!("entry-{index}"),
                            kind: "reasoning".to_string(),
                            title: "Reasoning".to_string(),
                            content: texts.join("\n\n"),
                            timestamp,
                        });
                    }
                    _ => {}
                }
            }
            "event_msg" => {
                let Some(payload_type) = value
                    .get("payload")
                    .and_then(|item| item.get("type"))
                    .and_then(|item| item.as_str())
                else {
                    continue;
                };
                if payload_type == "token_count" {
                    continue;
                }
                entries.push(SessionEntry {
                    id: format!("entry-{index}"),
                    kind: "event".to_string(),
                    title: payload_type.replace('_', " "),
                    content: payload_type.to_string(),
                    timestamp,
                });
            }
            "turn_context" => {
                let mut lines = Vec::new();
                if let Some(cwd) = value
                    .get("payload")
                    .and_then(|item| item.get("cwd"))
                    .and_then(|item| item.as_str())
                {
                    lines.push(format!("cwd: {cwd}"));
                }
                if let Some(model) = value
                    .get("payload")
                    .and_then(|item| item.get("model"))
                    .and_then(|item| item.as_str())
                {
                    lines.push(format!("model: {model}"));
                }
                if let Some(approval) = value
                    .get("payload")
                    .and_then(|item| item.get("approval_policy"))
                    .and_then(|item| item.as_str())
                {
                    lines.push(format!("approval: {approval}"));
                }
                if lines.is_empty() {
                    continue;
                }
                entries.push(SessionEntry {
                    id: format!("entry-{index}"),
                    kind: "note".to_string(),
                    title: "Context".to_string(),
                    content: lines.join("\n"),
                    timestamp,
                });
            }
            _ => {}
        }
    }

    Ok(entries)
}

fn parse_generic_jsonl_session_entries(
    path: &std::path::Path,
) -> Result<Vec<SessionEntry>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();

    for (index, line) in BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .enumerate()
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let mut texts = Vec::new();
        preferred_texts_from_value(&value, &mut texts, 0);
        let content = texts.join("\n\n");
        if content.trim().is_empty() {
            continue;
        }
        let kind = value
            .get("role")
            .and_then(|item| item.as_str())
            .or_else(|| value.get("type").and_then(|item| item.as_str()))
            .unwrap_or("entry")
            .to_string();
        let timestamp = value
            .get("timestamp")
            .and_then(|item| item.as_str())
            .and_then(format_timestamp_text)
            .or_else(|| {
                value
                    .get("ts")
                    .and_then(|item| item.as_i64())
                    .and_then(format_unix_timestamp)
            });
        entries.push(SessionEntry {
            id: format!("entry-{index}"),
            kind: kind.clone(),
            title: kind.replace('_', " "),
            content,
            timestamp,
        });
    }

    Ok(entries)
}

fn load_generic_sqlite_entries(
    db_path: &std::path::Path,
    session_id: &str,
) -> Result<Vec<SessionEntry>, String> {
    let external =
        rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;

    for table_name in ["messages", "entries", "events"] {
        let columns = match sqlite_table_columns(&external, table_name) {
            Ok(columns) if !columns.is_empty() => columns,
            _ => continue,
        };
        let session_column = if columns.contains("session_id") {
            "session_id"
        } else if columns.contains("thread_id") {
            "thread_id"
        } else if columns.contains("conversation_id") {
            "conversation_id"
        } else {
            continue;
        };
        let role_expr = select_sqlite_expr(&columns, &["role", "kind", "type"], "'entry'");
        let content_expr =
            select_sqlite_expr(&columns, &["content", "text", "body", "message"], "''");
        let timestamp_expr = select_sqlite_expr(
            &columns,
            &["created_at", "updated_at", "timestamp", "ts"],
            "NULL",
        );
        let sort_column = if columns.contains("created_at") {
            "created_at"
        } else if columns.contains("timestamp") {
            "timestamp"
        } else {
            "rowid"
        };

        let sql = format!(
            "SELECT {role_expr}, {content_expr}, {timestamp_expr}
             FROM {table_name}
             WHERE CAST({session_column} AS TEXT) = ?1
             ORDER BY {sort_column} ASC
             LIMIT 400"
        );
        let mut stmt = match external.prepare(&sql) {
            Ok(stmt) => stmt,
            Err(_) => continue,
        };

        let rows = match stmt.query_map(rusqlite::params![session_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => continue,
        };

        let mut entries = Vec::new();
        for (index, row) in rows.flatten().enumerate() {
            let (role, content, timestamp) = row;
            let content = content.unwrap_or_default();
            if content.trim().is_empty() {
                continue;
            }
            let kind = role.unwrap_or_else(|| "entry".to_string());
            entries.push(SessionEntry {
                id: format!("sqlite-entry-{index}"),
                kind: kind.clone(),
                title: kind.replace('_', " "),
                content,
                timestamp: timestamp.as_deref().and_then(format_timestamp_text),
            });
        }
        if !entries.is_empty() {
            return Ok(entries);
        }
    }

    Ok(vec![SessionEntry {
        id: "sqlite-fallback".to_string(),
        kind: "note".to_string(),
        title: "Metadata".to_string(),
        content: format!("Session metadata is stored in {}", db_path.display()),
        timestamp: None,
    }])
}

fn load_session_detail(session: &SessionSummary) -> Result<SessionDetail, String> {
    let source_path = std::path::PathBuf::from(&session.source_path);
    let entries = if session.tool_id == "codex" && session.source_kind == "codex_jsonl" {
        parse_codex_session_entries(&source_path)?
    } else if session.source_backend == "jsonl" {
        parse_generic_jsonl_session_entries(&source_path)?
    } else {
        load_generic_sqlite_entries(&source_path, &session.id)?
    };

    Ok(SessionDetail {
        session: session.clone(),
        entries,
    })
}

fn is_valid_session_source_path(
    conn: &rusqlite::Connection,
    tool_id: &str,
    source_path: &str,
) -> bool {
    let source = PathBuf::from(source_path);
    let normalized_source = source.canonicalize().unwrap_or(source);
    let Ok(roots) = session_roots_for_tool(conn, tool_id) else {
        return false;
    };

    roots.into_iter().any(|root| {
        let normalized_root = root.canonicalize().unwrap_or(root);
        normalized_source.starts_with(&normalized_root)
    })
}

fn scrub_codex_history(root: &std::path::Path, session_id: &str) -> Result<(), String> {
    let history_path = root.join("history.jsonl");
    if !history_path.exists() {
        return Ok(());
    }

    let file = std::fs::File::open(&history_path).map_err(|e| e.to_string())?;
    let mut kept_lines = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let keep = serde_json::from_str::<serde_json::Value>(&line)
            .ok()
            .and_then(|value| {
                value
                    .get("session_id")
                    .and_then(|item| item.as_str())
                    .map(|id| id != session_id)
            })
            .unwrap_or(true);
        if keep {
            kept_lines.push(line);
        }
    }

    let content = if kept_lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", kept_lines.join("\n"))
    };
    crate::utils::atomic_write_string(&history_path, &content).map_err(|e| e.to_string())
}

fn delete_codex_session_records(root: &std::path::Path, session_id: &str) -> Result<(), String> {
    scrub_codex_history(root, session_id)?;

    for db_path in codex_state_databases(root) {
        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(conn) => conn,
            Err(_) => continue,
        };
        let _ = conn.execute(
            "DELETE FROM thread_dynamic_tools WHERE thread_id = ?1",
            rusqlite::params![session_id],
        );
        let _ = conn.execute(
            "DELETE FROM thread_spawn_edges WHERE child_thread_id = ?1 OR parent_thread_id = ?1",
            rusqlite::params![session_id],
        );
        let _ = conn.execute(
            "DELETE FROM agent_job_items WHERE assigned_thread_id = ?1",
            rusqlite::params![session_id],
        );
        let _ = conn.execute(
            "DELETE FROM threads WHERE id = ?1",
            rusqlite::params![session_id],
        );
    }

    Ok(())
}

fn delete_session_impl(
    conn: &rusqlite::Connection,
    tool_id: &str,
    session_id: &str,
    source_path: &str,
    source_backend: &str,
) -> Result<(), String> {
    if !is_valid_session_source_path(conn, tool_id, source_path) {
        return Err("Invalid session source path".to_string());
    }
    let root = resolve_tool_config_dir(conn, tool_id)?;

    if tool_id == "codex" {
        delete_codex_session_records(&root, session_id)?;
    }

    if source_backend == "jsonl" {
        let path = PathBuf::from(source_path);
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_sessions(
    tool_id: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
    db: State<'_, DbState>,
) -> Result<Vec<SessionSummary>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        scan_sessions_from_conn(&conn, tool_id, query, limit)
    })();
    log_command_timing("get_sessions", started_at);
    result
}

#[tauri::command]
pub fn get_session_detail(
    tool_id: String,
    session_id: String,
    source_path: String,
    source_kind: String,
    source_backend: String,
    cwd: Option<String>,
    title: String,
    preview: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    message_count: usize,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    tokens_used: Option<u64>,
    can_resume: bool,
    can_delete: bool,
    db: State<'_, DbState>,
) -> Result<SessionDetail, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if !is_valid_session_source_path(&conn, &tool_id, &source_path) {
            return Err("Invalid session source path".to_string());
        }

        let summary = SessionSummary {
            id: session_id,
            tool_id: tool_id.clone(),
            tool_name: tool_label(&tool_id).to_string(),
            title,
            cwd,
            source_kind,
            source_backend,
            source_path,
            created_at,
            updated_at,
            preview,
            message_count,
            input_tokens,
            output_tokens,
            tokens_used,
            search_hit_count: 0,
            can_resume,
            can_delete,
        };
        load_session_detail(&summary)
    })();
    log_command_timing("get_session_detail", started_at);
    result
}

#[tauri::command]
pub fn delete_session(
    tool_id: String,
    session_id: String,
    source_path: String,
    source_backend: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    delete_session_impl(&conn, &tool_id, &session_id, &source_path, &source_backend)
}

#[tauri::command]
pub fn delete_sessions(
    sessions: Vec<SessionDeleteTarget>,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut deleted = 0usize;

    for session in sessions {
        delete_session_impl(
            &conn,
            &session.tool_id,
            &session.session_id,
            &session.source_path,
            &session.source_backend,
        )?;
        deleted += 1;
    }

    Ok(deleted)
}

/// Write a tool's config file content
#[tauri::command]
pub fn write_tool_config(
    tool_id: String,
    content: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    apply_tool_snapshot(&conn, &tool_id, &content)?;
    crate::utils::append_runtime_log(
        "info",
        "tools",
        &format!("Updated tool config for {tool_id}"),
    );
    Ok(())
}

#[tauri::command]
pub fn read_codex_toml_structured(
    path: Option<String>,
    db: State<'_, DbState>,
) -> Result<CodexTomlStructuredConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (config_path, auth_path) = resolve_codex_structured_paths(&conn, path)?;
    let content = if config_path.exists() {
        std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let auth = read_json_file_or_default(&auth_path)?;
    let api_key = auth
        .get("OPENAI_API_KEY")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    Ok(read_codex_structured_config_from_content(&content, api_key))
}

#[tauri::command]
pub fn write_codex_toml_structured(
    path: Option<String>,
    raw_toml: String,
    config: CodexTomlStructuredConfig,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (config_path, auth_path) = resolve_codex_structured_paths(&conn, path)?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let written_toml = write_codex_structured_config_to_text(&raw_toml, &config);
    crate::utils::atomic_write_string(&config_path, &written_toml).map_err(|e| e.to_string())?;

    let mut auth = read_json_file_or_default(&auth_path)?;
    if !auth.is_object() {
        auth = serde_json::json!({});
    }
    if let Some(api_key) = normalized_non_empty(&config.api_key) {
        auth["OPENAI_API_KEY"] = serde_json::json!(api_key);
    } else if let Some(auth_obj) = auth.as_object_mut() {
        auth_obj.remove("OPENAI_API_KEY");
    }
    write_json_file_pretty(&auth_path, &auth)?;

    Ok(written_toml)
}

#[tauri::command]
pub fn get_common_config_snippet(
    tool_id: String,
    db: State<'_, DbState>,
) -> Result<CommonConfigSnippet, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_common_config_snippet_from_conn(&conn, &tool_id)
}

#[tauri::command]
pub fn set_common_config_snippet(
    tool_id: String,
    snippet: CommonConfigSnippet,
    db: State<'_, DbState>,
) -> Result<CommonConfigSnippet, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    write_common_config_snippet_to_conn(&conn, &tool_id, snippet)
}

#[tauri::command]
pub fn read_claude_config_toggles(db: State<'_, DbState>) -> Result<ClaudeConfigToggles, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_claude_config_toggles_from_conn(&conn)
}

#[tauri::command]
pub fn write_claude_config_toggle(
    key: String,
    enabled: bool,
    db: State<'_, DbState>,
) -> Result<ClaudeConfigToggles, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    write_claude_config_toggle_to_conn(&conn, &key, enabled)
}

/// Get Claude Code permissions level (0=strict, 1=standard, 2=relaxed, 3=bypass)
#[tauri::command]
pub fn get_claude_permissions_level() -> Result<u32, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok(0);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let mode = settings
        .get("permissions")
        .and_then(|p| p.get("defaultMode"))
        .and_then(|m| m.as_str())
        .unwrap_or("");

    if mode == "bypassPermissions" {
        return Ok(3);
    }

    let allow = settings
        .get("permissions")
        .and_then(|p| p.get("allow"))
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    // NOTE: level 3 is already short-circuited above via `mode == "bypassPermissions"`.
    // The setter writes level 2 with Write(*) but NOT Bash(*), so checking both
    // here misses level 2 and falsely reports it as level 1. Use Write(*) alone.
    if allow.iter().any(|s| *s == "Write(*)") {
        Ok(2)
    } else if allow.iter().any(|s| *s == "Read(*)") {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Set Claude Code permissions level (0=strict, 1=standard, 2=relaxed, 3=bypass)
#[tauri::command]
pub fn set_claude_permissions_level(level: u32) -> Result<u32, String> {
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
        1 => (
            vec!["Read(*)", "Glob(*)", "Grep(*)", "WebSearch(*)"],
            "normal",
            false,
        ),
        2 => (
            vec![
                "Read(*)",
                "Write(*)",
                "Edit(*)",
                "Glob(*)",
                "Grep(*)",
                "WebFetch(*)",
                "WebSearch(*)",
                "Agent(*)",
                "NotebookEdit(*)",
            ],
            "normal",
            false,
        ),
        3 => (
            vec![
                "Bash(*)",
                "Read(*)",
                "Write(*)",
                "Edit(*)",
                "Glob(*)",
                "Grep(*)",
                "WebFetch(*)",
                "WebSearch(*)",
                "Agent(*)",
                "NotebookEdit(*)",
                "Skill(*)",
                "mcp__*",
            ],
            "bypassPermissions",
            true,
        ),
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
    Ok(level)
}

/// Get Claude Code auto-update channel
#[tauri::command]
pub fn get_claude_auto_update() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok("latest".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    // Canonical disable: env.DISABLE_AUTOUPDATER = "1". Check this first, since
    // Claude Code's autoUpdatesChannel only accepts "stable"/"latest" — there is
    // no "disabled" channel value, so removing the key alone round-trips back
    // to the default ("latest").
    let disabled = settings
        .get("env")
        .and_then(|e| e.get("DISABLE_AUTOUPDATER"))
        .and_then(|v| v.as_str())
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if disabled {
        return Ok("disabled".to_string());
    }
    Ok(settings
        .get("autoUpdatesChannel")
        .and_then(|v| v.as_str())
        .unwrap_or("latest")
        .to_string())
}

/// Set Claude Code auto-update channel
#[tauri::command]
pub fn set_claude_auto_update(channel: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };
    if channel == "disabled" {
        // Canonical disable: env.DISABLE_AUTOUPDATER = "1". Also clear the
        // channel key so the getter has a single source of truth.
        if !settings.is_object() {
            settings = serde_json::json!({});
        }
        let obj = settings.as_object_mut().unwrap();
        obj.remove("autoUpdatesChannel");
        let env_entry = obj
            .entry("env".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !env_entry.is_object() {
            *env_entry = serde_json::json!({});
        }
        env_entry
            .as_object_mut()
            .unwrap()
            .insert("DISABLE_AUTOUPDATER".to_string(), serde_json::json!("1"));
    } else {
        // Re-enable: clear the env flag (if present) and write the channel.
        if let Some(env_obj) = settings.get_mut("env").and_then(|v| v.as_object_mut()) {
            env_obj.remove("DISABLE_AUTOUPDATER");
        }
        // Drop an empty env object to keep settings.json tidy.
        if settings
            .get("env")
            .and_then(|v| v.as_object())
            .map(|o| o.is_empty())
            .unwrap_or(false)
        {
            settings.as_object_mut().map(|o| o.remove("env"));
        }
        settings["autoUpdatesChannel"] = serde_json::json!(channel);
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(channel)
}

/// Get Codex CLI settings (approval_mode, reasoning_effort, disable_response_storage)
#[tauri::command]
pub fn get_codex_settings() -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".codex").join("config.toml");
    if !path.exists() {
        return Ok(serde_json::json!({
            "approval_mode": "suggest",
            "reasoning_effort": "medium",
            "disable_response_storage": false,
            "context_window_1m": false,
        }));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // NOTE: `content.parse::<toml::Value>()` is broken in toml 1.0 — the
    // `FromStr for Value` impl only parses a single TOML value expression,
    // not a whole document, so it fails on any real config.toml with
    // "unexpected content, expected nothing". Parse as `toml::Table` instead.
    let doc: toml::Table = content
        .parse()
        .map_err(|e: toml::de::Error| e.to_string())?;

    // Read approval mode from personality or dedicated field
    let personality = doc
        .get("personality")
        .and_then(|v| v.as_str())
        .unwrap_or("pragmatic");
    let approval_mode = if personality == "full-auto" {
        "full-auto"
    } else if personality == "auto-edit" {
        "auto-edit"
    } else {
        "suggest"
    };

    let reasoning = doc
        .get("model_reasoning_effort")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");
    let disable_storage = doc
        .get("disable_response_storage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let context_window_1m = doc
        .get("model_context_window")
        .and_then(|v| v.as_integer())
        .is_some_and(|value| value == 1_000_000);

    Ok(serde_json::json!({
        "approval_mode": approval_mode,
        "reasoning_effort": reasoning,
        "disable_response_storage": disable_storage,
        "context_window_1m": context_window_1m,
    }))
}

/// Set a Codex CLI setting
#[tauri::command]
pub fn set_codex_setting(key: String, value: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".codex").join("config.toml");

    let content = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;

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
        "context_window_1m" => {
            if value == "true" {
                doc["model_context_window"] = toml_edit::value(1_000_000);
            } else {
                doc.remove("model_context_window");
            }
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
    if !path.exists() {
        return Ok("".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(settings
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

/// Set Claude Code model
#[tauri::command]
pub fn set_claude_model(model: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };
    settings["model"] = serde_json::json!(model);
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(settings["model"].as_str().unwrap_or_default().to_string())
}

/// Get Claude Code Tool Search (ENABLE_TOOL_SEARCH) status from settings.local.json
#[tauri::command]
pub fn get_claude_tool_search(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_claude_config_toggles_from_conn(&conn)?.enable_tool_search)
}

/// Set Claude Code Tool Search (ENABLE_TOOL_SEARCH) in settings.local.json
#[tauri::command]
pub fn set_claude_tool_search(enabled: bool, db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let _ = write_claude_config_toggle_to_conn(&conn, "enableToolSearch", enabled)?;
    Ok(enabled)
}

// ── StatusLine (claude-hud) ──

/// Check if claude-hud plugin is installed and return its status + config
#[tauri::command]
pub fn get_claude_hud_status() -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let cache_dir = home
        .join(".claude")
        .join("plugins")
        .join("cache")
        .join("claude-hud")
        .join("claude-hud");

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
        let settings: serde_json::Value =
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
        settings
            .get("statusLine")
            .and_then(|s| s.get("command"))
            .and_then(|c| c.as_str())
            .is_some()
    } else {
        false
    };

    // Read claude-hud config
    let hud_config_path = home
        .join(".claude")
        .join("plugins")
        .join("claude-hud")
        .join("config.json");
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

/// Write env-entry.ts (for bun) and env-entry.mjs (for node) into the plugin version directory.
/// These entry points read stdin from the CLAUDE_HUD_STDIN env var instead of piped stdin,
/// which avoids Windows pipe timing issues with Claude Code's statusline renderer.
fn write_hud_env_entries(version_dir: &std::path::Path) -> Result<(), String> {
    // env-entry.ts — for bun (native TypeScript support)
    let ts_content = r#"// Entry point that reads stdin from env var instead of piped stdin
// This avoids the Windows pipe timing issue with Claude Code's statusline renderer
import { main } from './src/index.ts';

const stdinRaw = process.env.CLAUDE_HUD_STDIN || '';
let stdinData = null;
try {
  if (stdinRaw.trim()) {
    stdinData = JSON.parse(stdinRaw);
  }
} catch {}

await main({
  readStdin: async () => stdinData,
});
"#;

    // env-entry.mjs — for node (compiled JS fallback)
    let mjs_content = r#"// Entry point that reads stdin from env var instead of piped stdin
// Node.js fallback version using compiled dist/index.js
const { main } = await import('./dist/index.js');

const stdinRaw = process.env.CLAUDE_HUD_STDIN || '';
let stdinData = null;
try {
  if (stdinRaw.trim()) {
    stdinData = JSON.parse(stdinRaw);
  }
} catch {}

await main({
  readStdin: async () => stdinData,
});
"#;

    let ts_path = version_dir.join("env-entry.ts");
    let mjs_path = version_dir.join("env-entry.mjs");

    std::fs::write(&ts_path, ts_content)
        .map_err(|e| format!("Write env-entry.ts failed: {}", e))?;
    std::fs::write(&mjs_path, mjs_content)
        .map_err(|e| format!("Write env-entry.mjs failed: {}", e))?;

    Ok(())
}

/// Build the statusLine command string for Windows-compatible multi-line claude-hud output.
/// Uses env var stdin + tr -d '\r' to fix Windows line ending issues.
fn build_hud_statusline_command(home: &std::path::Path) -> Result<String, String> {
    let cache_dir = home
        .join(".claude")
        .join("plugins")
        .join("cache")
        .join("claude-hud")
        .join("claude-hud");

    // Find the installed version directory
    let mut version = String::new();
    if cache_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let candidate = entry.path().join("dist").join("index.js");
                if candidate.exists() {
                    version = entry.file_name().to_string_lossy().to_string();
                    break;
                }
            }
        }
    }
    if version.is_empty() {
        return Err("claude-hud not installed".to_string());
    }

    // Detect bun path
    let bun_path = find_bun_path(home);

    // Build the command:
    // 1. Read stdin to env var (avoids pipe timing issues)
    // 2. Find latest plugin version dir
    // 3. Run with bun (preferred) or node
    // 4. Strip \r from output (Windows line ending fix)
    let cmd = if let Some(bun) = &bun_path {
        // Use bun with env-entry.ts (native TypeScript)
        format!(
            "bash -c 'export CLAUDE_HUD_STDIN=$(cat); \
plugin_dir=$(ls -d \"${{CLAUDE_CONFIG_DIR:-$HOME/.claude}}\"/plugins/cache/claude-hud/claude-hud/*/ 2>/dev/null \
| awk -F/ '\"'\"'{{ print $(NF-1) \"\\t\" $(0) }}'\"'\"' \
| sort -t. -k1,1n -k2,2n -k3,3n -k4,4n | tail -1 | cut -f2-); \
\"{}\" --env-file /dev/null \"${{plugin_dir}}env-entry.ts\" 2>/dev/null | tr -d \"\\r\"'",
            bun
        )
    } else {
        // Fallback to node with env-entry.mjs
        format!(
            "bash -c 'export CLAUDE_HUD_STDIN=$(cat); \
plugin_dir=$(ls -d \"${{CLAUDE_CONFIG_DIR:-$HOME/.claude}}\"/plugins/cache/claude-hud/claude-hud/*/ 2>/dev/null \
| awk -F/ '\"'\"'{{ print $(NF-1) \"\\t\" $(0) }}'\"'\"' \
| sort -t. -k1,1n -k2,2n -k3,3n -k4,4n | tail -1 | cut -f2-); \
node \"${{plugin_dir}}env-entry.mjs\" 2>/dev/null | tr -d \"\\r\"'"
        )
    };

    Ok(cmd)
}

/// Find bun executable path, checking common locations
fn find_bun_path(home: &std::path::Path) -> Option<String> {
    // Check common bun install locations on Windows (Git Bash paths)
    let candidates = [
        home.join(".bun").join("bin").join("bun"),
        home.join(".bun").join("bin").join("bun.exe"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            // Convert to Git Bash compatible path: C:\Users\xxx -> /c/Users/xxx
            let path_str = candidate.to_string_lossy().replace('\\', "/");
            if let Some(rest) = path_str
                .strip_prefix("C:")
                .or_else(|| path_str.strip_prefix("c:"))
            {
                return Some(format!("/c{}", rest));
            }
            // Other drive letters
            if path_str.len() >= 2 && path_str.as_bytes()[1] == b':' {
                let drive = path_str.as_bytes()[0].to_ascii_lowercase() as char;
                return Some(format!("/{}{}", drive, &path_str[2..]));
            }
            return Some(path_str.to_string());
        }
    }

    // Also check if bun is in PATH via which
    let probe = if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("where");
        command.arg("bun");
        command
    } else {
        let mut command = std::process::Command::new("which");
        command.arg("bun");
        command
    };
    let mut probe = probe;
    configure_background_command(&mut probe);
    if let Ok(output) = probe.output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

/// Install claude-hud plugin from GitHub repository
#[tauri::command]
pub async fn install_claude_hud(db: State<'_, crate::db::DbState>) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    // Build HTTP client with proxy support
    let proxy_url = get_proxy(db);
    let client = if !proxy_url.is_empty() {
        let proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| format!("Invalid proxy: {}", e))?;
        reqwest::Client::builder()
            .proxy(proxy)
            .user_agent("CCHub")
            .build()
            .map_err(|e| format!("Client build failed: {}", e))?
    } else {
        reqwest::Client::builder()
            .user_agent("CCHub")
            .build()
            .map_err(|e| format!("Client build failed: {}", e))?
    };

    // Fetch plugin version from GitHub
    let plugin_json_urls = [
        "https://raw.githubusercontent.com/jarrodwatts/claude-hud/main/.claude-plugin/plugin.json",
        "https://ghgo.xyz/raw.githubusercontent.com/jarrodwatts/claude-hud/main/.claude-plugin/plugin.json",
    ];

    let mut version = String::new();
    for url in &plugin_json_urls {
        if let Ok(resp) = client.get(*url).send().await {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(v) = json.get("version").and_then(|v| v.as_str()) {
                            version = v.to_string();
                            break;
                        }
                    }
                }
            }
        }
    }
    if version.is_empty() {
        version = "0.0.11".to_string(); // fallback
    }

    let cache_dir = home
        .join(".claude")
        .join("plugins")
        .join("cache")
        .join("claude-hud")
        .join("claude-hud");
    let version_dir = cache_dir.join(&version);
    let dist_dir = version_dir.join("dist");

    // Skip if already installed
    if dist_dir.join("index.js").exists() {
        // Ensure env-entry files exist even for existing installs
        write_hud_env_entries(&version_dir)?;
        return Ok(());
    }

    std::fs::create_dir_all(&dist_dir).map_err(|e| e.to_string())?;

    // Download tarball from GitHub
    let tarball_urls = [
        "https://github.com/jarrodwatts/claude-hud/archive/refs/heads/main.tar.gz",
        "https://ghgo.xyz/github.com/jarrodwatts/claude-hud/archive/refs/heads/main.tar.gz",
    ];

    let mut bytes = None;
    let mut last_err = String::new();
    for url in &tarball_urls {
        match client.get(*url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(b) => {
                    bytes = Some(b);
                    break;
                }
                Err(e) => last_err = format!("Read failed: {}", e),
            },
            Ok(resp) => last_err = format!("HTTP {} from {}", resp.status(), url),
            Err(e) => last_err = format!("Download failed: {}", e),
        }
    }
    let bytes = bytes.ok_or(format!("All sources failed: {}", last_err))?;

    // Extract tarball: GitHub format is claude-hud-main/{dist,src}/*
    let gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| format!("Tar read failed: {}", e))?;

    // Extract both dist/ and src/ (src/ needed for bun TypeScript support)
    let prefix_candidates = [
        "claude-hud-main/dist/",
        "claude-hud-master/dist/",
        "claude-hud-main/src/",
        "claude-hud-master/src/",
    ];

    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Tar entry error: {}", e))?;
        let entry_path = entry
            .path()
            .map_err(|e| format!("Path error: {}", e))?
            .to_path_buf();
        let entry_str = entry_path.to_string_lossy().replace('\\', "/");

        for prefix in &prefix_candidates {
            if entry_str.starts_with(prefix) {
                // Strip the GitHub prefix (e.g., "claude-hud-main/") to get "dist/..." or "src/..."
                let repo_prefix = prefix.split('/').next().unwrap_or("claude-hud-main");
                let relative = entry_str
                    .strip_prefix(&format!("{}/", repo_prefix))
                    .unwrap_or(&entry_str);
                if relative.is_empty() || relative.ends_with('/') {
                    continue;
                }
                let target = version_dir.join(relative);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut file = std::fs::File::create(&target).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
                break;
            }
        }
    }

    // Verify index.js exists
    if !dist_dir.join("index.js").exists() {
        return Err("Installation failed: index.js not found after extraction".to_string());
    }

    // Write env-entry files (for Windows stdin pipe fix)
    write_hud_env_entries(&version_dir)?;

    // Create default hud config
    let hud_config_dir = home.join(".claude").join("plugins").join("claude-hud");
    std::fs::create_dir_all(&hud_config_dir).map_err(|e| e.to_string())?;
    let hud_config_path = hud_config_dir.join("config.json");
    if !hud_config_path.exists() {
        let default_config = serde_json::json!({
            "lineLayout": "expanded",
            "showSeparators": false,
            "pathLevels": 1,
            "gitStatus": {
                "enabled": true,
                "showDirty": true,
                "showAheadBehind": false,
                "showFileStats": false
            },
            "display": {
                "showModel": true,
                "showContextBar": true,
                "showUsage": true,
                "usageBarEnabled": true,
                "showTokenBreakdown": true
            }
        });
        let config_str =
            serde_json::to_string_pretty(&default_config).map_err(|e| e.to_string())?;
        crate::utils::atomic_write_string(&hud_config_path, &config_str)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Check if there's a newer version of claude-hud on GitHub
#[tauri::command]
pub async fn check_claude_hud_update(
    db: State<'_, crate::db::DbState>,
) -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let cache_dir = home
        .join(".claude")
        .join("plugins")
        .join("cache")
        .join("claude-hud")
        .join("claude-hud");

    // Get current installed version
    let mut current_version = String::new();
    if cache_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let candidate = entry.path().join("dist").join("index.js");
                if candidate.exists() {
                    current_version = entry.file_name().to_string_lossy().to_string();
                    break;
                }
            }
        }
    }

    if current_version.is_empty() {
        return Err("claude-hud not installed".to_string());
    }

    // Check latest version from GitHub plugin.json
    let proxy_url = get_proxy(db);
    let client = if !proxy_url.is_empty() {
        let proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| format!("Invalid proxy: {}", e))?;
        reqwest::Client::builder()
            .proxy(proxy)
            .user_agent("CCHub")
            .build()
            .map_err(|e| format!("Client build failed: {}", e))?
    } else {
        reqwest::Client::builder()
            .user_agent("CCHub")
            .build()
            .map_err(|e| format!("Client build failed: {}", e))?
    };

    let plugin_json_urls = [
        "https://raw.githubusercontent.com/jarrodwatts/claude-hud/main/.claude-plugin/plugin.json",
        "https://ghgo.xyz/raw.githubusercontent.com/jarrodwatts/claude-hud/main/.claude-plugin/plugin.json",
    ];

    let mut latest_version = String::new();
    for url in &plugin_json_urls {
        if let Ok(resp) = client.get(*url).send().await {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(v) = json.get("version").and_then(|v| v.as_str()) {
                            latest_version = v.to_string();
                            break;
                        }
                    }
                }
            }
        }
    }

    if latest_version.is_empty() {
        return Err("Failed to check latest version from GitHub".to_string());
    }

    let has_update = latest_version != current_version;

    Ok(serde_json::json!({
        "currentVersion": current_version,
        "latestVersion": latest_version,
        "hasUpdate": has_update,
    }))
}

/// Update claude-hud to the latest GitHub version
#[tauri::command]
pub async fn update_claude_hud(
    db: State<'_, crate::db::DbState>,
) -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let cache_dir = home
        .join(".claude")
        .join("plugins")
        .join("cache")
        .join("claude-hud")
        .join("claude-hud");

    // Build HTTP client with proxy
    let proxy_url = get_proxy(db);
    let client = if !proxy_url.is_empty() {
        let proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| format!("Invalid proxy: {}", e))?;
        reqwest::Client::builder()
            .proxy(proxy)
            .user_agent("CCHub")
            .build()
            .map_err(|e| format!("Client build failed: {}", e))?
    } else {
        reqwest::Client::builder()
            .user_agent("CCHub")
            .build()
            .map_err(|e| format!("Client build failed: {}", e))?
    };

    // Get latest version from GitHub plugin.json
    let plugin_json_urls = [
        "https://raw.githubusercontent.com/jarrodwatts/claude-hud/main/.claude-plugin/plugin.json",
        "https://ghgo.xyz/raw.githubusercontent.com/jarrodwatts/claude-hud/main/.claude-plugin/plugin.json",
    ];

    let mut version = String::new();
    for url in &plugin_json_urls {
        if let Ok(resp) = client.get(*url).send().await {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(v) = json.get("version").and_then(|v| v.as_str()) {
                            version = v.to_string();
                            break;
                        }
                    }
                }
            }
        }
    }

    if version.is_empty() {
        return Err("Failed to get latest version from GitHub".to_string());
    }

    let dist_dir = cache_dir.join(&version).join("dist");

    // Skip if already installed
    if dist_dir.join("index.js").exists() {
        return Ok(serde_json::json!({ "version": version, "skipped": true }));
    }

    std::fs::create_dir_all(&dist_dir).map_err(|e| e.to_string())?;

    // Download tarball from GitHub
    let tarball_urls = [
        "https://github.com/jarrodwatts/claude-hud/archive/refs/heads/main.tar.gz",
        "https://ghgo.xyz/github.com/jarrodwatts/claude-hud/archive/refs/heads/main.tar.gz",
    ];

    let mut bytes = None;
    let mut last_err = String::new();
    for url in &tarball_urls {
        match client.get(*url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(b) => {
                    bytes = Some(b);
                    break;
                }
                Err(e) => last_err = format!("Read failed: {}", e),
            },
            Ok(resp) => last_err = format!("HTTP {} from {}", resp.status(), url),
            Err(e) => last_err = format!("Download failed: {}", e),
        }
    }
    let bytes = bytes.ok_or(format!("All sources failed: {}", last_err))?;

    // Extract tarball
    let gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| format!("Tar read failed: {}", e))?;

    // Extract both dist/ and src/ (src/ needed for bun TypeScript support)
    let prefix_candidates = [
        "claude-hud-main/dist/",
        "claude-hud-master/dist/",
        "claude-hud-main/src/",
        "claude-hud-master/src/",
    ];

    let version_dir = cache_dir.join(&version);

    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Tar entry error: {}", e))?;
        let entry_path = entry
            .path()
            .map_err(|e| format!("Path error: {}", e))?
            .to_path_buf();
        let entry_str = entry_path.to_string_lossy().replace('\\', "/");

        for prefix in &prefix_candidates {
            if entry_str.starts_with(prefix) {
                let repo_prefix = prefix.split('/').next().unwrap_or("claude-hud-main");
                let relative = entry_str
                    .strip_prefix(&format!("{}/", repo_prefix))
                    .unwrap_or(&entry_str);
                if relative.is_empty() || relative.ends_with('/') {
                    continue;
                }
                let target = version_dir.join(relative);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut file = std::fs::File::create(&target).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
                break;
            }
        }
    }

    // Verify
    if !dist_dir.join("index.js").exists() {
        return Err("Update failed: index.js not found after extraction".to_string());
    }

    // Write env-entry files (for Windows stdin pipe fix)
    write_hud_env_entries(&version_dir)?;

    // Remove old version directories
    if let Ok(dir_entries) = std::fs::read_dir(&cache_dir) {
        for entry in dir_entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != version {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    // Update statusLine in settings.json with Windows-compatible command
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            if let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content) {
                if settings
                    .get("statusLine")
                    .and_then(|s| s.get("command"))
                    .is_some()
                {
                    let new_cmd = build_hud_statusline_command(&home)?;
                    settings["statusLine"]["command"] = serde_json::Value::String(new_cmd);
                    let out = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
                    crate::utils::atomic_write_string(&settings_path, &out)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    Ok(serde_json::json!({ "version": version, "skipped": false }))
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
        let cmd = build_hud_statusline_command(&home)?;

        settings["statusLine"] = serde_json::json!({
            "type": "command",
            "command": cmd
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
        if let Some(plugins) = settings
            .get_mut("enabledPlugins")
            .and_then(|p| p.as_object_mut())
        {
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
pub fn set_claude_hud_config(config: serde_json::Value) -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let config_dir = home.join(".claude").join("plugins").join("claude-hud");
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let config_path = config_dir.join("config.json");
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&config_path, &content).map_err(|e| e.to_string())?;
    get_claude_hud_status()
}

const HELLO2CC_PLUGIN_ID: &str = "hello2cc@hello2cc";
const HELLO2CC_MANIFEST_URLS: [&str; 2] = [
    "https://raw.githubusercontent.com/hellowind777/hello2cc/main/.claude-plugin/plugin.json",
    "https://ghgo.xyz/raw.githubusercontent.com/hellowind777/hello2cc/main/.claude-plugin/plugin.json",
];
const HELLO2CC_TARBALL_URLS: [&str; 2] = [
    "https://github.com/hellowind777/hello2cc/archive/refs/heads/main.tar.gz",
    "https://ghgo.xyz/github.com/hellowind777/hello2cc/archive/refs/heads/main.tar.gz",
];
const HELLO2CC_ROOT_PREFIXES: [&str; 2] = ["hello2cc-main/", "hello2cc-master/"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Hello2ccConfig {
    pub routing_policy: String,
    pub mirror_session_model: bool,
    pub default_agent_model: String,
    pub primary_model: String,
    pub subagent_model: String,
    pub guide_model: String,
    pub explore_model: String,
    pub plan_model: String,
    pub general_model: String,
    pub team_model: String,
    pub compatibility_mode: String,
}

impl Default for Hello2ccConfig {
    fn default() -> Self {
        Self {
            routing_policy: "native-inject".to_string(),
            mirror_session_model: true,
            default_agent_model: String::new(),
            primary_model: String::new(),
            subagent_model: String::new(),
            guide_model: String::new(),
            explore_model: String::new(),
            plan_model: String::new(),
            general_model: String::new(),
            team_model: String::new(),
            compatibility_mode: "full".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello2ccStatus {
    pub installed: bool,
    pub enabled: bool,
    pub version: String,
    pub install_path: String,
    pub settings_path: String,
    pub config: Hello2ccConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello2ccUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
}

fn claude_settings_path(home: &std::path::Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

fn hello2cc_cache_dir(home: &std::path::Path) -> PathBuf {
    home.join(".claude")
        .join("plugins")
        .join("cache")
        .join("hello2cc")
        .join("hello2cc")
}

fn hello2cc_required_paths(version_dir: &std::path::Path) -> [PathBuf; 4] {
    [
        version_dir.join(".claude-plugin").join("plugin.json"),
        version_dir.join(".claude-plugin").join("marketplace.json"),
        version_dir.join("agents").join("native.md"),
        version_dir
            .join("output-styles")
            .join("hello2cc-native.md"),
    ]
}

fn validate_hello2cc_install(version_dir: &std::path::Path, action: &str) -> Result<(), String> {
    for required_path in hello2cc_required_paths(version_dir) {
        if !required_path.exists() {
            return Err(format!("{} failed: missing {}", action, required_path.display()));
        }
    }

    Ok(())
}

fn ensure_json_object(
    value: &mut serde_json::Value,
) -> &mut serde_json::Map<String, serde_json::Value> {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    value.as_object_mut().expect("value should be an object")
}

fn ensure_child_object<'a>(
    parent: &'a mut serde_json::Value,
    key: &str,
) -> &'a mut serde_json::Map<String, serde_json::Value> {
    let parent_obj = ensure_json_object(parent);
    let entry = parent_obj
        .entry(key.to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !entry.is_object() {
        *entry = serde_json::json!({});
    }
    entry.as_object_mut().expect("value should be an object")
}

fn read_json_value_or_default(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }

    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }

    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn write_json_value(path: &std::path::Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(path, &content).map_err(|e| e.to_string())
}

fn normalize_hello2cc_mode(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("sanitize-only")
        || trimmed.eq_ignore_ascii_case("sanitize_only")
        || trimmed.eq_ignore_ascii_case("sanitizeonly")
    {
        "sanitize-only".to_string()
    } else {
        "full".to_string()
    }
}

fn normalize_hello2cc_routing_policy(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("prompt-only") {
        "prompt-only".to_string()
    } else {
        "native-inject".to_string()
    }
}

fn sanitize_hello2cc_config(config: Hello2ccConfig) -> Hello2ccConfig {
    Hello2ccConfig {
        routing_policy: normalize_hello2cc_routing_policy(&config.routing_policy),
        mirror_session_model: config.mirror_session_model,
        default_agent_model: config.default_agent_model.trim().to_string(),
        primary_model: config.primary_model.trim().to_string(),
        subagent_model: config.subagent_model.trim().to_string(),
        guide_model: config.guide_model.trim().to_string(),
        explore_model: config.explore_model.trim().to_string(),
        plan_model: config.plan_model.trim().to_string(),
        general_model: config.general_model.trim().to_string(),
        team_model: config.team_model.trim().to_string(),
        compatibility_mode: normalize_hello2cc_mode(&config.compatibility_mode),
    }
}

fn read_hello2cc_config_from_settings(settings: &serde_json::Value) -> Hello2ccConfig {
    let options = settings
        .get("pluginConfigs")
        .and_then(|value| value.get(HELLO2CC_PLUGIN_ID))
        .and_then(|value| value.get("options"))
        .and_then(|value| value.as_object());

    let string_value = |key: &str| {
        options
            .and_then(|opts| opts.get(key))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim()
            .to_string()
    };

    Hello2ccConfig {
        routing_policy: normalize_hello2cc_routing_policy(&string_value("routing_policy")),
        mirror_session_model: options
            .and_then(|opts| opts.get("mirror_session_model"))
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        default_agent_model: string_value("default_agent_model"),
        primary_model: string_value("primary_model"),
        subagent_model: string_value("subagent_model"),
        guide_model: string_value("guide_model"),
        explore_model: string_value("explore_model"),
        plan_model: string_value("plan_model"),
        general_model: string_value("general_model"),
        team_model: string_value("team_model"),
        compatibility_mode: normalize_hello2cc_mode(&string_value("compatibility_mode")),
    }
}

fn write_hello2cc_config_into_settings(
    settings: &mut serde_json::Value,
    config: Hello2ccConfig,
) -> Hello2ccConfig {
    let sanitized = sanitize_hello2cc_config(config);
    let plugin_configs = ensure_child_object(settings, "pluginConfigs");
    let plugin_config_entry = plugin_configs
        .entry(HELLO2CC_PLUGIN_ID.to_string())
        .or_insert_with(|| serde_json::json!({}));
    let options = ensure_child_object(plugin_config_entry, "options");

    options.insert(
        "routing_policy".to_string(),
        serde_json::Value::String(sanitized.routing_policy.clone()),
    );
    options.insert(
        "mirror_session_model".to_string(),
        serde_json::Value::Bool(sanitized.mirror_session_model),
    );
    options.insert(
        "default_agent_model".to_string(),
        serde_json::Value::String(sanitized.default_agent_model.clone()),
    );
    options.insert(
        "primary_model".to_string(),
        serde_json::Value::String(sanitized.primary_model.clone()),
    );
    options.insert(
        "subagent_model".to_string(),
        serde_json::Value::String(sanitized.subagent_model.clone()),
    );
    options.insert(
        "guide_model".to_string(),
        serde_json::Value::String(sanitized.guide_model.clone()),
    );
    options.insert(
        "explore_model".to_string(),
        serde_json::Value::String(sanitized.explore_model.clone()),
    );
    options.insert(
        "plan_model".to_string(),
        serde_json::Value::String(sanitized.plan_model.clone()),
    );
    options.insert(
        "general_model".to_string(),
        serde_json::Value::String(sanitized.general_model.clone()),
    );
    options.insert(
        "team_model".to_string(),
        serde_json::Value::String(sanitized.team_model.clone()),
    );
    options.insert(
        "compatibility_mode".to_string(),
        serde_json::Value::String(sanitized.compatibility_mode.clone()),
    );

    sanitized
}

fn parse_version_components(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.parse::<u64>().unwrap_or(0))
        .collect()
}

fn compare_version_like(left: &str, right: &str) -> Ordering {
    let left_parts = parse_version_components(left);
    let right_parts = parse_version_components(right);
    let max_len = left_parts.len().max(right_parts.len());

    for index in 0..max_len {
        let left_part = *left_parts.get(index).unwrap_or(&0);
        let right_part = *right_parts.get(index).unwrap_or(&0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => continue,
            non_equal => return non_equal,
        }
    }

    left.cmp(right)
}

fn find_latest_installed_plugin_version(
    cache_dir: &std::path::Path,
    required_relative_path: &std::path::Path,
) -> Option<(String, PathBuf)> {
    let entries = std::fs::read_dir(cache_dir).ok()?;
    let mut candidates = Vec::new();

    for entry in entries.flatten() {
        let version_dir = entry.path();
        if !version_dir.is_dir() {
            continue;
        }
        if version_dir.join(required_relative_path).exists() {
            candidates.push((entry.file_name().to_string_lossy().to_string(), version_dir));
        }
    }

    candidates.sort_by(|left, right| compare_version_like(&right.0, &left.0));
    candidates.into_iter().next()
}

fn build_plugin_http_client(proxy_url: &str) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().user_agent("CCHub");
    if !proxy_url.trim().is_empty() {
        let proxy =
            reqwest::Proxy::all(proxy_url).map_err(|e| format!("Invalid proxy: {}", e))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|e| format!("Client build failed: {}", e))
}

async fn fetch_plugin_version_from_manifest(
    client: &reqwest::Client,
    urls: &[&str],
) -> Result<String, String> {
    for url in urls {
        let response = match client.get(*url).send().await {
            Ok(response) if response.status().is_success() => response,
            _ => continue,
        };
        let text = response.text().await.map_err(|e| e.to_string())?;
        let manifest: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if let Some(version) = manifest.get("version").and_then(|value| value.as_str()) {
            let trimmed = version.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    Err("Failed to fetch plugin manifest version".to_string())
}

async fn download_first_available(
    client: &reqwest::Client,
    urls: &[&str],
) -> Result<bytes::Bytes, String> {
    let mut last_err = String::new();
    for url in urls {
        match client.get(*url).send().await {
            Ok(response) if response.status().is_success() => match response.bytes().await {
                Ok(bytes) => return Ok(bytes),
                Err(error) => last_err = format!("Read failed: {}", error),
            },
            Ok(response) => last_err = format!("HTTP {} from {}", response.status(), url),
            Err(error) => last_err = format!("Download failed: {}", error),
        }
    }

    Err(format!("All sources failed: {}", last_err))
}

fn extract_repo_tarball(
    bytes: &[u8],
    version_dir: &std::path::Path,
    root_prefixes: &[&str],
) -> Result<(), String> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| format!("Tar read failed: {}", e))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Tar entry error: {}", e))?;
        let entry_path = entry
            .path()
            .map_err(|e| format!("Path error: {}", e))?
            .to_path_buf();
        let entry_str = entry_path.to_string_lossy().replace('\\', "/");

        let relative = root_prefixes
            .iter()
            .find_map(|prefix| entry_str.strip_prefix(prefix))
            .unwrap_or("");
        if relative.is_empty() || relative.ends_with('/') {
            continue;
        }

        let relative_path = PathBuf::from(relative);
        if relative_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            continue;
        }

        let target = version_dir.join(relative_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn get_hello2cc_status_from_home(home: &std::path::Path) -> Result<Hello2ccStatus, String> {
    let settings_path = claude_settings_path(home);
    let settings = read_json_value_or_default(&settings_path)?;
    let cache_dir = hello2cc_cache_dir(home);
    let installed = find_latest_installed_plugin_version(
        &cache_dir,
        &PathBuf::from(".claude-plugin").join("plugin.json"),
    );

    let (version, install_path, is_installed) = if let Some((version, install_path)) = installed {
        (version, install_path.to_string_lossy().to_string(), true)
    } else {
        (String::new(), String::new(), false)
    };

    let enabled = settings
        .get("enabledPlugins")
        .and_then(|value| value.get(HELLO2CC_PLUGIN_ID))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    Ok(Hello2ccStatus {
        installed: is_installed,
        enabled,
        version,
        install_path,
        settings_path: settings_path.to_string_lossy().to_string(),
        config: read_hello2cc_config_from_settings(&settings),
    })
}

#[tauri::command]
pub fn get_hello2cc_status() -> Result<Hello2ccStatus, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    get_hello2cc_status_from_home(&home)
}

#[tauri::command]
pub fn get_hello2cc_config() -> Result<Hello2ccConfig, String> {
    Ok(get_hello2cc_status()?.config)
}

#[tauri::command]
pub fn set_hello2cc_config(config: Hello2ccConfig) -> Result<Hello2ccStatus, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let settings_path = claude_settings_path(&home);
    let mut settings = read_json_value_or_default(&settings_path)?;
    write_hello2cc_config_into_settings(&mut settings, config);
    write_json_value(&settings_path, &settings)?;
    get_hello2cc_status_from_home(&home)
}

#[tauri::command]
pub fn set_hello2cc_enabled(enabled: bool) -> Result<Hello2ccStatus, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let status = get_hello2cc_status_from_home(&home)?;
    if enabled && !status.installed {
        return Err("hello2cc not installed".to_string());
    }

    let settings_path = claude_settings_path(&home);
    let mut settings = read_json_value_or_default(&settings_path)?;
    let enabled_plugins = ensure_child_object(&mut settings, "enabledPlugins");
    if enabled {
        enabled_plugins.insert(
            HELLO2CC_PLUGIN_ID.to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        enabled_plugins.remove(HELLO2CC_PLUGIN_ID);
    }
    write_json_value(&settings_path, &settings)?;
    get_hello2cc_status_from_home(&home)
}

#[tauri::command]
pub fn uninstall_hello2cc() -> Result<Hello2ccStatus, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let cache_dir = hello2cc_cache_dir(&home);
    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    }

    let settings_path = claude_settings_path(&home);
    let mut settings = read_json_value_or_default(&settings_path)?;
    if let Some(enabled_plugins) = settings
        .get_mut("enabledPlugins")
        .and_then(|value| value.as_object_mut())
    {
        enabled_plugins.remove(HELLO2CC_PLUGIN_ID);
    }
    if let Some(plugin_configs) = settings
        .get_mut("pluginConfigs")
        .and_then(|value| value.as_object_mut())
    {
        plugin_configs.remove(HELLO2CC_PLUGIN_ID);
    }
    write_json_value(&settings_path, &settings)?;
    get_hello2cc_status_from_home(&home)
}

#[tauri::command]
pub async fn install_hello2cc(db: State<'_, crate::db::DbState>) -> Result<Hello2ccStatus, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let client = build_plugin_http_client(&get_proxy(db))?;
    let version = fetch_plugin_version_from_manifest(&client, &HELLO2CC_MANIFEST_URLS).await?;
    let cache_dir = hello2cc_cache_dir(&home);
    let version_dir = cache_dir.join(&version);
    let manifest_path = version_dir.join(".claude-plugin").join("plugin.json");

    if manifest_path.exists() {
        validate_hello2cc_install(&version_dir, "Installation")?;
        return get_hello2cc_status_from_home(&home);
    }

    if version_dir.exists() {
        std::fs::remove_dir_all(&version_dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;

    let bytes = download_first_available(&client, &HELLO2CC_TARBALL_URLS).await?;
    extract_repo_tarball(&bytes, &version_dir, &HELLO2CC_ROOT_PREFIXES)?;
    validate_hello2cc_install(&version_dir, "Installation")?;

    get_hello2cc_status_from_home(&home)
}

#[tauri::command]
pub async fn check_hello2cc_update(
    db: State<'_, crate::db::DbState>,
) -> Result<Hello2ccUpdateInfo, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let status = get_hello2cc_status_from_home(&home)?;
    if !status.installed {
        return Err("hello2cc not installed".to_string());
    }

    let client = build_plugin_http_client(&get_proxy(db))?;
    let latest_version = fetch_plugin_version_from_manifest(&client, &HELLO2CC_MANIFEST_URLS).await?;

    Ok(Hello2ccUpdateInfo {
        current_version: status.version.clone(),
        has_update: latest_version != status.version,
        latest_version,
    })
}

#[tauri::command]
pub async fn update_hello2cc(db: State<'_, crate::db::DbState>) -> Result<Hello2ccStatus, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let status = get_hello2cc_status_from_home(&home)?;
    if !status.installed {
        return Err("hello2cc not installed".to_string());
    }

    let client = build_plugin_http_client(&get_proxy(db))?;
    let version = fetch_plugin_version_from_manifest(&client, &HELLO2CC_MANIFEST_URLS).await?;
    let cache_dir = hello2cc_cache_dir(&home);
    let version_dir = cache_dir.join(&version);
    let manifest_path = version_dir.join(".claude-plugin").join("plugin.json");

    if !manifest_path.exists() {
        if version_dir.exists() {
            std::fs::remove_dir_all(&version_dir).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;
        let bytes = download_first_available(&client, &HELLO2CC_TARBALL_URLS).await?;
        extract_repo_tarball(&bytes, &version_dir, &HELLO2CC_ROOT_PREFIXES)?;
    }
    validate_hello2cc_install(&version_dir, "Update")?;

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != version {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    get_hello2cc_status_from_home(&home)
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

    if let Ok(mut stmt) = conn.prepare(
        "SELECT base_path FROM workspaces WHERE base_path IS NOT NULL AND trim(base_path) != ''",
    ) {
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

fn is_openclaw_daily_memory_candidate(path: &std::path::Path, base_dir: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    let extension_allowed = match extension.as_deref() {
        None => true,
        Some("md" | "txt" | "json" | "jsonl" | "yaml" | "yml" | "log") => true,
        _ => false,
    };
    if !extension_allowed {
        return false;
    }

    let relative = match path.strip_prefix(base_dir) {
        Ok(relative) => relative,
        Err(_) => return false,
    };

    let mut components = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(|component| component.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if components.is_empty() {
        return false;
    }

    let file_name = components.pop().unwrap_or_default();
    if file_name.contains("memory") || file_name.contains("journal") || file_name.contains("diary")
    {
        return true;
    }

    components.iter().any(|component| {
        component.contains("memory")
            || component.contains("journal")
            || component.contains("daily")
            || component.contains("diary")
    })
}

fn format_local_datetime(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

fn condense_openclaw_memory_preview(text: &str) -> Option<String> {
    let condensed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if condensed.is_empty() {
        None
    } else {
        Some(condensed.chars().take(220).collect::<String>())
    }
}

fn build_openclaw_memory_preview(content: &str, query: Option<&str>) -> Option<String> {
    let normalized = content.replace('\r', "");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(query) = query {
        let query = query.trim();
        if !query.is_empty() {
            let lowered_content = trimmed.to_lowercase();
            let lowered_query = query.to_lowercase();
            if !lowered_content.contains(&lowered_query) {
                return None;
            }
            if let Some(line_preview) = trimmed
                .lines()
                .find(|line| line.to_lowercase().contains(&lowered_query))
                .and_then(condense_openclaw_memory_preview)
            {
                return Some(line_preview);
            }
        }
    }

    condense_openclaw_memory_preview(trimmed)
}

fn is_valid_openclaw_daily_memory_path(
    path: &std::path::Path,
    conn: &rusqlite::Connection,
) -> bool {
    let canonical_path = match std::fs::canonicalize(path) {
        Ok(path) if path.is_file() => path,
        _ => return false,
    };

    let mut roots = Vec::new();

    if let Some(home) = dirs::home_dir() {
        let global_dir = home.join(".openclaw");
        if let Ok(global_root) = std::fs::canonicalize(&global_dir) {
            roots.push(global_root);
        }
    }

    for project_root in discover_project_roots(conn) {
        let openclaw_root = project_root.join(".openclaw");
        if let Ok(root) = std::fs::canonicalize(&openclaw_root) {
            roots.push(root);
        }
    }

    roots.into_iter().any(|root| {
        canonical_path.starts_with(&root)
            && is_openclaw_daily_memory_candidate(&canonical_path, &root)
    })
}

fn collect_openclaw_daily_memory_files(
    current_dir: &std::path::Path,
    base_dir: &std::path::Path,
    source: &str,
    project_name: Option<&str>,
    query: Option<&str>,
    entries: &mut Vec<OpenClawDailyMemoryEntry>,
    depth: usize,
) {
    if depth > 5 {
        return;
    }

    let read_dir = match std::fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_openclaw_daily_memory_files(
                &path,
                base_dir,
                source,
                project_name,
                query,
                entries,
                depth + 1,
            );
            continue;
        }

        if !is_openclaw_daily_memory_candidate(&path, base_dir) {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let preview = match build_openclaw_memory_preview(&content, query) {
            Some(preview) => preview,
            None => continue,
        };

        let modified_at = std::fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(format_local_datetime);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        entries.push(OpenClawDailyMemoryEntry {
            path: path.to_string_lossy().to_string(),
            file_name,
            source: source.to_string(),
            project_name: project_name.map(str::to_string),
            modified_at,
            preview,
        });
    }
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
        .is_some_and(|(left, right)| {
            left.replace('\\', "/")
                .eq_ignore_ascii_case(&right.replace('\\', "/"))
        })
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
        if !roots
            .iter()
            .any(|value| project_root_paths_match(value, next_path))
        {
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
    let file_name = PathBuf::from(normalized)
        .file_name()?
        .to_string_lossy()
        .to_string();
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
        .filter(|candidate| {
            project_root_match_key(candidate).as_deref() == Some(pending_key.as_str())
        })
        .map(|candidate| {
            (
                candidate,
                shared_trailing_segment_count(pending_path, candidate),
            )
        })
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
    let tools = crate::skills::tools::detect_tools_for_conn(conn);
    let mut reports = Vec::new();

    for tool in tools {
        let cli_command = tool_cli_command(&tool.id).to_string();
        let config_path = resolve_tool_config_path(conn, &tool.id)?
            .to_string_lossy()
            .to_string();
        let mcp_config_path = if tool.id == "claude" {
            resolve_claude_paths(conn)?.0.to_string_lossy().to_string()
        } else {
            resolve_tool_config_path(conn, &tool.id)?
                .to_string_lossy()
                .to_string()
        };
        let skills_dir = resolve_tool_skills_dir(conn, &tool.id)?
            .to_string_lossy()
            .to_string();
        let config_dir = resolve_tool_config_dir(conn, &tool.id)?
            .to_string_lossy()
            .to_string();

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
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
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

    if let Ok(mut stmt) =
        conn.prepare("SELECT tool_id, config_path, config_content FROM _tool_configs")
    {
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
                        crate::utils::atomic_write_string(&settings_json_path, &config_content)
                            .is_ok()
                    }
                    "claude" => {
                        let parsed =
                            serde_json::from_str::<serde_json::Value>(&config_content).ok();
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
                            crate::utils::atomic_write_string(&claude_json_path, &config_content)
                                .is_ok()
                        }
                    }
                    "codex" | "gemini" | "opencode" | "openclaw" | "hermes" => {
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
                if crate::utils::atomic_write_string(&skills_dir.join(&name), &file_content).is_ok()
                {
                    skills_restored += 1;
                }
            }
        }
    }

    if let Ok(mut stmt) = conn
        .prepare("SELECT root_key, relative_path, content_base64 FROM _backup_files ORDER BY id")
    {
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
                    store_imported_project_file(
                        conn,
                        project_root,
                        &relative_path,
                        &content_base64,
                    )?;
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
pub(crate) fn generate_sql_backup(conn: &rusqlite::Connection, home: &std::path::Path) -> String {
    let mut sql = String::new();

    // Header
    sql.push_str("-- ═══════════════════════════════════════════════════════\n");
    sql.push_str("-- CCHub Database Backup (.sql)\n");
    sql.push_str(&format!("-- Version: {}\n", env!("CARGO_PKG_VERSION")));
    sql.push_str(&format!(
        "-- Created: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    sql.push_str("-- ═══════════════════════════════════════════════════════\n\n");

    // Schema (CREATE TABLE IF NOT EXISTS)
    sql.push_str("-- ── Schema ──\n\n");
    sql.push_str(&crate::db::schema::get_schema_sql());
    sql.push_str("\n");

    // Backup metadata table
    sql.push_str("CREATE TABLE IF NOT EXISTS _backup_meta (key TEXT PRIMARY KEY, value TEXT);\n");
    sql.push_str(&format!(
        "INSERT OR REPLACE INTO _backup_meta VALUES ('version', '{}');\n",
        env!("CARGO_PKG_VERSION")
    ));
    sql.push_str(&format!(
        "INSERT OR REPLACE INTO _backup_meta VALUES ('created_at', '{}');\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));

    // Tool configs table
    sql.push_str("CREATE TABLE IF NOT EXISTS _tool_configs (tool_id TEXT PRIMARY KEY, config_path TEXT, config_content TEXT);\n");

    // Skill files table
    sql.push_str("CREATE TABLE IF NOT EXISTS _skill_files (id INTEGER PRIMARY KEY AUTOINCREMENT, tool_id TEXT, name TEXT, content TEXT);\n\n");
    sql.push_str("CREATE TABLE IF NOT EXISTS _backup_files (id INTEGER PRIMARY KEY AUTOINCREMENT, root_key TEXT, relative_path TEXT, content_base64 TEXT);\n\n");

    // Data dump for all 12 business tables
    sql.push_str("-- ── Data ──\n\n");
    let tables = [
        "mcp_servers",
        "plugins",
        "skills",
        "hooks",
        "activity_logs",
        "mcp_clients",
        "workspaces",
        "custom_paths",
        "config_profiles",
        "app_settings",
        "imported_project_files",
        "update_history",
        "metrics",
    ];

    for table in tables {
        let query = format!("SELECT * FROM {}", table);
        if let Ok(mut stmt) = conn.prepare(&query) {
            let col_count = stmt.column_count();
            let col_names: Vec<String> = (0..col_count)
                .map(|i| stmt.column_name(i).unwrap_or("").to_string())
                .collect();

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
                    sql.push_str(&format!(
                        "INSERT OR REPLACE INTO {} ({}) VALUES ({});\n",
                        table,
                        col_names.join(", "),
                        row.join(", ")
                    ));
                }
            }
            if has_rows {
                sql.push('\n');
            }
        }
    }

    // Tool config files
    sql.push_str("-- ── Tool Configs ──\n\n");
    let tool_ids = ["claude", "codex", "gemini", "opencode", "openclaw", "hermes"];
    for tool_id in tool_ids {
        if let Ok(content) = read_tool_snapshot(conn, tool_id) {
            let config_path = match tool_id {
                "claude" => resolve_claude_paths(conn)
                    .map(|(claude_json, settings_json)| {
                        format!("{} | {}", claude_json.display(), settings_json.display())
                    })
                    .unwrap_or_else(|_| {
                        home.join(".claude")
                            .join("settings.json")
                            .display()
                            .to_string()
                    }),
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
                            let name = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
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
            collect_backup_file_rows(
                root_path,
                root_key,
                std::path::Path::new(""),
                &mut backup_file_rows,
            );
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
    let project_relative_dirs = [".claude", ".codex", ".gemini", ".opencode", ".openclaw", ".hermes"];

    for project_root in discover_project_roots(conn) {
        let root_key = format!("project:{}", project_root.to_string_lossy());

        for relative_file in project_relative_files {
            let relative_path = std::path::Path::new(relative_file);
            let absolute_path = project_root.join(relative_path);
            collect_backup_entry_row(
                &absolute_path,
                &root_key,
                relative_path,
                &mut backup_file_rows,
            );
        }

        for relative_dir in project_relative_dirs {
            let relative_path = std::path::Path::new(relative_dir);
            let absolute_path = project_root.join(relative_path);
            if absolute_path.is_dir() {
                collect_backup_file_rows(
                    &absolute_path,
                    &root_key,
                    relative_path,
                    &mut backup_file_rows,
                );
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

fn managed_backups_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(home.join(".cchub").join("backups"))
}

fn ensure_managed_backups_dir() -> Result<PathBuf, String> {
    let dir = managed_backups_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn sanitize_backup_file_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => ch,
        })
        .collect::<String>()
}

fn infer_backup_kind(name: &str) -> String {
    if name.contains("auto") {
        "scheduled".to_string()
    } else {
        "manual".to_string()
    }
}

fn map_backup_entry(path: &std::path::Path) -> Result<ManagedBackupFile, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let modified = metadata
        .modified()
        .unwrap_or_else(|_| std::time::SystemTime::now());
    let modified_at: chrono::DateTime<chrono::Local> = modified.into();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("Invalid backup file name: {}", path.display()))?
        .to_string();

    Ok(ManagedBackupFile {
        path: path.to_string_lossy().to_string(),
        name: name.clone(),
        created_at: modified_at.to_rfc3339(),
        size_bytes: metadata.len(),
        kind: infer_backup_kind(&name),
        can_restore: path.extension().and_then(|value| value.to_str()) == Some("sql"),
    })
}

fn list_managed_backups_from_dir(dir: &std::path::Path) -> Result<Vec<ManagedBackupFile>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("sql") {
            continue;
        }
        items.push(map_backup_entry(&path)?);
    }

    items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(items)
}

fn prune_managed_backups(dir: &std::path::Path, retention_count: usize) -> Result<(), String> {
    let retention_count = retention_count.max(1);
    let backups = list_managed_backups_from_dir(dir)?;
    for backup in backups.into_iter().skip(retention_count) {
        let _ = std::fs::remove_file(&backup.path);
    }
    Ok(())
}

fn create_managed_backup_from_conn(
    conn: &rusqlite::Connection,
    kind: &str,
    retention_count: usize,
) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let backup_dir = ensure_managed_backups_dir()?;
    let prefix = if kind == "scheduled" {
        "cchub-auto-backup"
    } else {
        "cchub-backup"
    };
    let file_path = backup_dir.join(format!(
        "{prefix}-{}.sql",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let sql_content = generate_sql_backup(conn, &home);
    std::fs::write(&file_path, sql_content).map_err(|e| e.to_string())?;
    prune_managed_backups(&backup_dir, retention_count)?;
    Ok(file_path.to_string_lossy().to_string())
}

pub(crate) fn import_backup_from_path_impl(
    db: &State<'_, DbState>,
    file_path: &std::path::Path,
) -> Result<String, String> {
    let raw_content = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
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
            temp_conn
                .execute_batch(content)
                .map_err(|e| e.to_string())?;
            crate::db::schema::run_migrations(&temp_conn).map_err(|e| e.to_string())?;
            validate_imported_backup_tables(&temp_conn)?;
        }

        let placeholder = rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?;
        let old_conn = std::mem::replace(&mut *conn, placeholder);
        drop(conn);

        if let Err((old_conn, err)) = old_conn.close() {
            let mut conn = db.0.lock().map_err(|e| e.to_string())?;
            *conn = old_conn;
            return Err(err.to_string());
        }

        temp_file
    };

    let import_result =
        (|| -> Result<(rusqlite::Connection, usize, usize, usize, usize, usize), String> {
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
            ) = restore_imported_artifacts(&reopened, restored_count)?;
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
        .set_file_name(&format!(
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
