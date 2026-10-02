//! Prepare composable JSON/JSONC MCP edits before writing files or catalog state.
use super::config::McpServerConfig;
use super::formats::{edit_json_value, JsonMcpFormat};
use crate::config_write::{FilePlan, FileUpdate};
use std::path::Path;

pub(crate) struct Edit<'a> {
    pub name: &'a str,
    pub config: Option<&'a McpServerConfig>,
    pub format: JsonMcpFormat,
}

pub(crate) fn edit_text(source: &str, edits: &[Edit<'_>]) -> Result<String, String> {
    crate::json_config::edit_json_text(source, |document| {
        for edit in edits {
            edit_json_value(document, edit.name, edit.config, edit.format)?;
        }
        Ok(())
    })
}

// Callers acquire the application write lock before the database lock, prepare
// the whole operation, then commit with their immediate SQLite finalizer.
pub(crate) fn prepare(path: &Path, edits: &[Edit<'_>]) -> Result<FilePlan, String> {
    let original = crate::config_write::read(path)?;
    let source = original
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| "Native MCP configuration must be UTF-8")?
        .unwrap_or("{}\n");
    let desired = edit_text(source, edits)?.into_bytes();
    let mut plan = FilePlan::default();
    if original.as_deref() == Some(&desired) || desired == source.as_bytes() {
        plan.guards.push((path.to_owned(), original));
    } else {
        plan.updates.push(FileUpdate {
            path: path.to_owned(),
            original,
            desired,
        });
    }
    Ok(plan)
}

pub(super) fn update_at(
    path: &Path,
    name: &str,
    config: Option<&McpServerConfig>,
    format: JsonMcpFormat,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    prepare(
        path,
        &[Edit {
            name,
            config,
            format,
        }],
    )?
    .commit()
}

#[cfg(test)]
mod tests;
