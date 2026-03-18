use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DetectedTool {
    pub id: String,
    pub name: String,
    pub config_path: String,
    pub skills_dir: String,
    pub mcp_config_path: String,
    pub installed: bool,
    pub install_command: String,
    pub install_url: String,
}

struct ToolCandidate {
    id: &'static str,
    name: &'static str,
    dir: &'static str,
    config_file: &'static str,
    mcp_config_file: &'static str,
    skills_subdir: &'static str,
    install_command: &'static str,
    install_url: &'static str,
}

const TOOL_CANDIDATES: &[ToolCandidate] = &[
    ToolCandidate {
        id: "claude",
        name: "Claude Code",
        dir: ".claude",
        config_file: "settings.json",
        mcp_config_file: "settings.json",
        skills_subdir: "skills",
        install_command: "npm install -g @anthropic-ai/claude-code",
        install_url: "https://docs.anthropic.com/en/docs/claude-code",
    },
    ToolCandidate {
        id: "codex",
        name: "Codex CLI",
        dir: ".codex",
        config_file: "config.toml",
        mcp_config_file: "config.toml",
        skills_subdir: "skills",
        install_command: "npm install -g @openai/codex",
        install_url: "https://github.com/openai/codex",
    },
    ToolCandidate {
        id: "gemini",
        name: "Gemini CLI",
        dir: ".gemini",
        config_file: "settings.json",
        mcp_config_file: "settings.json",
        skills_subdir: "skills",
        install_command: "npm install -g @anthropic-ai/claude-code",
        install_url: "https://github.com/google-gemini/gemini-cli",
    },
    ToolCandidate {
        id: "cursor",
        name: "Cursor",
        dir: ".cursor",
        config_file: "mcp.json",
        mcp_config_file: "mcp.json",
        skills_subdir: "skills",
        install_command: "",
        install_url: "https://cursor.com",
    },
    ToolCandidate {
        id: "windsurf",
        name: "Windsurf",
        dir: ".windsurf",
        config_file: "mcp.json",
        mcp_config_file: "mcp.json",
        skills_subdir: "skills",
        install_command: "",
        install_url: "https://windsurf.com",
    },
    ToolCandidate {
        id: "opencode",
        name: "OpenCode",
        dir: ".opencode",
        config_file: "opencode.json",
        mcp_config_file: "opencode.json",
        skills_subdir: "skills",
        install_command: "go install github.com/opencode-ai/opencode@latest",
        install_url: "https://github.com/opencode-ai/opencode",
    },
    ToolCandidate {
        id: "openclaw",
        name: "OpenClaw",
        dir: ".openclaw",
        config_file: "config.json",
        mcp_config_file: "config.json",
        skills_subdir: "skills",
        install_command: "npm install -g @anthropic-ai/openclaw",
        install_url: "https://github.com/anthropics/openclaw",
    },
];

/// Detect AI coding tools installed on the system
pub fn detect_tools() -> Vec<DetectedTool> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    TOOL_CANDIDATES
        .iter()
        .map(|t| {
            let base = home.join(t.dir);
            let config_path = base.join(t.config_file);
            let mcp_config_path = base.join(t.mcp_config_file);
            let skills_dir = base.join(t.skills_subdir);
            let installed = base.exists();

            DetectedTool {
                id: t.id.to_string(),
                name: t.name.to_string(),
                config_path: config_path.to_string_lossy().to_string(),
                skills_dir: skills_dir.to_string_lossy().to_string(),
                mcp_config_path: mcp_config_path.to_string_lossy().to_string(),
                installed,
                install_command: t.install_command.to_string(),
                install_url: t.install_url.to_string(),
            }
        })
        .collect()
}
