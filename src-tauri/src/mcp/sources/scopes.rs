use super::{SourceBinding, SourceRole};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(super) fn configured(conn: &Connection) -> Result<(Vec<SourceBinding>, Vec<PathBuf>), String> {
    use crate::commands::extra_commands::{
        resolve_tool_config_dir, resolve_tool_config_path, resolve_tool_mcp_path,
    };
    let mut bindings = Vec::new();
    // Resolve all settings first; an invalid setting cannot fall back to home.
    for tool in [
        "claude",
        "claude-desktop",
        "codex",
        "gemini",
        "grokbuild",
        "opencode",
        "hermes",
        "mcode",
    ] {
        bindings.push(SourceBinding {
            tool: tool.into(),
            path: resolve_tool_mcp_path(conn, tool)?,
            role: SourceRole::Primary,
        });
    }
    bindings.push(SourceBinding {
        tool: "claude".into(),
        path: resolve_tool_config_path(conn, "claude")?,
        role: SourceRole::Secondary,
    });
    let root = resolve_tool_config_dir(conn, "claude")?.join("plugins");
    let mut files = Vec::new();
    plugins(&root, &mut files)?;
    files.sort();
    bindings.extend(files.into_iter().map(|path| SourceBinding {
        tool: "claude".into(),
        path,
        role: SourceRole::Plugin,
    }));
    Ok((bindings, vec![root]))
}

pub(super) fn plugins(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut visited = HashSet::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        let metadata = match std::fs::metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && directory == root => {
                continue
            }
            Err(_) => return Err("Cannot inspect the MCP plugin scope".into()),
        };
        if !metadata.is_dir() {
            return Err("MCP plugin scope must be a directory".into());
        }
        let canonical = crate::config_write::target_key(&directory)?;
        if !visited.insert(canonical) {
            // Linked directory aliases/cycles have already been requested.
            continue;
        }
        if visited.len() > 10_000 {
            return Err("MCP plugin scope contains too many directories".into());
        }
        let entries =
            std::fs::read_dir(&directory).map_err(|_| "Cannot read the MCP plugin scope")?;
        for entry in entries {
            let entry = entry.map_err(|_| "Cannot enumerate the MCP plugin scope")?;
            let path = entry.path();
            let metadata =
                std::fs::metadata(&path).map_err(|_| "Cannot inspect an MCP plugin entry")?;
            if metadata.is_dir() {
                pending.push(path);
            } else if entry.file_name() == ".mcp.json" {
                if !metadata.is_file() {
                    return Err("MCP plugin document must be a regular file".into());
                }
                files.push(path);
            }
        }
    }
    Ok(())
}
