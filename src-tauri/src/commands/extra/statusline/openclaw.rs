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

pub fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}

pub fn path_is_within(path: &std::path::Path, root: &std::path::Path) -> bool {
    path.starts_with(root)
}

pub fn collect_backup_file_rows(
    base_path: &std::path::Path,
    root_key: &str,
    relative_prefix: &std::path::Path,
    rows: &mut Vec<(String, String, String)>,
) {
    let entries = match std::fs::read_dir(base_path) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let next_relative = relative_prefix.join(name);

        if path.is_dir() {
            collect_backup_file_rows(&path, root_key, &next_relative, rows);
            continue;
        }

        if !path.is_file() {
            continue;
        }

        if let Ok(bytes) = std::fs::read(&path) {
            let relative = next_relative.to_string_lossy().replace('\\', "/");
            let content_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            rows.push((root_key.to_string(), relative, content_base64));
        }
    }
}

pub fn collect_backup_entry_row(
    path: &std::path::Path,
    root_key: &str,
    relative_path: &std::path::Path,
    rows: &mut Vec<(String, String, String)>,
) {
    if !path.is_file() {
        return;
    }

    if let Ok(bytes) = std::fs::read(path) {
        let relative = relative_path.to_string_lossy().replace('\\', "/");
        let content_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        rows.push((root_key.to_string(), relative, content_base64));
    }
}

pub fn discover_project_roots(conn: &rusqlite::Connection) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    let mut push_root = |raw_path: String| {
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            return;
        }

        let key = trimmed.replace('\\', "/");
        if !seen.insert(key) {
            return;
        }

        let path = PathBuf::from(trimmed);
        if path.exists() {
            roots.push(path);
        }
    };

    if let Ok(mut stmt) = conn.prepare(
        "SELECT base_path FROM workspaces WHERE base_path IS NOT NULL AND trim(base_path) != ''",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                push_root(row);
            }
        }
    }

    if let Ok(mut stmt) = conn.prepare("SELECT project_path FROM hooks WHERE project_path IS NOT NULL AND trim(project_path) != ''") {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                push_root(row);
            }
        }
    }

    let known_roots: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'known_project_roots'",
            [],
            |row| row.get(0),
        )
        .ok();
    if let Some(raw) = known_roots {
        if let Ok(paths) = serde_json::from_str::<Vec<String>>(&raw) {
            for path in paths {
                push_root(path);
            }
        }
    }

    roots
}

pub fn is_openclaw_daily_memory_candidate(
    path: &std::path::Path,
    base_dir: &std::path::Path,
) -> bool {
    if !path.is_file() {
        return false;
    }

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    let extension_allowed = matches!(
        extension.as_deref(),
        None | Some("md" | "txt" | "json" | "jsonl" | "yaml" | "yml" | "log")
    );
    if !extension_allowed {
        return false;
    }

    let relative = match path.strip_prefix(base_dir) {
        Ok(relative) => relative,
        Err(_) => return false,
    };

    let mut components = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(|component| component.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if components.is_empty() {
        return false;
    }

    let file_name = components.pop().unwrap_or_default();
    if file_name.contains("memory") || file_name.contains("journal") || file_name.contains("diary")
    {
        return true;
    }

    components.iter().any(|component| {
        component.contains("memory")
            || component.contains("journal")
            || component.contains("daily")
            || component.contains("diary")
    })
}

pub fn format_local_datetime(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

pub fn condense_openclaw_memory_preview(text: &str) -> Option<String> {
    let condensed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if condensed.is_empty() {
        None
    } else {
        Some(condensed.chars().take(220).collect::<String>())
    }
}

pub fn build_openclaw_memory_preview(content: &str, query: Option<&str>) -> Option<String> {
    let normalized = content.replace('\r', "");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(query) = query {
        let query = query.trim();
        if !query.is_empty() {
            let lowered_content = trimmed.to_lowercase();
            let lowered_query = query.to_lowercase();
            if !lowered_content.contains(&lowered_query) {
                return None;
            }
            if let Some(line_preview) = trimmed
                .lines()
                .find(|line| line.to_lowercase().contains(&lowered_query))
                .and_then(condense_openclaw_memory_preview)
            {
                return Some(line_preview);
            }
        }
    }

    condense_openclaw_memory_preview(trimmed)
}

pub fn is_valid_openclaw_daily_memory_path(
    path: &std::path::Path,
    conn: &rusqlite::Connection,
) -> bool {
    let canonical_path = match std::fs::canonicalize(path) {
        Ok(path) if path.is_file() => path,
        _ => return false,
    };

    let mut roots = Vec::new();

    if let Some(home) = dirs::home_dir() {
        let global_dir = home.join(".openclaw");
        if let Ok(global_root) = std::fs::canonicalize(&global_dir) {
            roots.push(global_root);
        }
    }

    for project_root in discover_project_roots(conn) {
        let openclaw_root = project_root.join(".openclaw");
        if let Ok(root) = std::fs::canonicalize(&openclaw_root) {
            roots.push(root);
        }
    }

    roots.into_iter().any(|root| {
        canonical_path.starts_with(&root)
            && is_openclaw_daily_memory_candidate(&canonical_path, &root)
    })
}

pub fn collect_openclaw_daily_memory_files(
    current_dir: &std::path::Path,
    base_dir: &std::path::Path,
    source: &str,
    project_name: Option<&str>,
    query: Option<&str>,
    entries: &mut Vec<OpenClawDailyMemoryEntry>,
    depth: usize,
) {
    if depth > 5 {
        return;
    }

    let read_dir = match std::fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_openclaw_daily_memory_files(
                &path,
                base_dir,
                source,
                project_name,
                query,
                entries,
                depth + 1,
            );
            continue;
        }

        if !is_openclaw_daily_memory_candidate(&path, base_dir) {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let preview = match build_openclaw_memory_preview(&content, query) {
            Some(preview) => preview,
            None => continue,
        };

        let modified_at = std::fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(format_local_datetime);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        entries.push(OpenClawDailyMemoryEntry {
            path: path.to_string_lossy().to_string(),
            file_name,
            source: source.to_string(),
            project_name: project_name.map(str::to_string),
            modified_at,
            preview,
        });
    }
}

pub fn normalize_project_root_path(path: &str) -> Option<&str> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.trim_end_matches(['\\', '/']))
    }
}

pub fn project_root_paths_match(left: &str, right: &str) -> bool {
    normalize_project_root_path(left)
        .zip(normalize_project_root_path(right))
        .is_some_and(|(left, right)| {
            left.replace('\\', "/")
                .eq_ignore_ascii_case(&right.replace('\\', "/"))
        })
}

pub fn sync_known_project_root(
    conn: &rusqlite::Connection,
    previous_path: Option<&str>,
    next_path: Option<&str>,
) -> Result<(), String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'known_project_roots'",
            [],
            |row| row.get(0),
        )
        .ok();

    let mut roots: Vec<String> = existing
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();

    if let Some(previous_path) = previous_path.and_then(normalize_project_root_path) {
        roots.retain(|value| !project_root_paths_match(value, previous_path));
    }

    if let Some(next_path) = next_path.and_then(normalize_project_root_path) {
        if !roots
            .iter()
            .any(|value| project_root_paths_match(value, next_path))
        {
            roots.push(next_path.to_string());
        }
    }

    let payload = serde_json::to_string(&roots).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('known_project_roots', ?1)",
        rusqlite::params![payload],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
