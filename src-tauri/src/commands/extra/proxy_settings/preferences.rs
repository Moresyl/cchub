#![allow(clippy::too_many_arguments)]
use serde::Serialize;
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::time::{Duration, Instant};
use tauri::State;

use crate::db::DbState;
use crate::hermes;

use super::super::config_profiles::*;
use super::super::statusline::*;
use super::super::types::*;

// ── Proxy Settings ──

/// Set HTTP/HTTPS proxy for all network requests (persisted to database)
#[tauri::command]
pub fn set_proxy(proxy_url: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    if proxy_url.trim().is_empty() {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('proxy_url', '')",
            [],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let url = proxy_url.trim().to_string();
        validate_proxy_url(&url)?;
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('proxy_url', ?1)",
            rusqlite::params![url],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn validate_proxy_url(proxy_url: &str) -> Result<(), String> {
    reqwest::Proxy::all(proxy_url)
        .map(|_| ())
        .map_err(|error| format!("Invalid proxy URL: {error}"))
}

/// Get current proxy setting
#[tauri::command]
pub fn get_proxy(db: State<'_, DbState>) -> String {
    // Read from database. Network calls inject this per reqwest::Client.
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
    String::new()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub success: bool,
    pub latency_ms: u64,
    pub status: Option<u16>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn test_proxy_url(proxy_url: String) -> Result<ProxyTestResult, String> {
    let trimmed = proxy_url.trim();
    if trimmed.is_empty() {
        return Err("Proxy URL is empty".to_string());
    }
    let proxy =
        reqwest::Proxy::all(trimmed).map_err(|error| format!("Invalid proxy URL: {error}"))?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| format!("Failed to build proxy test client: {error}"))?;
    let started = Instant::now();
    let mut last_error = None;
    for target in [
        "https://httpbin.org/get",
        "https://www.google.com",
        "https://api.anthropic.com",
    ] {
        match client.head(target).send().await {
            Ok(response) => {
                return Ok(ProxyTestResult {
                    success: true,
                    latency_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                    status: Some(response.status().as_u16()),
                    error: None,
                });
            }
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    Ok(ProxyTestResult {
        success: false,
        latency_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        status: None,
        error: last_error.or_else(|| Some("All proxy test targets failed".to_string())),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedProxy {
    pub url: String,
    pub proxy_type: String,
    pub port: u16,
}

#[tauri::command]
pub async fn scan_local_proxies() -> Vec<DetectedProxy> {
    tokio::task::spawn_blocking(|| {
        let candidates: &[(u16, &str, bool)] = &[
            (7890, "http", true),
            (7891, "socks5", false),
            (1080, "socks5", false),
            (8080, "http", false),
            (8888, "http", false),
            (3128, "http", false),
            (10808, "socks5", false),
            (10809, "http", false),
        ];
        candidates
            .iter()
            .filter(|(port, _, _)| {
                TcpStream::connect_timeout(
                    &SocketAddrV4::new(Ipv4Addr::LOCALHOST, *port).into(),
                    Duration::from_millis(100),
                )
                .is_ok()
            })
            .flat_map(|(port, proxy_type, mixed)| {
                let mut found = vec![DetectedProxy {
                    url: format!("{proxy_type}://127.0.0.1:{port}"),
                    proxy_type: (*proxy_type).to_string(),
                    port: *port,
                }];
                if *mixed {
                    found.push(DetectedProxy {
                        url: format!("socks5://127.0.0.1:{port}"),
                        proxy_type: "socks5".to_string(),
                        port: *port,
                    });
                }
                found
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::validate_proxy_url;

    #[test]
    fn validates_http_proxy_urls() {
        assert!(validate_proxy_url("http://127.0.0.1:7890").is_ok());
    }

    #[test]
    fn rejects_invalid_proxy_urls() {
        assert!(validate_proxy_url("not a proxy").is_err());
    }
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
pub fn set_welcome_completed(completed: bool, db: State<'_, DbState>) -> Result<bool, String> {
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
