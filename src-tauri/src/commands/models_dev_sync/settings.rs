use crate::commands::extended_compat::{normalize_sync_config, ModelsDevSyncConfig};
use rusqlite::{Connection, OptionalExtension};

pub fn same_preferences(left: &ModelsDevSyncConfig, right: &ModelsDevSyncConfig) -> bool {
    let left = normalize_sync_config(left.clone());
    let right = normalize_sync_config(right.clone());
    left.auto_sync_enabled == right.auto_sync_enabled
        && left.include_common_models == right.include_common_models
        && left.selected_model_keys == right.selected_model_keys
        && left.excluded_common_model_keys == right.excluded_common_model_keys
}

pub fn read_config(conn: &Connection) -> Result<ModelsDevSyncConfig, String> {
    let raw = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'models_dev_sync_config'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| "PRICING_SETTINGS_READ_FAILED".to_string())?;
    match raw {
        None => Ok(ModelsDevSyncConfig::default()),
        Some(raw) => serde_json::from_str(&raw)
            .map(normalize_sync_config)
            .map_err(|_| "PRICING_SETTINGS_INVALID".to_string()),
    }
}

pub fn write_config(conn: &Connection, config: &ModelsDevSyncConfig) -> Result<(), String> {
    let payload =
        serde_json::to_string(config).map_err(|_| "PRICING_SETTINGS_WRITE_FAILED".to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('models_dev_sync_config', ?1)",
        [payload],
    )
    .map_err(|_| "PRICING_SETTINGS_WRITE_FAILED".to_string())?;
    Ok(())
}

pub fn save_preferences(
    conn: &mut Connection,
    desired: ModelsDevSyncConfig,
    expected: Option<ModelsDevSyncConfig>,
) -> Result<ModelsDevSyncConfig, String> {
    let tx = conn
        .transaction()
        .map_err(|_| "PRICING_SETTINGS_WRITE_FAILED".to_string())?;
    let current = read_config(&tx)?;
    if expected
        .as_ref()
        .is_some_and(|expected| !same_preferences(&current, expected))
    {
        return Err("PRICING_SETTINGS_CONFLICT".to_string());
    }
    let mut next = normalize_sync_config(desired);
    next.last_sync_at = current.last_sync_at;
    next.last_sync_error = current.last_sync_error;
    write_config(&tx, &next)?;
    tx.commit()
        .map_err(|_| "PRICING_SETTINGS_WRITE_FAILED".to_string())?;
    Ok(next)
}

pub fn mark_sync_error(conn: &Connection) -> Result<(), String> {
    let mut current = read_config(conn)?;
    current.last_sync_error = Some("PRICING_SYNC_FAILED".to_string());
    write_config(conn, &current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn saves_normalized_preferences_without_replaying_sync_metadata() {
        let mut conn = database();
        let current = ModelsDevSyncConfig {
            last_sync_at: Some(42),
            last_sync_error: Some("failed".into()),
            ..Default::default()
        };
        write_config(&conn, &current).unwrap();
        let desired = ModelsDevSyncConfig {
            selected_model_keys: vec![" model ".into(), "model".into(), "".into()],
            ..Default::default()
        };
        let saved =
            save_preferences(&mut conn, desired, Some(ModelsDevSyncConfig::default())).unwrap();
        assert_eq!(saved.selected_model_keys, ["model"]);
        assert_eq!(saved.last_sync_at, Some(42));
        assert_eq!(saved.last_sync_error.as_deref(), Some("failed"));
        assert!(same_preferences(&saved, &read_config(&conn).unwrap()));
    }

    #[test]
    fn rejects_stale_preferences_and_preserves_latest_changes() {
        let mut conn = database();
        let latest = ModelsDevSyncConfig {
            auto_sync_enabled: true,
            ..Default::default()
        };
        write_config(&conn, &latest).unwrap();
        assert_eq!(
            save_preferences(
                &mut conn,
                ModelsDevSyncConfig::default(),
                Some(ModelsDevSyncConfig::default())
            )
            .unwrap_err(),
            "PRICING_SETTINGS_CONFLICT"
        );
        assert!(read_config(&conn).unwrap().auto_sync_enabled);
        mark_sync_error(&conn).unwrap();
        let after = read_config(&conn).unwrap();
        assert!(after.auto_sync_enabled);
        assert_eq!(
            after.last_sync_error.as_deref(),
            Some("PRICING_SYNC_FAILED")
        );
    }

    #[test]
    fn malformed_settings_are_never_replaced_with_defaults() {
        let mut conn = database();
        conn.execute(
            "INSERT INTO app_settings VALUES ('models_dev_sync_config', '{broken')",
            [],
        )
        .unwrap();
        assert_eq!(read_config(&conn).unwrap_err(), "PRICING_SETTINGS_INVALID");
        assert!(save_preferences(&mut conn, ModelsDevSyncConfig::default(), None).is_err());
        let raw: String = conn
            .query_row("SELECT value FROM app_settings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(raw, "{broken");
        assert!(read_config(&Connection::open_in_memory().unwrap()).is_err());
    }
}
