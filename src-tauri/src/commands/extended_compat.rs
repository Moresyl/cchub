#![allow(clippy::too_many_arguments)]

use chrono::Utc;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tauri::State;
use tokio::io::AsyncReadExt;

use crate::commands::extra_commands::{
    get_text_app_setting, launch_preferred_terminal_impl, normalize_terminal_target,
    read_all_config_profiles_from_conn, read_terminal_preferences_from_conn, read_tool_snapshot,
};
use crate::db::DbState;

const MODELS_DEV_SYNC_KEY: &str = "models_dev_sync_config";
const INIT_ERROR_KEY: &str = "init_error";
const MIGRATION_RESULT_KEY: &str = "migration_result";
const SKILLS_MIGRATION_RESULT_KEY: &str = "skills_migration_result";
const MAX_USAGE_SCRIPT_BYTES: usize = 256 * 1024;
const MAX_USAGE_SCRIPT_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_USAGE_SCRIPT_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsDevSyncConfig {
    #[serde(default)]
    pub auto_sync_enabled: bool,
    #[serde(default = "default_true")]
    pub include_common_models: bool,
    #[serde(default)]
    pub selected_model_keys: Vec<String>,
    #[serde(default)]
    pub excluded_common_model_keys: Vec<String>,
    #[serde(default)]
    pub last_sync_at: Option<i64>,
    #[serde(default)]
    pub last_sync_error: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for ModelsDevSyncConfig {
    fn default() -> Self {
        Self {
            auto_sync_enabled: false,
            include_common_models: true,
            selected_model_keys: Vec::new(),
            excluded_common_model_keys: Vec::new(),
            last_sync_at: None,
            last_sync_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsDevSyncState {
    pub config: ModelsDevSyncConfig,
    pub config_path: String,
}

fn normalize_model_keys(values: Vec<String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn normalize_sync_config(mut config: ModelsDevSyncConfig) -> ModelsDevSyncConfig {
    config.selected_model_keys = normalize_model_keys(config.selected_model_keys);
    config.excluded_common_model_keys = normalize_model_keys(config.excluded_common_model_keys);
    config.last_sync_error = config.last_sync_error.and_then(|error| {
        let error = error.trim();
        (!error.is_empty()).then(|| error.chars().take(1000).collect())
    });
    config
}

fn models_dev_sync_path() -> PathBuf {
    crate::commands::model_pricing_file::model_pricing_file_path()
}

#[tauri::command]
pub fn get_models_dev_sync_config(db: State<'_, DbState>) -> Result<ModelsDevSyncState, String> {
    let mut conn = db.0.lock().map_err(|error| error.to_string())?;
    crate::commands::model_pricing_file::sync_local_model_pricing(&mut conn)?;
    let config = get_text_app_setting(&conn, MODELS_DEV_SYNC_KEY)?
        .and_then(|raw| serde_json::from_str::<ModelsDevSyncConfig>(&raw).ok())
        .map(normalize_sync_config)
        .unwrap_or_default();
    Ok(ModelsDevSyncState {
        config,
        config_path: models_dev_sync_path().to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn save_models_dev_sync_config(
    config: ModelsDevSyncConfig,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let config = normalize_sync_config(config);
    let payload = serde_json::to_string(&config).map_err(|error| error.to_string())?;
    let mut conn = db.0.lock().map_err(|error| error.to_string())?;
    crate::commands::model_pricing_file::sync_local_model_pricing(&mut conn)?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![MODELS_DEV_SYNC_KEY, payload],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentConflictCompat {
    pub var_name: String,
    pub var_value: String,
    pub source_type: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentBackupInfo {
    pub backup_path: String,
    pub timestamp: String,
    pub conflicts: Vec<EnvironmentConflictCompat>,
}

fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn env_backup_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".cchub").join("backups"))
        .ok_or_else(|| "Cannot determine home directory".to_string())
}

fn source_file_path(source_path: &str) -> PathBuf {
    let value = source_path.trim();
    if let Some((path, line)) = value.rsplit_once(':') {
        if !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_digit()) {
            return PathBuf::from(path);
        }
    }
    PathBuf::from(value)
}

fn update_env_file(conflict: &EnvironmentConflictCompat, restore: bool) -> Result<(), String> {
    if conflict.source_type == "system" {
        if restore {
            std::env::set_var(&conflict.var_name, &conflict.var_value);
        } else {
            std::env::remove_var(&conflict.var_name);
        }
        return Ok(());
    }
    if conflict.source_type != "file" {
        return Err(format!(
            "Unsupported environment source: {}",
            conflict.source_type
        ));
    }
    let path = source_file_path(&conflict.source_path);
    let content = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "Failed to read environment file {}: {error}",
            path.display()
        )
    })?;
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    if restore {
        if !lines.iter().any(|line| {
            line.trim_start()
                .starts_with(&format!("{}=", conflict.var_name))
        }) {
            lines.push(format!("{}={}", conflict.var_name, conflict.var_value));
        }
    } else {
        lines.retain(|line| {
            let candidate = line.trim().strip_prefix("export ").unwrap_or(line.trim());
            candidate
                .split_once('=')
                .map(|(name, _)| name.trim() != conflict.var_name)
                .unwrap_or(true)
        });
    }
    crate::utils::atomic_write_string(&path, &lines.join("\n")).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_env_vars(
    conflicts: Vec<EnvironmentConflictCompat>,
) -> Result<EnvironmentBackupInfo, String> {
    if conflicts.is_empty() {
        return Err("No environment variables selected".to_string());
    }
    for conflict in &conflicts {
        if !valid_env_name(&conflict.var_name) {
            return Err(format!(
                "Invalid environment variable name: {}",
                conflict.var_name
            ));
        }
    }
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S%.3f").to_string();
    let backup_dir = env_backup_dir()?;
    std::fs::create_dir_all(&backup_dir).map_err(|error| error.to_string())?;
    let path = backup_dir.join(format!("env-backup-{timestamp}.json"));
    let info = EnvironmentBackupInfo {
        backup_path: path.to_string_lossy().into_owned(),
        timestamp,
        conflicts: conflicts.clone(),
    };
    let payload = serde_json::to_string_pretty(&info).map_err(|error| error.to_string())?;
    crate::utils::atomic_write_string(&path, &payload).map_err(|error| error.to_string())?;
    for conflict in &conflicts {
        update_env_file(conflict, false)
            .map_err(|error| format!("{error}; backup preserved at {}", info.backup_path))?;
    }
    Ok(info)
}

#[tauri::command(rename_all = "camelCase")]
pub fn restore_env_backup(backup_path: String) -> Result<(), String> {
    let path = PathBuf::from(backup_path.trim());
    if !path.is_file() {
        return Err(format!("Environment backup not found: {}", path.display()));
    }
    let content = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let info: EnvironmentBackupInfo =
        serde_json::from_str(&content).map_err(|error| error.to_string())?;
    for conflict in &info.conflicts {
        if !valid_env_name(&conflict.var_name) {
            return Err(format!(
                "Invalid environment variable name: {}",
                conflict.var_name
            ));
        }
        update_env_file(conflict, true)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointLatencyCompat {
    pub url: String,
    pub latency: Option<u64>,
    pub status: Option<u16>,
    pub error: Option<String>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn test_api_endpoints(
    urls: Vec<String>,
    timeout_secs: Option<u64>,
) -> Result<Vec<EndpointLatencyCompat>, String> {
    let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(10).clamp(1, 120));
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent("CCHub Endpoint Probe")
        .build()
        .map_err(|error| error.to_string())?;
    let mut normalized = Vec::new();
    for raw in urls.into_iter().take(64) {
        let value = raw.trim().to_string();
        let parsed =
            reqwest::Url::parse(&value).map_err(|error| format!("Invalid URL {value}: {error}"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(format!("Only HTTP(S) URLs are supported: {value}"));
        }
        if !normalized.iter().any(|item| item == &value) {
            normalized.push(value);
        }
    }
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let checks = normalized.into_iter().map(|url| {
        let client = client.clone();
        async move {
            let started = std::time::Instant::now();
            let response = client.head(&url).send().await;
            let response = match response {
                Ok(value) if value.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED => {
                    client.get(&url).send().await
                }
                other => other,
            };
            let latency = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            match response {
                Ok(value) => EndpointLatencyCompat {
                    url,
                    latency: Some(latency),
                    status: Some(value.status().as_u16()),
                    error: None,
                },
                Err(error) => EndpointLatencyCompat {
                    url,
                    latency: Some(latency),
                    status: None,
                    error: Some(error.to_string()),
                },
            }
        }
    });
    Ok(join_all(checks).await)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptUsageResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[allow(non_snake_case)]
#[tauri::command(rename_all = "camelCase")]
pub async fn testUsageScript(
    _provider_id: String,
    _app: String,
    script_code: String,
    timeout: Option<u64>,
    api_key: Option<String>,
    base_url: Option<String>,
    access_token: Option<String>,
    user_id: Option<String>,
    _template_type: Option<String>,
) -> Result<ScriptUsageResult, String> {
    let code = script_code.trim();
    if code.is_empty() || code.len() > MAX_USAGE_SCRIPT_BYTES {
        return Err("Usage script must be between 1 and 262144 bytes".to_string());
    }
    let mut command = tokio::process::Command::new("node");
    command
        .args([
            "--no-warnings",
            "--max-old-space-size=16",
            "--stack-size=256",
            "-e",
            code,
        ])
        .env("CCHUB_API_KEY", api_key.unwrap_or_default())
        .env("CCHUB_BASE_URL", base_url.unwrap_or_default())
        .env("CCHUB_ACCESS_TOKEN", access_token.unwrap_or_default())
        .env("CCHUB_USER_ID", user_id.unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to run usage script: {error}"))?;
    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| "Usage script stdout pipe unavailable".to_string())?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| "Usage script stderr pipe unavailable".to_string())?;
    let timeout_ms = timeout
        .unwrap_or(MAX_USAGE_SCRIPT_TIMEOUT_MS)
        .min(MAX_USAGE_SCRIPT_TIMEOUT_MS)
        .max(100);
    let execution = tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), async {
        let status_future = child.wait();
        let (status, stdout, stderr) = tokio::join!(
            status_future,
            read_limited_output(stdout_pipe, MAX_USAGE_SCRIPT_OUTPUT_BYTES),
            read_limited_output(stderr_pipe, MAX_USAGE_SCRIPT_OUTPUT_BYTES),
        );
        Ok::<_, String>((
            status.map_err(|error| format!("Usage script process failed: {error}"))?,
            stdout?,
            stderr?,
        ))
    })
    .await;
    let (status, stdout_bytes, stderr_bytes) = match execution {
        Ok(result) => result?,
        Err(_) => {
            terminate_usage_process_tree(&mut child).await;
            return Err("Usage script timed out".to_string());
        }
    };
    let stdout = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr_bytes).trim().to_string();
        return Ok(ScriptUsageResult {
            success: false,
            data: None,
            error: Some(if stderr.is_empty() { stdout } else { stderr }),
        });
    }
    let data = serde_json::from_str::<serde_json::Value>(&stdout)
        .map_err(|error| format!("Usage script must print JSON: {error}"))?;
    Ok(ScriptUsageResult {
        success: true,
        data: Some(data),
        error: None,
    })
}

async fn terminate_usage_process_tree(child: &mut tokio::process::Child) {
    #[cfg(target_os = "windows")]
    if let Some(pid) = child.id() {
        let _ = tokio::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn read_limited_output<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| format!("Usage script output could not be read: {error}"))?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > max_bytes {
            return Err(format!("Usage script output exceeds {max_bytes} bytes"));
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn launch_session_terminal(
    command: String,
    cwd: Option<String>,
    custom_config: Option<String>,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let command = command.trim();
    if command.is_empty() || command.len() > 16 * 1024 {
        return Err("Session command is empty or too large".to_string());
    }
    let target = normalize_terminal_target(cwd)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let preferences = read_terminal_preferences_from_conn(&conn)?;
    drop(conn);
    let command = custom_config
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|config| format!("CCHUB_CUSTOM_CONFIG={} {}", shell_quote(config), command))
        .unwrap_or_else(|| command.to_string());
    launch_preferred_terminal_impl(&preferences, &target, Some(&command))
}

#[tauri::command(rename_all = "camelCase")]
pub fn open_provider_terminal(
    app: String,
    provider_id: String,
    cwd: Option<String>,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let app = app.trim().to_ascii_lowercase();
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        return Err("Provider id is required".to_string());
    }
    let target = normalize_terminal_target(cwd)?;
    let (preferences, profile) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let preferences = read_terminal_preferences_from_conn(&conn)?;
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|item| item.id == provider_id && item.tool_id == app)
            .ok_or_else(|| format!("Provider profile not found: {provider_id}"))?;
        (preferences, profile)
    };
    let vars = env_vars_from_snapshot(&profile.config_snapshot);
    let prefix = vars
        .iter()
        .map(|(key, value)| format!("{}={}", key, shell_quote(value)))
        .collect::<Vec<_>>()
        .join(" ");
    let command = if prefix.is_empty() {
        app.clone()
    } else {
        format!("{prefix} {app}")
    };
    launch_preferred_terminal_impl(&preferences, &target, Some(&command))
}

fn shell_quote(value: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn env_vars_from_snapshot(snapshot: &str) -> Vec<(String, String)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(snapshot) else {
        return Vec::new();
    };
    let mut vars = Vec::new();
    if let Some(env) = value.get("env").and_then(serde_json::Value::as_object) {
        vars.extend(env.iter().filter_map(|(key, value)| {
            value.as_str().map(|value| (key.clone(), value.to_string()))
        }));
    }
    for (key, source) in [("OPENAI_API_KEY", "auth"), ("GEMINI_API_KEY", "api_key")] {
        if let Some(value) = value.get(source).and_then(serde_json::Value::as_str) {
            vars.push((key.to_string(), value.to_string()));
        }
    }
    vars
}

#[tauri::command]
pub fn import_hermes_providers_from_live(db: State<'_, DbState>) -> Result<usize, String> {
    import_live_profile("hermes", db)
}

#[tauri::command]
pub fn import_openclaw_providers_from_live(db: State<'_, DbState>) -> Result<usize, String> {
    import_live_profile("openclaw", db)
}

#[tauri::command]
pub fn import_opencode_providers_from_live(db: State<'_, DbState>) -> Result<usize, String> {
    import_live_profile("opencode", db)
}

fn import_live_profile(tool_id: &str, db: State<'_, DbState>) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let snapshot = read_tool_snapshot(&conn, tool_id)?;
    let parsed = serde_json::from_str::<serde_json::Value>(&snapshot).unwrap_or_default();
    let count = parsed
        .get("providers")
        .and_then(serde_json::Value::as_object)
        .map(|value| value.len())
        .unwrap_or(if snapshot.trim().is_empty() { 0 } else { 1 });
    let now = Utc::now().to_rfc3339();
    let profile_id = format!("live-{tool_id}");
    conn.execute(
        "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 0, 'live', ?3, ?5, ?5)
         ON CONFLICT(id) DO UPDATE SET config_snapshot = excluded.config_snapshot, updated_at = excluded.updated_at",
        rusqlite::params![profile_id, format!("{tool_id} current configuration"), tool_id, snapshot, now],
    )
    .map_err(|error| error.to_string())?;
    Ok(count)
}

#[tauri::command]
pub async fn open_hermes_web_ui(
    _app: tauri::AppHandle,
    path: Option<String>,
) -> Result<(), String> {
    let port = std::env::var("HERMES_WEB_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(9119);
    let suffix = path.unwrap_or_default();
    if !suffix.is_empty() && (!suffix.starts_with('/') || suffix.contains("..")) {
        return Err("Invalid Hermes UI path".to_string());
    }
    let target = format!("http://127.0.0.1:{port}{}", suffix);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|error| error.to_string())?;
    let status = client
        .get(format!("http://127.0.0.1:{port}/api/status"))
        .send()
        .await
        .map_err(|error| format!("Hermes UI is not reachable: {error}"))?
        .status();
    if !(status.is_success() || status == reqwest::StatusCode::UNAUTHORIZED) {
        return Err(format!("Hermes UI returned HTTP {status}"));
    }
    crate::commands::extra_commands::open_in_system(target)
}

#[tauri::command]
pub fn launch_hermes_dashboard(db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let preferences = read_terminal_preferences_from_conn(&conn)?;
    let target = normalize_terminal_target(None)?;
    launch_preferred_terminal_impl(&preferences, &target, Some("hermes dashboard"))?;
    Ok(())
}

#[tauri::command]
pub fn extract_common_config_snippet(
    app_type: String,
    settings_config: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let app = app_type.trim().to_ascii_lowercase();
    if !matches!(app.as_str(), "claude" | "codex" | "gemini") {
        return Err(format!("Unsupported app for common config: {app}"));
    }
    let raw = match settings_config.filter(|value| !value.trim().is_empty()) {
        Some(value) => value,
        None => {
            let conn = db.0.lock().map_err(|error| error.to_string())?;
            read_tool_snapshot(&conn, &app)?
        }
    };
    if app == "codex" {
        let mut document = raw
            .parse::<toml_edit::DocumentMut>()
            .map_err(|error| format!("Invalid TOML: {error}"))?;
        for key in [
            "model",
            "model_provider",
            "model_providers",
            "api_key",
            "base_url",
        ] {
            document.as_table_mut().remove(key);
        }
        return Ok(document.to_string());
    }
    let mut value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("Invalid JSON: {error}"))?;
    if let Some(object) = value.as_object_mut() {
        for key in [
            "api_key", "apiKey", "auth", "base_url", "baseUrl", "model", "models",
        ] {
            object.remove(key);
        }
        if let Some(env) = object
            .get_mut("env")
            .and_then(serde_json::Value::as_object_mut)
        {
            env.retain(|key, _| {
                let upper = key.to_ascii_uppercase();
                !upper.contains("KEY")
                    && !upper.contains("TOKEN")
                    && !upper.contains("BASE_URL")
                    && !upper.ends_with("_MODEL")
            });
        }
    }
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn update_toml_common_config_snippet(
    config_toml: String,
    snippet_toml: String,
    enabled: bool,
) -> Result<String, String> {
    let mut document = config_toml
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("Invalid config TOML: {error}"))?;
    let snippet = snippet_toml
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("Invalid snippet TOML: {error}"))?;
    for (key, item) in snippet.iter() {
        if enabled {
            document[key] = item.clone();
        } else if document
            .get(key)
            .is_some_and(|existing| existing.to_string() == item.to_string())
        {
            document.as_table_mut().remove(key);
        }
    }
    Ok(document.to_string())
}

