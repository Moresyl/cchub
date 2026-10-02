use std::path::PathBuf;

/// Resolve the optional shared skill directory selected in Settings.
/// Empty or unknown values keep the legacy per-tool directory behavior.
pub fn configured_skill_storage_dir(conn: &rusqlite::Connection) -> Option<PathBuf> {
    read_skill_storage_dir(conn).ok().flatten()
}

pub(super) fn read_skill_storage_dir(
    conn: &rusqlite::Connection,
) -> Result<Option<PathBuf>, String> {
    use rusqlite::OptionalExtension;
    let location = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'skill_storage_location'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|_| "Cannot read skill storage settings; repair settings before continuing")?
        .flatten();
    let Some(location) = location.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    Ok(storage_root_for_location(&home, &location))
}

fn storage_root_for_location(home: &std::path::Path, location: &str) -> Option<PathBuf> {
    match location.trim().to_ascii_lowercase().as_str() {
        "cchub" => Some(home.join(".cchub").join("skills")),
        "unified" => Some(home.join(".agents").join("skills")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::storage_root_for_location;
    use std::path::Path;

    #[test]
    fn resolves_supported_storage_roots_without_accepting_unknown_values() {
        let home = Path::new("C:/Users/tester");
        assert_eq!(
            storage_root_for_location(home, "cchub"),
            Some(home.join(".cchub").join("skills"))
        );
        assert_eq!(
            storage_root_for_location(home, " UNIFIED "),
            Some(home.join(".agents").join("skills"))
        );
        assert_eq!(storage_root_for_location(home, "tool"), None);
    }
}
