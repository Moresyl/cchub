//! Strict persisted path reads. A database error must never select a home fallback.
use rusqlite::{Connection, OptionalExtension};
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub(crate) enum Field {
    ConfigDir,
    McpFile,
    SkillsDir,
}

impl Field {
    fn column(self) -> &'static str {
        match self {
            Self::ConfigDir => "config_dir",
            Self::McpFile => "mcp_config_path",
            Self::SkillsDir => "skills_dir",
        }
    }
}

pub(crate) fn validate(value: &str, file: bool) -> Result<Option<PathBuf>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(value);
    if value.chars().any(char::is_control)
        || !path.is_absolute()
        || (file && path.file_name().is_none())
    {
        return Err("Configured paths must be absolute and identify a valid location".into());
    }
    match std::fs::metadata(&path) {
        Ok(metadata) if (file && !metadata.is_file()) || (!file && !metadata.is_dir()) => {
            return Err("Configured path has the wrong file or directory type".into());
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err("Cannot inspect configured path".into());
        }
        _ => {}
    }
    Ok(Some(path))
}

pub(crate) fn read(conn: &Connection, tool: &str, field: Field) -> Result<Option<PathBuf>, String> {
    // Only the closed enum supplies SQL identifiers; the tool is bound data.
    let value = conn
        .query_row(
            &format!(
                "SELECT {} FROM custom_paths WHERE tool_id = ?1",
                field.column()
            ),
            [tool],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|_| "Cannot read configured tool paths; repair settings before continuing")?
        .flatten();
    value
        .as_deref()
        .map(|value| validate(value, matches!(field, Field::McpFile)))
        .transpose()
        .map(Option::flatten)
}

#[cfg(test)]
mod tests;
