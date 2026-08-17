use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

use crate::utils::configure_background_command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthCheckResult {
    pub server_id: String,
    pub server_name: String,
    pub status: String,
    pub command_exists: bool,
    pub can_start: bool,
    pub error_message: Option<String>,
    pub latency_ms: Option<u64>,
    pub checked_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeDepStatus {
    pub name: String,
    pub display_name: String,
    pub installed: bool,
    pub version: Option<String>,
}

pub fn check_command_exists(command: &str) -> bool {
    if command.is_empty() {
        return false;
    }

    // For npx, node, python etc, check with where.exe on Windows
    let check_cmd = if cfg!(windows) { "where.exe" } else { "which" };

    // Extract the actual command (first part, ignore arguments)
    let cmd = command.split_whitespace().next().unwrap_or(command);

    let mut process = std::process::Command::new(check_cmd);
    configure_background_command(&mut process);

    process
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn get_command_version(command: &str, version_flag: &str) -> Option<String> {
    let mut process = std::process::Command::new(command);
    configure_background_command(&mut process);

    let output = process
        .arg(version_flag)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            None
        } else {
            Some(stderr.lines().next().unwrap_or("").to_string())
        }
    } else {
        Some(stdout.lines().next().unwrap_or("").to_string())
    }
}

/// Check common runtime dependencies for MCP servers
pub fn check_runtime_deps() -> Vec<RuntimeDepStatus> {
    let runtimes = vec![
        ("node", "Node.js", "--version"),
        ("npx", "npx", "--version"),
        ("npm", "npm", "--version"),
        ("uvx", "uvx", "--version"),
        ("uv", "uv", "--version"),
        ("python", "Python", "--version"),
        ("python3", "Python3", "--version"),
        ("pip", "pip", "--version"),
        ("pip3", "pip3", "--version"),
        ("docker", "Docker", "--version"),
        ("bun", "Bun", "--version"),
        ("deno", "Deno", "--version"),
    ];

    runtimes
        .into_iter()
        .map(|(name, display_name, flag)| {
            let installed = check_command_exists(name);
            let version = if installed {
                get_command_version(name, flag)
            } else {
                None
            };
            RuntimeDepStatus {
                name: name.to_string(),
                display_name: display_name.to_string(),
                installed,
                version,
            }
        })
        .collect()
}

pub fn try_spawn_server(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> (bool, Option<u64>, Option<String>) {
    let start = Instant::now();

    let mut cmd = std::process::Command::new(command);
    configure_background_command(&mut cmd);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    match cmd.spawn() {
        Ok(mut child) => {
            let elapsed = start.elapsed().as_millis() as u64;
            // Wait briefly to see if it crashes immediately
            match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success() {
                        (true, Some(elapsed), None)
                    } else {
                        (
                            false,
                            Some(elapsed),
                            Some(format!("Process exited with code {}", status)),
                        )
                    }
                }
                Ok(None) => {
                    // Process is still running — that's good, kill it
                    let _ = child.kill();
                    let _ = child.wait();
                    (true, Some(elapsed), None)
                }
                Err(e) => {
                    let _ = child.kill();
                    (
                        false,
                        Some(elapsed),
                        Some(format!("Error checking process: {}", e)),
                    )
                }
            }
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            (
                false,
                Some(elapsed),
                Some(format!("Failed to spawn: {}", e)),
            )
        }
    }
}

pub fn check_server_health(
    server_id: &str,
    server_name: &str,
    command: &str,
    args_json: &str,
    env_json: &str,
) -> HealthCheckResult {
    let now = chrono::Utc::now().to_rfc3339();

    if command.is_empty() {
        return HealthCheckResult {
            server_id: server_id.to_string(),
            server_name: server_name.to_string(),
            status: "unknown".to_string(),
            command_exists: false,
            can_start: false,
            error_message: Some("No command configured".to_string()),
            latency_ms: None,
            checked_at: now,
        };
    }

    if command.starts_with("https://") || command.starts_with("http://") {
        let valid_url = url::Url::parse(command).is_ok();
        return HealthCheckResult {
            server_id: server_id.to_string(),
            server_name: server_name.to_string(),
            status: if valid_url { "healthy" } else { "unhealthy" }.to_string(),
            command_exists: valid_url,
            can_start: valid_url,
            error_message: (!valid_url).then(|| "Invalid remote MCP URL".to_string()),
            latency_ms: None,
            checked_at: now,
        };
    }

    let cmd_exists = check_command_exists(command);

    if !cmd_exists {
        return HealthCheckResult {
            server_id: server_id.to_string(),
            server_name: server_name.to_string(),
            status: "unhealthy".to_string(),
            command_exists: false,
            can_start: false,
            error_message: Some(format!("Command '{}' not found", command)),
            latency_ms: None,
            checked_at: now,
        };
    }

    let args: Vec<String> = serde_json::from_str(args_json).unwrap_or_default();
    let env: HashMap<String, String> = serde_json::from_str(env_json).unwrap_or_default();

    let (can_start, latency, error) = try_spawn_server(command, &args, &env);

    let status = if can_start { "healthy" } else { "unhealthy" };

    HealthCheckResult {
        server_id: server_id.to_string(),
        server_name: server_name.to_string(),
        status: status.to_string(),
        command_exists: true,
        can_start,
        error_message: error,
        latency_ms: latency,
        checked_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::check_server_health;

    #[test]
    fn remote_mcp_url_is_not_treated_as_a_local_executable() {
        let result = check_server_health("remote", "Remote", "https://example.com/mcp", "[]", "{}");

        assert_eq!(result.status, "healthy");
        assert!(result.command_exists);
        assert!(result.can_start);
        assert!(result.error_message.is_none());
    }
}
