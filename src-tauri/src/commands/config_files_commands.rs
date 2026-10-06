use crate::db::DbState;
use crate::skills::scanner::FolderNode;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct ConfigRoot {
    pub id: String,
    pub name: String,
    pub path: String,
    pub exists: bool,
    pub config_file: Option<String>,
}

struct ConfigRootCandidate {
    id: &'static str,
    name: &'static str,
}

const CONFIG_ROOTS: &[ConfigRootCandidate] = &[
    ConfigRootCandidate {
        id: "claude",
        name: "Claude",
    },
    ConfigRootCandidate {
        id: "codex",
        name: "Codex",
    },
    ConfigRootCandidate {
        id: "gemini",
        name: "Gemini",
    },
    ConfigRootCandidate {
        id: "grokbuild",
        name: "Grok Build",
    },
    ConfigRootCandidate {
        id: "opencode",
        name: "OpenCode",
    },
    ConfigRootCandidate {
        id: "openclaw",
        name: "OpenClaw",
    },
    ConfigRootCandidate {
        id: "hermes",
        name: "Hermes",
    },
    ConfigRootCandidate {
        id: "pi",
        name: "Pi",
    },
    ConfigRootCandidate {
        id: "mcode",
        name: "MiniMax Code",
    },
    ConfigRootCandidate {
        id: "claude-desktop",
        name: "Claude Desktop",
    },
];

fn config_root_paths(
    conn: &rusqlite::Connection,
) -> Result<Vec<(String, String, PathBuf)>, String> {
    use crate::commands::extra_commands::resolve_tool_config_dir;
    let mut roots = Vec::new();
    for root in CONFIG_ROOTS {
        let directory = if root.id == "claude-desktop" {
            match crate::configured_paths::read(
                conn,
                root.id,
                crate::configured_paths::Field::ConfigDir,
            )? {
                Some(directory) => directory,
                None => crate::mcp::config::claude_desktop_config_path()
                    .and_then(|path| path.parent().map(Path::to_owned))
                    .ok_or("Cannot find Claude Desktop config directory")?,
            }
        } else {
            resolve_tool_config_dir(conn, root.id)?
        };
        mcp_file_for_root(conn, root.id)?;
        roots.push((root.id.to_owned(), root.name.to_owned(), directory));
    }
    Ok(roots)
}

fn mcp_file_for_root(
    conn: &rusqlite::Connection,
    root_id: &str,
) -> Result<Option<PathBuf>, String> {
    if matches!(root_id, "pi" | "openclaw") {
        return Ok(None);
    }
    crate::commands::extra_commands::resolve_tool_mcp_path(conn, root_id).map(Some)
}

