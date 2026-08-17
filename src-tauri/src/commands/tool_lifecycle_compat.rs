use serde::Serialize;

fn validate_tool_name(name: &str) -> bool {
    matches!(
        name,
        "claude" | "codex" | "gemini" | "opencode" | "openclaw" | "hermes" | "pi"
    )
}

fn npm_package_for(tool: &str) -> Option<&'static str> {
    match tool {
        "claude" => Some("@anthropic-ai/claude-code"),
        "codex" => Some("@openai/codex"),
        "gemini" => Some("@google/gemini-cli"),
        "opencode" => Some("opencode-ai"),
        "openclaw" => Some("openclaw"),
        "pi" => Some("@earendil-works/pi-coding-agent"),
        _ => None,
    }
}

const HERMES_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/NousResearch/hermes-agent/main/scripts/install.sh";

fn hermes_lifecycle_command(action: &str) -> (&'static str, Vec<String>) {
    #[cfg(windows)]
    {
        let script = if action == "install" {
            format!(
                "$ErrorActionPreference='Stop'; irm {} | iex",
                HERMES_INSTALL_URL
            )
        } else {
            format!(
                "$ErrorActionPreference='Stop'; hermes update; if ($LASTEXITCODE -ne 0) {{ irm {} | iex }}",
                HERMES_INSTALL_URL
            )
        };
        (
            "powershell.exe",
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-Command".to_string(),
                script,
            ],
        )
    }
    #[cfg(not(windows))]
    {
        let installer = format!(
            "tmp=$(mktemp) && curl -fsSL {HERMES_INSTALL_URL} -o $tmp && bash $tmp; status=$?; rm -f $tmp; exit $status"
        );
        let script = if action == "install" {
            format!("bash -c '{installer}'")
        } else {
            format!("hermes update || bash -c '{installer}'")
        };
        ("sh", vec!["-c".to_string(), script])
    }
}

fn run_hermes_lifecycle(action: &str) -> std::io::Result<std::process::Output> {
    let (program, args) = hermes_lifecycle_command(action);
    std::process::Command::new(program).args(args).output()
}

fn package_manager() -> Option<&'static str> {
    for candidate in if cfg!(windows) {
        ["pnpm.cmd", "npm.cmd"]
    } else {
        ["pnpm", "npm"]
    } {
        if std::process::Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return Some(candidate);
        }
    }
    None
}