#[tauri::command]
pub fn get_init_error() -> Result<Option<serde_json::Value>, String> {
    Ok(std::env::var(INIT_ERROR_KEY)
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok()))
}

#[tauri::command]
pub fn get_migration_result() -> Result<bool, String> {
    Ok(std::env::var(MIGRATION_RESULT_KEY)
        .ok()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")))
}

#[tauri::command]
pub fn get_skills_migration_result() -> Result<Option<serde_json::Value>, String> {
    Ok(std::env::var(SKILLS_MIGRATION_RESULT_KEY)
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok()))
}

#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) -> Result<bool, String> {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
        app.exit(0);
    });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{normalize_model_keys, source_file_path, valid_env_name};

    #[test]
    fn normalizes_sync_model_keys() {
        assert_eq!(
            normalize_model_keys(vec![" b ".into(), "a".into(), "b".into()]),
            vec!["a", "b"]
        );
    }

    #[test]
    fn validates_environment_names() {
        assert!(valid_env_name("OPENAI_API_KEY"));
        assert!(!valid_env_name("A=B"));
        assert!(!valid_env_name(""));
    }

    #[test]
    fn strips_unix_line_suffix_from_environment_source() {
        assert_eq!(
            source_file_path("/tmp/.profile:4"),
            std::path::PathBuf::from("/tmp/.profile")
        );
    }
}
