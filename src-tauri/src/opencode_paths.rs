//! One path policy shared by configuration, MCP, plugins and tool detection.
use std::path::{Path, PathBuf};

fn absolute_environment_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

pub(crate) fn default_config_dir(home: &Path) -> PathBuf {
    absolute_environment_path("OPENCODE_CONFIG_DIR").unwrap_or_else(|| {
        absolute_environment_path("XDG_CONFIG_HOME")
            .unwrap_or_else(|| home.join(".config"))
            .join("opencode")
    })
}

pub(crate) fn data_dir(home: &Path) -> PathBuf {
    absolute_environment_path("XDG_DATA_HOME")
        .unwrap_or_else(|| home.join(".local").join("share"))
        .join("opencode")
}

pub(crate) fn database_path(home: &Path) -> PathBuf {
    absolute_environment_path("OPENCODE_DB").unwrap_or_else(|| data_dir(home).join("opencode.db"))
}

pub(crate) fn config_in_dir(directory: &Path) -> Result<PathBuf, String> {
    for name in ["opencode.jsonc", "opencode.json"] {
        let path = directory.join(name);
        if path
            .try_exists()
            .map_err(|error| format!("Cannot inspect configuration path: {error}"))?
        {
            if !path.is_file() {
                return Err("OpenCode configuration path must be a regular file".into());
            }
            return Ok(path);
        }
    }
    Ok(directory.join("opencode.json"))
}

fn custom_path(conn: &rusqlite::Connection, column: &str) -> Option<PathBuf> {
    // The column comes from the fixed internal call sites below.
    conn.query_row(
        &format!("SELECT {column} FROM custom_paths WHERE tool_id = 'opencode'"),
        [],
        |row| row.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
    .filter(|value| !value.trim().is_empty())
    .map(PathBuf::from)
}

pub(crate) fn config_dir(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    if let Some(directory) = custom_path(conn, "config_dir") {
        return Ok(directory);
    }
    if let Some(path) = custom_path(conn, "mcp_config_path")
        .or_else(|| absolute_environment_path("OPENCODE_CONFIG"))
    {
        return path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "Invalid OpenCode configuration path".into());
    }
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(default_config_dir(&home))
}

pub(crate) fn config_path(conn: &rusqlite::Connection) -> Result<PathBuf, String> {
    if let Some(path) = custom_path(conn, "mcp_config_path") {
        return Ok(path);
    }
    if let Some(directory) = custom_path(conn, "config_dir") {
        return config_in_dir(&directory);
    }
    default_config_path()
}

pub(crate) fn default_config_path() -> Result<PathBuf, String> {
    if let Some(path) = absolute_environment_path("OPENCODE_CONFIG") {
        return Ok(path);
    }
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    config_in_dir(&default_config_dir(&home))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_existing_jsonc_and_never_creates_files_during_resolution() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            config_in_dir(directory.path()).unwrap(),
            directory.path().join("opencode.json")
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        std::fs::write(directory.path().join("opencode.json"), "{}").unwrap();
        std::fs::write(directory.path().join("opencode.jsonc"), "// note\n{}").unwrap();
        assert_eq!(
            config_in_dir(directory.path()).unwrap(),
            directory.path().join("opencode.jsonc")
        );
    }

    #[test]
    fn honors_explicit_file_and_directory_overrides_with_jsonc() {
        let directory = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE custom_paths (tool_id TEXT, config_dir TEXT, mcp_config_path TEXT)",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO custom_paths VALUES ('opencode', ?1, NULL)",
            [directory.path().to_str().unwrap()],
        )
        .unwrap();
        std::fs::write(directory.path().join("opencode.jsonc"), "{}").unwrap();
        assert_eq!(
            config_path(&conn).unwrap(),
            directory.path().join("opencode.jsonc")
        );
        let explicit = directory.path().join("custom.jsonc");
        conn.execute(
            "UPDATE custom_paths SET config_dir = NULL, mcp_config_path = ?1",
            [explicit.to_str().unwrap()],
        )
        .unwrap();
        assert_eq!(config_path(&conn).unwrap(), explicit);
        assert_eq!(config_dir(&conn).unwrap(), directory.path());
        assert!(!explicit.exists());
    }

    #[test]
    fn rejects_directory_named_like_a_configuration_file() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("opencode.jsonc")).unwrap();
        assert!(config_in_dir(directory.path()).is_err());
    }
}
