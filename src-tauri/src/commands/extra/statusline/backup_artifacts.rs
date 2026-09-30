use std::path::PathBuf;

use super::super::config_profiles::*;
use super::backup_file_rollback::FileRollback;
use super::*;

type RestoreCounts = (usize, usize, usize, usize, usize);

struct ToolConfig {
    tool: String,
    content: String,
    targets: Vec<PathBuf>,
}
struct FileContent {
    target: PathBuf,
    bytes: Vec<u8>,
}
struct ProjectFile {
    root: String,
    path: String,
    content: String,
}

fn checked_file(path: PathBuf) -> Result<PathBuf, String> {
    let parent = path.parent().ok_or("无效的工具配置文件路径")?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("无效的工具配置文件名")?;
    super::backup_paths::confined_target(parent, name, false)
}

fn tool_targets(conn: &rusqlite::Connection, tool: &str) -> Result<Vec<PathBuf>, String> {
    let paths = match tool {
        "claude-settings" => vec![resolve_claude_paths(conn)?.1],
        "claude" => {
            let (mcp, settings) = resolve_claude_paths(conn)?;
            vec![mcp, settings]
        }
        "codex" => vec![
            resolve_tool_config_dir(conn, tool)?.join("auth.json"),
            resolve_tool_config_path(conn, tool)?,
        ],
        "gemini" => vec![
            resolve_tool_config_dir(conn, tool)?.join(".env"),
            resolve_tool_config_path(conn, tool)?,
        ],
        "hermes" => vec![
            crate::hermes::config_path(conn)?,
            crate::hermes::env_path(conn)?,
        ],
        "grokbuild" => vec![resolve_tool_config_path(conn, tool)?],
        "opencode" | "openclaw" | "pi" => vec![resolve_tool_config_path(conn, tool)?],
        _ => return Err("备份包含不支持的工具配置，恢复已停止".into()),
    };
    paths.into_iter().map(checked_file).collect()
}

pub(super) fn restore_artifacts_with_rollback(
    conn: &rusqlite::Connection,
    restored_count: usize,
    rollback: &mut FileRollback,
) -> Result<RestoreCounts, String> {
    super::backup_paths::validate_artifact_paths(conn)?;
    let temp_rows: usize = conn.query_row("SELECT (SELECT COUNT(*) FROM _backup_meta) + (SELECT COUNT(*) FROM _tool_configs) + (SELECT COUNT(*) FROM _skill_files) + (SELECT COUNT(*) FROM _backup_files)", [], |row| row.get(0)).map_err(|_| "无法读取备份记录")?;
    let mut tools = Vec::new();
    let mut stmt = conn
        .prepare("SELECT tool_id, config_content FROM _tool_configs ORDER BY tool_id")
        .map_err(|_| "无法读取工具备份")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|_| "无法读取工具备份")?;
    for row in rows {
        let (tool, content) = row.map_err(|_| "工具备份记录无效")?;
        let targets = tool_targets(conn, &tool)?;
        tools.push(ToolConfig {
            tool,
            content,
            targets,
        });
    }
    let mut skills = Vec::new();
    let mut stmt = conn
        .prepare("SELECT tool_id, name, content FROM _skill_files ORDER BY rowid")
        .map_err(|_| "无法读取技能备份")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| "无法读取技能备份")?;
    for row in rows {
        let (tool, name, content) = row.map_err(|_| "技能备份记录无效")?;
        let tool = if tool == "claude-settings" {
            "claude"
        } else {
            &tool
        };
        let root = resolve_tool_skills_dir(conn, tool)?;
        skills.push(FileContent {
            target: super::backup_paths::confined_target(&root, &name, false)?,
            bytes: content.into_bytes(),
        });
    }
    let mut files = Vec::new();
    let mut projects = Vec::new();
    let mut pending = 0;
    let mut stmt = conn
        .prepare("SELECT root_key, relative_path, content_base64 FROM _backup_files ORDER BY rowid")
        .map_err(|_| "无法读取附属文件备份")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| "无法读取附属文件备份")?;
    for row in rows {
        let (root, path, content) = row.map_err(|_| "附属文件备份记录无效")?;
        if let Some(project_root) = root.strip_prefix("project:") {
            projects.push(ProjectFile {
                root: project_root.into(),
                path: path.clone(),
                content: content.clone(),
            });
            if !std::path::Path::new(project_root).exists() {
                pending += 1;
                continue;
            }
        }
        let target =
            super::backup_paths::confined_target(&resolve_backup_root(conn, &root)?, &path, true)?;
        files.push(FileContent {
            target,
            bytes: super::backup_paths::decode_file(&content)?,
        });
    }
    // Capture every destination before the first tool/file write.
    for target in tools
        .iter()
        .flat_map(|tool| &tool.targets)
        .chain(skills.iter().chain(&files).map(|file| &file.target))
    {
        rollback.capture(target)?;
    }
    for tool in &tools {
        rollback.before_write(&tool.targets)?;
        let result = match tool.tool.as_str() {
            "claude-settings" => crate::utils::atomic_write_string(&tool.targets[0], &tool.content)
                .map_err(|_| "无法写入工具配置".to_string()),
            "claude" => {
                let parsed = serde_json::from_str::<serde_json::Value>(&tool.content).ok();
                let snapshot = parsed
                    .as_ref()
                    .and_then(|value| value.as_object())
                    .is_some_and(|obj| {
                        obj.contains_key("__claude_json_keys__")
                            || obj.contains_key("__settings_json_keys__")
                    });
                if snapshot {
                    apply_tool_snapshot(conn, "claude", &tool.content)
                } else {
                    crate::utils::atomic_write_string(&tool.targets[0], &tool.content)
                        .map_err(|_| "无法写入工具配置".to_string())
                }
            }
            "hermes" => {
                let snapshot = crate::provider_proxy::materialize_tool_snapshot_for_runtime(
                    conn,
                    "hermes",
                    &tool.content,
                )?;
                crate::hermes::snapshot::apply_snapshot_without_backup(conn, &snapshot).map(|_| ())
            }
            _ => apply_tool_snapshot(conn, &tool.tool, &tool.content),
        };
        result.map_err(|_| "工具配置恢复失败，请检查配置格式和目标目录".to_string())?;
    }
    for file in skills.iter().chain(&files) {
        rollback.before_write(std::slice::from_ref(&file.target))?;
        crate::utils::atomic_write(&file.target, &file.bytes)
            .map_err(|_| "附属文件恢复失败，请检查目标目录权限")?;
    }
    for file in &projects {
        store_imported_project_file(conn, &file.root, &file.path, &file.content)?;
    }
    conn.execute_batch("DROP TABLE _backup_meta; DROP TABLE _tool_configs; DROP TABLE _skill_files; DROP TABLE _backup_files;").map_err(|_| "无法完成备份记录整理")?;
    Ok((
        restored_count.saturating_sub(temp_rows),
        tools.len(),
        skills.len(),
        files.len(),
        pending,
    ))
}

#[cfg(test)]
pub fn restore_imported_artifacts(
    conn: &rusqlite::Connection,
    count: usize,
) -> Result<RestoreCounts, String> {
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut rollback = FileRollback::new(temp.path())?;
    match restore_artifacts_with_rollback(conn, count, &mut rollback) {
        Ok(counts) => {
            rollback.commit();
            Ok(counts)
        }
        Err(error) => Err(rollback.rollback(error)),
    }
}
