#![allow(clippy::too_many_arguments)]
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use crate::copilot_auth::{self, CopilotAuthState};
use crate::db::DbState;
use crate::hermes;
use crate::shared::http_client;
use crate::utils::configure_background_command;

use super::super::log_command_timing;
use super::super::proxy_settings::*;
use super::super::statusline::*;
use super::super::types::*;
use super::*;

pub const PREFERRED_TERMINAL_SETTING_KEY: &str = "preferred_terminal";
pub const BACKUP_PREFERENCES_SETTING_KEY: &str = "backup_preferences";
pub const LOG_PREFERENCES_SETTING_KEY: &str = "log_preferences";
pub const PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY: &str = "provider_config_fragments";

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

pub fn default_visible_apps() -> Vec<String> {
    MANAGED_APP_IDS.iter().map(|id| (*id).to_string()).collect()
}

pub fn normalize_visible_apps(visible_apps: Vec<String>) -> Vec<String> {
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

pub fn read_backup_preferences_from_conn(conn: &rusqlite::Connection) -> BackupPreferences {
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

pub fn normalize_log_level(level: &str) -> String {
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

pub fn build_log_file_targets() -> LogFileTargets {
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

pub fn updater_environment_state() -> UpdaterEnvironmentState {
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

pub fn log_provider_result(
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
        vec![
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
        ]
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

pub fn read_terminal_preferences_from_conn(
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

pub fn normalize_terminal_target(path: Option<String>) -> Result<PathBuf, String> {
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

pub fn launch_preferred_terminal_impl(
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

pub fn build_session_resume_command(
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
pub fn autostart_entry_path() -> Result<PathBuf, String> {
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
pub fn autostart_entry_content(exe: &std::path::Path) -> String {
    format!("@echo off\r\nstart \"\" \"{}\"\r\n", exe.display())
}

#[cfg(target_os = "macos")]
pub fn autostart_entry_path() -> Result<PathBuf, String> {
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
pub fn autostart_entry_content(exe: &std::path::Path) -> String {
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
pub fn autostart_entry_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir().ok_or("Cannot find config directory")?;
    Ok(config_dir.join("autostart").join("com.cchub.app.desktop"))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn autostart_entry_content(exe: &std::path::Path) -> String {
    let escaped = exe.to_string_lossy().replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=CCHub\nExec=\"{escaped}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}
