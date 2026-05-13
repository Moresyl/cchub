#![allow(clippy::too_many_arguments)]
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use crate::db::DbState;
use crate::shared::{github_release, github_urls, http_client};
use crate::utils::configure_background_command;

use super::super::config_profiles::*;
use super::super::log_command_timing;
use super::super::proxy_settings::*;
use super::super::types::*;
use super::*;

pub fn hello2cc_manifest_urls() -> Vec<String> {
    github_urls::raw_file_urls(
        "hellowind777",
        "hello2cc",
        "main",
        ".claude-plugin/plugin.json",
    )
}

pub fn hello2cc_tarball_urls() -> Vec<String> {
    github_urls::archive_branch_tarball_urls("hellowind777", "hello2cc", "main")
}

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

pub fn claude_settings_path(home: &std::path::Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

pub fn hello2cc_cache_dir(home: &std::path::Path) -> PathBuf {
    home.join(".claude")
        .join("plugins")
        .join("cache")
        .join("hello2cc")
        .join("hello2cc")
}

pub fn hello2cc_required_paths(version_dir: &std::path::Path) -> [PathBuf; 4] {
    [
        version_dir.join(".claude-plugin").join("plugin.json"),
        version_dir.join(".claude-plugin").join("marketplace.json"),
        version_dir.join("agents").join("native.md"),
        version_dir.join("output-styles").join("hello2cc-native.md"),
    ]
}

pub fn validate_hello2cc_install(
    version_dir: &std::path::Path,
    action: &str,
) -> Result<(), String> {
    for required_path in hello2cc_required_paths(version_dir) {
        if !required_path.exists() {
            return Err(format!(
                "{} failed: missing {}",
                action,
                required_path.display()
            ));
        }
    }

    Ok(())
}

pub fn ensure_json_object(
    value: &mut serde_json::Value,
) -> &mut serde_json::Map<String, serde_json::Value> {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    match value {
        serde_json::Value::Object(map) => map,
        _ => unreachable!("value was normalized to an object"),
    }
}

pub fn ensure_child_object<'a>(
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
    match entry {
        serde_json::Value::Object(map) => map,
        _ => unreachable!("value was normalized to an object"),
    }
}

pub fn read_json_value_or_default(path: &std::path::Path) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }

    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }

    serde_json::from_str(&content).map_err(|e| e.to_string())
}

pub fn write_json_value(path: &std::path::Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(path, &content).map_err(|e| e.to_string())
}

pub fn normalize_hello2cc_mode(value: &str) -> String {
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

pub fn normalize_hello2cc_routing_policy(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("prompt-only") {
        "prompt-only".to_string()
    } else {
        "native-inject".to_string()
    }
}

pub fn sanitize_hello2cc_config(config: Hello2ccConfig) -> Hello2ccConfig {
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

pub fn read_hello2cc_config_from_settings(settings: &serde_json::Value) -> Hello2ccConfig {
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

pub fn write_hello2cc_config_into_settings(
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

pub fn parse_version_components(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.parse::<u64>().unwrap_or(0))
        .collect()
}

pub fn compare_version_like(left: &str, right: &str) -> Ordering {
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

pub fn find_latest_installed_plugin_version(
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

pub fn build_plugin_http_client(proxy_url: &str) -> Result<reqwest::Client, String> {
    http_client::build_http_client(Some(proxy_url), Some("CCHub"), Duration::from_secs(30))
}

pub async fn fetch_plugin_version_from_manifest(
    client: &reqwest::Client,
    urls: &[String],
) -> Result<String, String> {
    for url in urls {
        let response = match client.get(url).send().await {
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

pub async fn download_first_available(
    client: &reqwest::Client,
    urls: &[String],
) -> Result<bytes::Bytes, String> {
    let mut last_err = String::new();
    for url in urls {
        match client.get(url).send().await {
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

pub fn extract_repo_tarball(
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

pub fn get_hello2cc_status_from_home(home: &std::path::Path) -> Result<Hello2ccStatus, String> {
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
    let version = fetch_plugin_version_from_manifest(&client, &hello2cc_manifest_urls()).await?;
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

    let bytes = download_first_available(&client, &hello2cc_tarball_urls()).await?;
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
    let latest_version =
        fetch_plugin_version_from_manifest(&client, &hello2cc_manifest_urls()).await?;

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
    let version = fetch_plugin_version_from_manifest(&client, &hello2cc_manifest_urls()).await?;
    let cache_dir = hello2cc_cache_dir(&home);
    let version_dir = cache_dir.join(&version);
    let manifest_path = version_dir.join(".claude-plugin").join("plugin.json");

    if !manifest_path.exists() {
        if version_dir.exists() {
            std::fs::remove_dir_all(&version_dir).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&version_dir).map_err(|e| e.to_string())?;
        let bytes = download_first_available(&client, &hello2cc_tarball_urls()).await?;
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

pub const SQL_BACKUP_MARKER: &str = "-- CCHub Database Backup (.sql)";
