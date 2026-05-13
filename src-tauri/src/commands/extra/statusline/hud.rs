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

use super::super::types::*;

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
pub fn write_hud_env_entries(version_dir: &std::path::Path) -> Result<(), String> {
    // env-entry.ts — for bun (native TypeScript support)
    let ts_content = r#"// Entry point that reads stdin from env var instead of piped stdin
// This avoids the Windows pipe timing issue with Claude Code's statusline renderer
import { main } from './src/index.ts';

pub const stdinRaw = process.env.CLAUDE_HUD_STDIN || '';
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
pub const { main } = await import('./dist/index.js');

pub const stdinRaw = process.env.CLAUDE_HUD_STDIN || '';
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
pub fn build_hud_statusline_command(home: &std::path::Path) -> Result<String, String> {
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
pub fn find_bun_path(home: &std::path::Path) -> Option<String> {
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
pub async fn fetch_claude_hud_latest_release(
    client: &reqwest::Client,
) -> Result<(String, String), String> {
    let release = github_release::fetch_latest_release(client, "jarrodwatts", "claude-hud").await?;
    let version = release.tag_name.trim_start_matches('v').to_string();
    Ok((version, release.tag_name))
}

pub fn build_cchub_http_client(proxy_url: &str) -> Result<reqwest::Client, String> {
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

pub const HELLO2CC_PLUGIN_ID: &str = "hello2cc@hello2cc";
pub const HELLO2CC_ROOT_PREFIXES: [&str; 2] = ["hello2cc-main/", "hello2cc-master/"];
