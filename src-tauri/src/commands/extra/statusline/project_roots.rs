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

pub fn restore_imported_project_root_snapshot(
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

pub fn store_imported_project_file(
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

pub fn apply_project_root_remap(
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

pub fn get_pending_imported_project_roots_from_conn(
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

pub fn project_root_match_key(path: &str) -> Option<String> {
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

pub fn normalized_path_segments(path: &str) -> Vec<String> {
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

pub fn shared_trailing_segment_count(left: &str, right: &str) -> usize {
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

pub fn best_project_root_candidate<'a>(
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

pub fn build_tool_environment_report_from_conn(
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
