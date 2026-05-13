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

use super::config_profiles::*;
use super::log_command_timing;
use super::proxy_settings::*;
use super::types::*;

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
} catch {
  stdinData = null;
}

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
} catch {
  stdinData = null;
}

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
        "bash -c 'export CLAUDE_HUD_STDIN=$(cat); \
plugin_dir=$(ls -d \"${{CLAUDE_CONFIG_DIR:-$HOME/.claude}}\"/plugins/cache/claude-hud/claude-hud/*/ 2>/dev/null \
| awk -F/ '\"'\"'{{ print $(NF-1) \"\\t\" $(0) }}'\"'\"' \
| sort -t. -k1,1n -k2,2n -k3,3n -k4,4n | tail -1 | cut -f2-); \
node \"${{plugin_dir}}env-entry.mjs\" 2>/dev/null | tr -d \"\\r\"'".to_string()
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

/// Fetch latest claude-hud release info from GitHub Releases API.
/// Returns (version_without_v_prefix, tag_name).
/// Source of truth for version is GitHub Releases, NOT plugin.json (main branch
/// can be ahead of latest published release).
async fn fetch_claude_hud_latest_release(
    client: &reqwest::Client,
) -> Result<(String, String), String> {
    let release = github_release::fetch_latest_release(client, "jarrodwatts", "claude-hud").await?;
    let version = release.tag_name.trim_start_matches('v').to_string();
    Ok((version, release.tag_name))
}

fn build_cchub_http_client(proxy_url: &str) -> Result<reqwest::Client, String> {
    http_client::build_http_client(Some(proxy_url), Some("CCHub"), Duration::from_secs(30))
}

/// Install claude-hud plugin from GitHub repository
#[tauri::command]
pub async fn install_claude_hud(db: State<'_, crate::db::DbState>) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    // Build HTTP client with proxy support
    let proxy_url = get_proxy(db);
    let client = build_cchub_http_client(&proxy_url)?;

    // Fetch latest published version from GitHub Releases (not plugin.json)
    let (version, tag_name) = fetch_claude_hud_latest_release(&client)
        .await
        .unwrap_or(("0.0.12".to_string(), "v0.0.12".to_string())); // fallback

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

    // Download tarball for this specific tag from GitHub
    let tarball_urls =
        github_urls::archive_tag_tarball_urls("jarrodwatts", "claude-hud", &tag_name);

    let mut bytes = None;
    let mut last_err = String::new();
    for url in &tarball_urls {
        match client.get(url).send().await {
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

    // Extract tarball: GitHub tag tarball format is claude-hud-{version}/{dist,src}/*
    let gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| format!("Tar read failed: {}", e))?;

    // Extract both dist/ and src/. Try version-named, branch-named (legacy), and
    // any single-prefix folder (fallback) since GitHub uses different layouts.
    let prefix_candidates = [
        format!("claude-hud-{}/dist/", version),
        format!("claude-hud-{}/src/", version),
        "claude-hud-main/dist/".to_string(),
        "claude-hud-master/dist/".to_string(),
        "claude-hud-main/src/".to_string(),
        "claude-hud-master/src/".to_string(),
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
    let client = build_cchub_http_client(&proxy_url)?;

    let (latest_version, _) = fetch_claude_hud_latest_release(&client).await?;

    let normalize = |v: &str| v.trim_start_matches('v').to_string();
    let has_update = normalize(&latest_version) != normalize(&current_version);

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
    let client = build_cchub_http_client(&proxy_url)?;

    // Get latest published version from GitHub Releases API
    let (version, tag_name) = fetch_claude_hud_latest_release(&client).await?;

    let dist_dir = cache_dir.join(&version).join("dist");

    // Skip if already installed
    if dist_dir.join("index.js").exists() {
        return Ok(serde_json::json!({ "version": version, "skipped": true }));
    }

    std::fs::create_dir_all(&dist_dir).map_err(|e| e.to_string())?;

    // Download tarball for this specific tag from GitHub
    let tarball_urls =
        github_urls::archive_tag_tarball_urls("jarrodwatts", "claude-hud", &tag_name);

    let mut bytes = None;
    let mut last_err = String::new();
    for url in &tarball_urls {
        match client.get(url).send().await {
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

    // Extract both dist/ and src/. GitHub tag tarball uses claude-hud-{version}/.
    let prefix_candidates = [
        format!("claude-hud-{}/dist/", version),
        format!("claude-hud-{}/src/", version),
        "claude-hud-main/dist/".to_string(),
        "claude-hud-master/dist/".to_string(),
        "claude-hud-main/src/".to_string(),
        "claude-hud-master/src/".to_string(),
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
const HELLO2CC_ROOT_PREFIXES: [&str; 2] = ["hello2cc-main/", "hello2cc-master/"];

fn hello2cc_manifest_urls() -> Vec<String> {
    github_urls::raw_file_urls(
        "hellowind777",
        "hello2cc",
        "main",
        ".claude-plugin/plugin.json",
    )
}

fn hello2cc_tarball_urls() -> Vec<String> {
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
        version_dir.join("output-styles").join("hello2cc-native.md"),
    ]
}

fn validate_hello2cc_install(version_dir: &std::path::Path, action: &str) -> Result<(), String> {
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

pub(super) fn ensure_json_object(
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
    match entry {
        serde_json::Value::Object(map) => map,
        _ => unreachable!("value was normalized to an object"),
    }
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
    http_client::build_http_client(Some(proxy_url), Some("CCHub"), Duration::from_secs(30))
}

async fn fetch_plugin_version_from_manifest(
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

async fn download_first_available(
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

pub(super) fn discover_project_roots(conn: &rusqlite::Connection) -> Vec<PathBuf> {
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
    let extension_allowed = matches!(
        extension.as_deref(),
        None | Some("md" | "txt" | "json" | "jsonl" | "yaml" | "yml" | "log")
    );
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

pub(super) fn format_local_datetime(time: std::time::SystemTime) -> String {
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

pub(super) fn is_valid_openclaw_daily_memory_path(
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

pub(super) fn collect_openclaw_daily_memory_files(
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

pub(super) fn normalize_project_root_path(path: &str) -> Option<&str> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.trim_end_matches(['\\', '/']))
    }
}

pub(super) fn project_root_paths_match(left: &str, right: &str) -> bool {
    normalize_project_root_path(left)
        .zip(normalize_project_root_path(right))
        .is_some_and(|(left, right)| {
            left.replace('\\', "/")
                .eq_ignore_ascii_case(&right.replace('\\', "/"))
        })
}

pub(super) fn sync_known_project_root(
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

pub(super) fn apply_project_root_remap(
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
    sql.push('\n');

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
    let tool_ids = [
        "claude", "codex", "gemini", "opencode", "openclaw", "hermes",
    ];
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
    let project_relative_dirs = [
        ".claude",
        ".codex",
        ".gemini",
        ".opencode",
        ".openclaw",
        ".hermes",
    ];

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
        .set_file_name(format!(
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
