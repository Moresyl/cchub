// 纯工具函数：路径解析、文件读写、命令查找、token 生成等
// 不依赖 AutopilotStatus / AutopilotRuntime 等内部状态类型
use serde_json::Value;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use uuid::Uuid;

use crate::utils;

use super::DEFAULT_CONFIRM_TEXT;

pub(super) fn completion_detected(
    last_message_path: &Path,
    done_token: &str,
) -> Result<bool, String> {
    let content = fs::read_to_string(last_message_path).unwrap_or_default();
    let lines = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Ok(lines.len() == 2
        && lines.first().copied().unwrap_or_default() == done_token
        && lines.get(1).copied().unwrap_or_default() == DEFAULT_CONFIRM_TEXT)
}

pub(super) fn find_session_id_in_value(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["session_id", "conversation_id", "thread_id"] {
                if let Some(candidate) = map.get(key).and_then(Value::as_str) {
                    if looks_like_uuid(candidate) {
                        return Some(candidate.to_string());
                    }
                }
            }
            for child in map.values() {
                if let Some(found) = find_session_id_in_value(child) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_session_id_in_value),
        _ => None,
    }
}

pub(super) fn read_last_message_preview(last_message_path: &Path) -> String {
    fs::read_to_string(last_message_path)
        .unwrap_or_default()
        .trim()
        .chars()
        .take(300)
        .collect::<String>()
        .replace('\n', " ↵ ")
}

pub(super) fn append_main_log(path: &Path, level: &str, message: &str) -> Result<(), String> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建日志目录失败: {e}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("写入主日志失败: {e}"))?;
    file.write_all(format!("{timestamp} [{level}] {message}\n").as_bytes())
        .map_err(|e| format!("写入主日志失败: {e}"))?;
    file.flush().map_err(|e| format!("写入主日志失败: {e}"))
}

pub(super) fn now_epoch_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub(super) fn resolve_codex_bin(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Codex 命令不能为空".to_string());
    }

    let direct_path = PathBuf::from(trimmed);
    if direct_path.components().count() > 1 || direct_path.is_absolute() {
        return direct_path
            .canonicalize()
            .map(|path| path.to_string_lossy().to_string())
            .map_err(|e| format!("找不到 Codex: {trimmed} ({e})"));
    }

    let path_env = env::var_os("PATH").unwrap_or_default();
    let path_dirs = env::split_paths(&path_env);
    let extensions: Vec<String> = if cfg!(windows) {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .map(|item| item.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|item| !item.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    for dir in path_dirs {
        let candidate = dir.join(trimmed);
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().to_string());
        }
        if cfg!(windows) {
            for ext in &extensions {
                let candidate = dir.join(format!("{trimmed}.{ext}"));
                if candidate.is_file() {
                    return Ok(candidate.to_string_lossy().to_string());
                }
            }
        }
    }

    Err(format!(
        "找不到 Codex: {trimmed}（请确认已安装并在 PATH 中）"
    ))
}

pub(super) fn canonicalize_existing_file(path: &str, label: &str) -> Result<PathBuf, String> {
    let original = PathBuf::from(path.trim());
    if original.as_os_str().is_empty() {
        return Err(format!("{label} 路径不能为空"));
    }
    if !original.exists() {
        return Err(format!("{label}不存在: {}", original.display()));
    }
    if !original.is_file() {
        return Err(format!("{label}不是文件: {}", original.display()));
    }
    original
        .canonicalize()
        .map_err(|e| format!("解析{label}路径失败: {e}"))
}

pub(super) fn resolve_workdir(value: &str, task_file: &Path) -> Result<PathBuf, String> {
    let raw = if value.trim().is_empty() {
        task_file
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(value.trim())
    };
    if !raw.exists() {
        return Err(format!("工作目录不存在: {}", raw.display()));
    }
    if !raw.is_dir() {
        return Err(format!("工作目录不是文件夹: {}", raw.display()));
    }
    raw.canonicalize()
        .map_err(|e| format!("解析工作目录失败: {e}"))
}

pub(super) fn resolve_run_dir(run_id: &str) -> Result<PathBuf, String> {
    let trimmed = run_id.trim();
    if trimmed.is_empty() {
        return Err("日志记录 ID 不能为空".to_string());
    }
    if trimmed.contains(['/', '\\']) {
        return Err("非法的日志记录 ID".to_string());
    }
    let path = Path::new(trimmed);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("非法的日志记录 ID".to_string());
    }
    Ok(utils::autopilot_runs_dir().join(trimmed))
}

pub(super) fn sanitize_task_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.trim_matches('_').is_empty() {
        "autopilot-task".to_string()
    } else {
        sanitized
    }
}

pub(super) fn write_text_file(path: &Path, content: &str) -> Result<(), String> {
    utils::atomic_write_string(path, content)
        .map_err(|e| format!("写入文件失败 {}: {e}", path.display()))
}

pub(super) fn read_optional_text(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn clear_state_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|e| format!("创建状态目录失败: {e}"))?;
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|e| format!("读取状态目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取状态目录失败: {e}"))?;
        let entry_path = entry.path();
        if entry_path.is_file() {
            fs::remove_file(&entry_path)
                .map_err(|e| format!("清理状态文件失败 {}: {e}", entry_path.display()))?;
        } else if entry_path.is_dir() {
            fs::remove_dir_all(&entry_path)
                .map_err(|e| format!("清理状态目录失败 {}: {e}", entry_path.display()))?;
        }
    }
    Ok(())
}

pub(super) fn kill_process_tree_by_pid(pid: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("taskkill");
        let pid = pid.to_string();
        command.args(["/PID", pid.as_str(), "/T", "/F"]);
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        utils::configure_background_command(&mut command);
        command.status().map_err(|e| format!("停止任务失败: {e}"))?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let pid_arg = pid.to_string();
        let mut command = Command::new("kill");
        command.args(["-TERM", pid_arg.as_str()]);
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command.status().map_err(|e| format!("停止任务失败: {e}"))?;
        Ok(())
    }
}

pub(super) fn generate_nonce() -> (String, String) {
    let raw = Uuid::new_v4().simple().to_string();
    let nonce = format!("{}-{}-{}", &raw[0..4], &raw[4..8], &raw[8..12]);
    let mut parts = nonce.split('-').collect::<Vec<_>>();
    parts.reverse();
    let done_token = parts.join("-");
    (nonce, done_token)
}

pub(super) fn normalize_optional(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub(super) fn looks_like_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if [8, 13, 18, 23].contains(&index) {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

pub(super) fn now_string() -> String {
    chrono::Local::now().to_rfc3339()
}