fn run_lifecycle_action(
    tools: &[String],
    action: &str,
    _wsl_shell_by_tool: Option<&serde_json::Value>,
) -> Result<(), String> {
    let manager = tools
        .iter()
        .any(|tool| npm_package_for(tool).is_some())
        .then(package_manager)
        .flatten();
    if tools.iter().any(|tool| npm_package_for(tool).is_some()) && manager.is_none() {
        return Err(
            "No supported package manager found. Install pnpm or npm and try again.".to_string(),
        );
    }
    let mut failures = Vec::new();
    for tool in tools {
        if tool == "hermes" {
            match run_hermes_lifecycle(action) {
                Ok(output) if output.status.success() => {}
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let detail = if stderr.is_empty() { stdout } else { stderr };
                    failures.push(format!(
                        "{tool}: {}",
                        detail.chars().take(500).collect::<String>()
                    ));
                }
                Err(error) => failures.push(format!("{tool}: {error}")),
            }
            continue;
        }
        let Some(package) = npm_package_for(tool) else {
            failures.push(format!("{tool}: no safe package mapping is configured"));
            continue;
        };
        let Some(manager) = manager else {
            failures.push(format!("{tool}: no supported package manager found"));
            continue;
        };
        let package_spec = format!("{package}@latest");
        let args = if manager.starts_with("pnpm") {
            if action == "install" {
                vec!["add", "--global", package_spec.as_str()]
            } else {
                vec!["update", "--global", package_spec.as_str()]
            }
        } else if action == "install" {
            vec!["install", "--global", package_spec.as_str()]
        } else {
            vec!["update", "--global", package_spec.as_str()]
        };
        let output = std::process::Command::new(manager).args(args).output();
        match output {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let detail = if stderr.is_empty() { stdout } else { stderr };
                failures.push(format!(
                    "{tool}: {}",
                    detail.chars().take(500).collect::<String>()
                ));
            }
            Err(error) => failures.push(format!("{tool}: {error}")),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolInstallationCompat {
    pub path: String,
    pub version: Option<String>,
    pub runnable: bool,
    pub error: Option<String>,
    pub source: String,
    pub is_path_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolInstallationReportCompat {
    pub tool: String,
    pub installs: Vec<ToolInstallationCompat>,
    pub is_conflict: bool,
    pub needs_confirmation: bool,
    pub command: String,
    pub anchored: bool,
}

#[tauri::command]
pub async fn probe_tool_installations(
    tools: Vec<String>,
) -> Result<Vec<ToolInstallationReportCompat>, String> {
    let mut normalized = tools
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| validate_tool_name(value))
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    if normalized.is_empty() {
        return Err("No supported tools selected".to_string());
    }
    tokio::task::spawn_blocking(move || {
        normalized
            .into_iter()
            .map(|tool| probe_tool_installation(&tool))
            .collect()
    })
    .await
    .map_err(|error| error.to_string())
}

fn probe_tool_installation(tool: &str) -> ToolInstallationReportCompat {
    let command_name = if tool == "pi" { "pi" } else { tool };
    let lookup = if cfg!(windows) { "where" } else { "which" };
    let output = std::process::Command::new(lookup)
        .arg(command_name)
        .output();
    let paths = output
        .ok()
        .filter(|value| value.status.success())
        .map(|value| {
            String::from_utf8_lossy(&value.stdout)
                .lines()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let installs = paths
        .iter()
        .map(|path| {
            let version = std::process::Command::new(path)
                .arg("--version")
                .output()
                .ok()
                .map(|value| String::from_utf8_lossy(&value.stdout).trim().to_string())
                .filter(|value| !value.is_empty());
            ToolInstallationCompat {
                path: path.clone(),
                version,
                runnable: true,
                error: None,
                source: "PATH".to_string(),
                is_path_default: true,
            }
        })
        .collect::<Vec<_>>();
    ToolInstallationReportCompat {
        tool: tool.to_string(),
        is_conflict: installs.len() > 1,
        needs_confirmation: installs.len() > 1,
        command: format!("{command_name} --version"),
        anchored: !installs.is_empty(),
        installs,
    }
}

#[tauri::command]
pub async fn run_tool_lifecycle_action(
    tools: Vec<String>,
    action: String,
    wsl_shell_by_tool: Option<serde_json::Value>,
) -> Result<(), String> {
    let action = action.trim().to_ascii_lowercase();
    if !matches!(action.as_str(), "install" | "update") {
        return Err(format!("Unsupported tool lifecycle action: {action}"));
    }
    let selected = tools
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| validate_tool_name(value))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err("No supported tools selected".to_string());
    }
    tokio::task::spawn_blocking(move || {
        run_lifecycle_action(&selected, &action, wsl_shell_by_tool.as_ref())
    })
    .await
    .map_err(|error| format!("Tool lifecycle task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::{hermes_lifecycle_command, npm_package_for};

    #[test]
    fn package_mapping_is_explicit() {
        assert_eq!(npm_package_for("codex"), Some("@openai/codex"));
        assert_eq!(npm_package_for("hermes"), None);
        assert_eq!(npm_package_for("unknown"), None);
    }

    #[test]
    fn hermes_lifecycle_uses_the_official_installer() {
        let (program, args) = hermes_lifecycle_command("install");
        let command = args.join(" ");
        assert!(!command.contains("pip"));
        assert!(command.contains("NousResearch/hermes-agent"));
        assert!(!program.is_empty());
    }
}