fn roots_from_conn(conn: &rusqlite::Connection) -> Result<Vec<ConfigRoot>, String> {
    config_root_paths(conn)?
        .into_iter()
        .map(|(id, name, path)| {
            let has_mcp = mcp_file_for_root(conn, &id)?.is_some_and(|file| file.is_file());
            let config_file = if id == "openclaw" {
                Some(
                    crate::commands::extra_commands::resolve_tool_config_path(conn, &id)?
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                None
            };
            Ok(ConfigRoot {
                id,
                name,
                exists: path.exists() || has_mcp,
                path: path.to_string_lossy().into_owned(),
                config_file,
            })
        })
        .collect()
}

pub(crate) fn count_existing_config_roots(conn: &rusqlite::Connection) -> Result<usize, String> {
    Ok(roots_from_conn(conn)?
        .iter()
        .filter(|root| root.exists)
        .count())
}

#[cfg(test)]
mod tests;

fn resolve_root_path(conn: &rusqlite::Connection, root_id: &str) -> Result<PathBuf, String> {
    let (_, _, path) = config_root_paths(conn)?
        .into_iter()
        .find(|(id, _, _)| id == root_id)
        .ok_or_else(|| format!("Unknown config root: {}", root_id))?;
    Ok(path)
}

fn is_allowed_path(conn: &rusqlite::Connection, path: &Path) -> Result<bool, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve path {}: {}", path.display(), e))?;
    let roots = config_root_paths(conn)?;
    for (id, _, root) in roots {
        if let Some(file) = mcp_file_for_root(conn, &id)? {
            // A separate MCP document grants access to this exact file only.
            if file.is_file()
                && file
                    .canonicalize()
                    .map_err(|_| "Cannot resolve MCP configuration file")?
                    == canonical
            {
                return Ok(true);
            }
        }
        if !root.exists() {
            continue;
        }
        if let Ok(canonical_root) = root.canonicalize() {
            if canonical.starts_with(&canonical_root) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn ensure_allowed_file(conn: &rusqlite::Connection, path: &str) -> Result<PathBuf, String> {
    let file_path = PathBuf::from(path);
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }
    if !file_path.is_file() {
        return Err(format!("Path is not a file: {}", path));
    }
    if !is_allowed_path(conn, &file_path)? {
        return Err(format!("Access denied: {}", path));
    }
    Ok(file_path)
}

fn build_tree(path: &Path, max_depth: usize, depth: usize) -> FolderNode {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    let mut node = FolderNode {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir: path.is_dir(),
        children: Vec::new(),
    };

    if !path.is_dir() || depth >= max_depth {
        return node;
    }

    let mut children = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&entry_path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name == "node_modules" || file_name == "target" || file_name == ".git" {
                continue;
            }
            children.push(build_tree(&entry_path, max_depth, depth + 1));
        }
    }

    children.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    node.children = children;
    node
}

#[tauri::command]
pub fn get_config_roots(db: State<'_, DbState>) -> Result<Vec<ConfigRoot>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    roots_from_conn(&conn)
}

fn tree_from_conn(conn: &rusqlite::Connection, root_id: &str) -> Result<FolderNode, String> {
    let root = resolve_root_path(conn, root_id)?;
    let external =
        mcp_file_for_root(conn, root_id)?.filter(|file| file.is_file() && !file.starts_with(&root));
    if !root.exists() && external.is_none() {
        return Err(format!("Directory does not exist: {}", root.display()));
    }
    let mut tree = if root.exists() {
        build_tree(&root, 8, 0)
    } else {
        FolderNode {
            name: root_id.to_owned(),
            path: root.to_string_lossy().into_owned(),
            is_dir: true,
            children: Vec::new(),
        }
    };
    if let Some(file) = external {
        let mut entry = build_tree(&file, 0, 0);
        entry.name = format!("MCP · {}", entry.name);
        tree.children.push(entry);
    }
    Ok(tree)
}

#[tauri::command]
pub fn get_config_file_tree(root_id: String, db: State<'_, DbState>) -> Result<FolderNode, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    tree_from_conn(&conn, &root_id)
}

#[tauri::command]
pub fn read_config_file_content(path: String, db: State<'_, DbState>) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let file_path = ensure_allowed_file(&conn, &path)?;
    std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))
}

#[tauri::command]
pub fn write_config_file_content(
    path: String,
    content: String,
    expected_content: Option<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let file_path = ensure_allowed_file(&conn, &path)?;
    write_checked_content(&file_path, &content, expected_content.as_deref())
}

fn write_checked_content(path: &Path, content: &str, expected: Option<&str>) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    let original = crate::config_write::read(path)?;
    if expected.is_some_and(|expected| original.as_deref() != Some(expected.as_bytes())) {
        return Err("Configuration changed externally; reload and review it before saving. Your draft is retained.".into());
    }
    crate::config_write::commit(vec![crate::config_write::FileUpdate {
        path: path.to_path_buf(),
        original,
        desired: content.as_bytes().to_vec(),
    }])
}

#[tauri::command]
pub fn parse_openclaw_config_content(content: String) -> Result<serde_json::Value, String> {
    crate::json_config::parse_json5_object(&content)
}

#[tauri::command]
pub fn edit_openclaw_config_content(
    content: String,
    desired: serde_json::Value,
) -> Result<String, String> {
    crate::json_config::edit_json5_text(&content, &desired)
}
