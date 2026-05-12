use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
