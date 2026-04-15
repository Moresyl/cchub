pub mod config;
pub mod env;
pub mod mcp;
pub mod providers;
pub mod snapshot;

use rusqlite::Connection;
use std::path::PathBuf;

pub const ROOT_OVERRIDE_SETTING_KEY: &str = "hermes.rootOverride";

pub fn default_root() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".hermes"))
        .ok_or_else(|| "Cannot find home directory".to_string())
}

pub fn read_root_override(conn: &Connection) -> Result<Option<String>, String> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![ROOT_OVERRIDE_SETTING_KEY],
            |row| row.get(0),
        )
        .ok();

    Ok(value.and_then(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }))
}

pub fn write_root_override(conn: &Connection, value: Option<&str>) -> Result<Option<String>, String> {
    let normalized = value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(str::to_string);

    match normalized.as_deref() {
        Some(path) => {
            conn.execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
                rusqlite::params![ROOT_OVERRIDE_SETTING_KEY, path],
            )
            .map_err(|e| e.to_string())?;
        }
        None => {
            conn.execute(
                "DELETE FROM app_settings WHERE key = ?1",
                rusqlite::params![ROOT_OVERRIDE_SETTING_KEY],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(normalized)
}

pub fn hermes_root(conn: &Connection) -> Result<PathBuf, String> {
    if let Some(override_path) = read_root_override(conn)? {
        return Ok(PathBuf::from(override_path));
    }
    default_root()
}

pub fn config_path(conn: &Connection) -> Result<PathBuf, String> {
    Ok(hermes_root(conn)?.join("config.yaml"))
}

pub fn env_path(conn: &Connection) -> Result<PathBuf, String> {
    Ok(hermes_root(conn)?.join(".env"))
}

pub fn skills_dir(conn: &Connection) -> Result<PathBuf, String> {
    Ok(hermes_root(conn)?.join("skills"))
}
