use std::path::PathBuf;

use super::super::config_profiles::*;
use super::backup_file_rollback::FileRollback;
use super::*;

type RestoreCounts = (usize, usize, usize, usize, usize);

struct ToolConfig {
    plan: crate::config_write::FilePlan,
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

fn prepare_tool(
    conn: &rusqlite::Connection,
    tool: &str,
    content: String,
) -> Result<crate::config_write::FilePlan, String> {
    let targets = tool_targets(conn, tool)?;
    let mut plan = crate::config_write::FilePlan::default();
    let claude_snapshot = tool == "claude"
        && serde_json::from_str::<serde_json::Value>(&content)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .is_some_and(|object| {
                object.contains_key("__claude_json_keys__")
                    || object.contains_key("__settings_json_keys__")
            });
    if tool == "claude-settings" || (tool == "claude" && !claude_snapshot) {
        // Raw legacy backups may contain comments or opaque bytes. Restore those
        // bytes exactly; profile-snapshot interpretation is a separate format.
        plan.replace(targets[0].clone(), content.into_bytes())?;
    } else if tool == "hermes" {
        let effective =
            crate::provider_proxy::materialize_tool_snapshot_for_runtime(conn, tool, &content)?;
        plan = crate::hermes::snapshot::prepare_snapshot(conn, &effective, false)?.0;
    } else {
        plan = prepare_tool_snapshot(conn, tool, &content, false)?;
    }
    for update in &mut plan.updates {
        update.path = checked_file(update.path.clone())?;
    }
    for (target, _) in &mut plan.guards {
        *target = checked_file(target.clone())?;
    }
    plan.check_targets()?;
    Ok(plan)
}

pub(super) fn restore_artifacts_with_rollback(
    conn: &rusqlite::Connection,
    restored_count: usize,
    rollback: &mut FileRollback,
    defer_projects: bool,
) -> Result<RestoreCounts, String> {
    let _guard = crate::json_config::write_lock()?;
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
        tools.push(ToolConfig {
            plan: prepare_tool(conn, &tool, content)
                .map_err(|_| "工具配置恢复失败，请检查配置格式和目标目录")?,
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
            if defer_projects || !std::path::Path::new(project_root).exists() {
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
        .flat_map(|tool| {
            tool.plan
                .updates
                .iter()
                .map(|update| &update.path)
                .chain(tool.plan.guards.iter().map(|(target, _)| target))
        })
        .chain(skills.iter().chain(&files).map(|file| &file.target))
    {
        rollback.capture(target)?;
    }
    let tool_count = tools.len();
    for tool in tools {
        rollback
            .apply_plan(tool.plan)
            .map_err(|_| "工具配置恢复失败，请检查配置格式和目标目录")?;
    }
    for file in skills.iter().chain(&files) {
        rollback
            .write(&file.target, &file.bytes)
            .map_err(|_| "附属文件恢复失败，请检查目标目录权限")?;
    }
    for file in &projects {
        store_imported_project_file(conn, &file.root, &file.path, &file.content)?;
    }
    if defer_projects {
        super::backup_project_state::defer_project_roots(
            conn,
            projects.iter().map(|file| file.root.as_str()),
        )?;
    }
    rollback.verify()?;
    conn.execute_batch("DROP TABLE _backup_meta; DROP TABLE _tool_configs; DROP TABLE _skill_files; DROP TABLE _backup_files;").map_err(|_| "无法完成备份记录整理")?;
    Ok((
        restored_count.saturating_sub(temp_rows),
        tool_count,
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
    match restore_artifacts_with_rollback(conn, count, &mut rollback, false) {
        Ok(counts) => {
            rollback.commit();
            Ok(counts)
        }
        Err(error) => Err(rollback.rollback(error)),
    }
}
