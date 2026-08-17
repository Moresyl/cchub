use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
        id: "grokbuild",
        name: "Grok Build",
        dir: ".grok",
        config_file: "config.toml",
        mcp_config_file: "config.toml",
        skills_subdir: "skills",
        install_command: "npm install -g @xai/grok-cli",
        install_url: "https://docs.x.ai/docs/guides/grok-build",
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
        config_file: "openclaw.json",
        mcp_config_file: "openclaw.json",
        skills_subdir: "skills",
        install_command: "npm install -g openclaw",
        install_url: "https://github.com/openclaw-ai/openclaw",
    },
    ToolCandidate {
        id: "hermes",
        name: "Hermes Agent",
        dir: ".hermes",
        config_file: "config.yaml",
        mcp_config_file: "config.yaml",
        skills_subdir: "skills",
        install_command:
            "curl -fsSL https://raw.githubusercontent.com/NousResearch/hermes-agent/main/scripts/install.sh | bash",
        install_url: "https://github.com/NousResearch/hermes-agent",
    },
    ToolCandidate {
        id: "pi",
        name: "Pi Coding Agent",
        dir: ".pi\\agent",
        config_file: "models.json",
        mcp_config_file: "settings.json",
        skills_subdir: "skills",
        install_command: "npm i -g @earendil-works/pi-coding-agent@latest",
        install_url: "https://pi.dev",
    },
];

/// Detect AI coding tools installed on the system
pub fn detect_tools() -> Vec<DetectedTool> {
    detect_tools_with_conn(None)
}

pub fn detect_tools_for_conn(conn: &Connection) -> Vec<DetectedTool> {
    detect_tools_with_conn(Some(conn))
}

/// `detect_tools` 在应用启动后会被多个 query 在 1 秒内连续调用 5+ 次，
/// 每次都做 12+ 次同步 fs::exists；安装/卸载工具属于罕见事件，所以
/// 这里加一个短 TTL 的进程内缓存避免重复 stat。
const DETECT_TOOLS_TTL: Duration = Duration::from_millis(1500);
static DETECT_TOOLS_CACHE: Mutex<Option<(Instant, Vec<DetectedTool>)>> = Mutex::new(None);

/// 安装/卸载工具或修改 hermes root 后调用，使下一次 detect_tools 重新扫描。
/// 目前缓存 TTL 是 1.5s，足以覆盖启动 prefetch 集中调用；用户主动安装/卸载是
/// 罕见动作且需要 UI 在 1-2 秒内反映，自然过期就能满足，因此暂未在 mutation
/// 路径调用此 invalidate（保留 API 以便未来需要更激进刷新时使用）。
#[allow(dead_code)]
pub fn invalidate_detect_tools_cache() {
    if let Ok(mut guard) = DETECT_TOOLS_CACHE.lock() {
        *guard = None;
    }
}

fn base_dir_for_candidate(
    candidate: &ToolCandidate,
    home: &std::path::Path,
    conn: Option<&Connection>,
) -> PathBuf {
    if candidate.id == "hermes" {
        if let Some(conn) = conn {
            if let Ok(path) = crate::hermes::hermes_root(conn) {
                return path;
            }
        }
    }

    home.join(candidate.dir)
}

fn detect_tools_with_conn(conn: Option<&Connection>) -> Vec<DetectedTool> {
    // 命中短 TTL 缓存就直接返回，避免在启动数据预热时重复 12×stat。
    if let Ok(guard) = DETECT_TOOLS_CACHE.lock() {
        if let Some((cached_at, ref cached)) = *guard {
            if cached_at.elapsed() < DETECT_TOOLS_TTL {
                return cached.clone();
            }
        }
    }

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let result: Vec<DetectedTool> = TOOL_CANDIDATES
        .iter()
        .map(|t| {
            let base = base_dir_for_candidate(t, &home, conn);
            let config_path = base.join(t.config_file);
            let mcp_config_path = if t.id == "claude" {
                home.join(".claude.json")
            } else {
                base.join(t.mcp_config_file)
            };
            let skills_dir = base.join(t.skills_subdir);
            let installed = if t.id == "claude" {
                base.exists() || mcp_config_path.exists()
            } else {
                base.exists()
            };

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
        .collect();

    if let Ok(mut guard) = DETECT_TOOLS_CACHE.lock() {
        *guard = Some((Instant::now(), result.clone()));
    }
    result
}
