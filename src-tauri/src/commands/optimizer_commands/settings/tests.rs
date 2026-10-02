use super::*;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::schema::run_migrations(&conn).unwrap();
    conn
}

fn put(conn: &Connection, key: &str, value: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings(key,value) VALUES(?1,?2)",
        [key, value],
    )
    .unwrap();
}

fn changed(conn: &Connection) -> ProxyAdvancedSettings {
    let mut draft = read(conn).unwrap();
    draft.config.non_streaming_timeout = 42;
    draft.rectifier_config.thinking_budget = false;
    draft
}

fn commit(
    conn: &mut Connection,
    draft: ProxyAdvancedSettings,
) -> Result<ProxyAdvancedSettings, String> {
    save(conn, draft.config, draft.rectifier_config, &draft.revision)
}

#[test]
fn only_missing_settings_use_defaults_and_corrupt_data_never_leaks() {
    let conn = database();
    let defaults = read(&conn).unwrap();
    assert_eq!(defaults.config.non_streaming_timeout, 600);
    assert!(defaults.rectifier_config.enabled);
    assert_eq!(defaults.revision.len(), 64);
    for key in [OPTIMIZER_CONFIG_SETTINGS_KEY, RECTIFIER_CONFIG_SETTINGS_KEY] {
        for invalid in [
            "",
            "null",
            "fixture-private-payload",
            "{\"enabled\":\"private-value\"}",
        ] {
            put(&conn, key, invalid);
            assert_eq!(read(&conn).unwrap_err(), READ_ERROR);
            conn.execute("DELETE FROM app_settings WHERE key=?1", [key])
                .unwrap();
        }
    }
    let mut invalid = defaults.config;
    invalid.streaming_first_byte_timeout = 86_401;
    put(
        &conn,
        OPTIMIZER_CONFIG_SETTINGS_KEY,
        &serde_json::to_string(&invalid).unwrap(),
    );
    assert_eq!(read(&conn).unwrap_err(), READ_ERROR);
    assert_eq!(read_optimizer(&conn).unwrap_err(), READ_ERROR);
    conn.execute_batch("DROP TABLE app_settings").unwrap();
    assert_eq!(read(&conn).unwrap_err(), READ_ERROR);
}

#[test]
fn legacy_optional_fields_keep_their_defined_defaults() {
    let conn = database();
    let mut payload = serde_json::to_value(OptimizerConfig::default()).unwrap();
    for field in [
        "nonStreamingTimeout",
        "streamingFirstByteTimeout",
        "streamingIdleTimeout",
    ] {
        payload.as_object_mut().unwrap().remove(field);
    }
    put(&conn, OPTIMIZER_CONFIG_SETTINGS_KEY, &payload.to_string());
    let settings = read(&conn).unwrap();
    assert_eq!(settings.config.non_streaming_timeout, 600);
    assert_eq!(settings.config.streaming_first_byte_timeout, 60);
    assert_eq!(settings.config.streaming_idle_timeout, 120);
}

#[test]
fn saves_both_settings_and_returns_a_confirmed_revision() {
    let mut conn = database();
    let before = read(&conn).unwrap();
    put(&conn, "unrelated", "untouched");
    let draft = changed(&conn);
    let saved = commit(&mut conn, draft).unwrap();
    assert_ne!(saved.revision, before.revision);
    let stored = read(&conn).unwrap();
    assert_eq!(saved.revision, stored.revision);
    assert_eq!(stored.config.non_streaming_timeout, 42);
    assert!(!stored.rectifier_config.thinking_budget);
    assert_eq!(
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='unrelated'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "untouched"
    );
    let again = commit(&mut conn, stored).unwrap();
    assert_eq!(saved.revision, again.revision);
}

#[test]
fn edits_to_either_saved_setting_reject_a_stale_editor_without_overwrite() {
    for key in [OPTIMIZER_CONFIG_SETTINGS_KEY, RECTIFIER_CONFIG_SETTINGS_KEY] {
        let mut conn = database();
        let draft = changed(&conn);
        let value = if key == OPTIMIZER_CONFIG_SETTINGS_KEY {
            serde_json::to_string(&OptimizerConfig::default()).unwrap()
        } else {
            serde_json::to_string(&RectifierConfig::default()).unwrap()
        };
        put(&conn, key, &value);
        let before = read_raw(&conn).unwrap();
        assert!(commit(&mut conn, draft).unwrap_err().contains("changed"));
        assert_eq!(read_raw(&conn).unwrap(), before);
        assert!(conn.is_autocommit());
    }
}

#[test]
fn validation_and_corruption_never_change_existing_settings() {
    let mut conn = database();
    let mut draft = changed(&conn);
    draft.config.non_streaming_timeout = 86_401;
    assert!(commit(&mut conn, draft).unwrap_err().contains("86400"));
    assert_eq!(read_raw(&conn).unwrap(), (None, None));
    let draft = changed(&conn);
    put(
        &conn,
        RECTIFIER_CONFIG_SETTINGS_KEY,
        "fixture-private-error",
    );
    let before = read_raw(&conn).unwrap();
    assert_eq!(commit(&mut conn, draft).unwrap_err(), READ_ERROR);
    assert_eq!(read_raw(&conn).unwrap(), before);
}

#[test]
fn failure_or_ignored_second_write_rolls_back_the_first_setting() {
    for action in ["RAISE(ABORT,'fixture-private-error')", "RAISE(IGNORE)"] {
        let mut conn = database();
        let draft = changed(&conn);
        conn.execute_batch(&format!("CREATE TRIGGER reject_rectifier BEFORE INSERT ON app_settings WHEN NEW.key='{RECTIFIER_CONFIG_SETTINGS_KEY}' BEGIN SELECT {action}; END;")).unwrap();
        let before = read_raw(&conn).unwrap();
        assert_eq!(commit(&mut conn, draft).unwrap_err(), SAVE_ERROR);
        assert_eq!(read_raw(&conn).unwrap(), before);
        assert!(conn.is_autocommit());
    }
}

#[test]
fn silent_rewrite_during_save_is_not_reported_as_success() {
    let mut conn = database();
    let draft = changed(&conn);
    conn.execute_batch(&format!("CREATE TRIGGER rewrite_rectifier AFTER INSERT ON app_settings WHEN NEW.key='{RECTIFIER_CONFIG_SETTINGS_KEY}' BEGIN UPDATE app_settings SET value='fixture-private-error' WHERE key=NEW.key; END;")).unwrap();
    assert_eq!(commit(&mut conn, draft).unwrap_err(), SAVE_ERROR);
    assert_eq!(read_raw(&conn).unwrap(), (None, None));
}

#[test]
fn public_commands_and_runtime_cache_follow_only_committed_settings() {
    use crate::db::DbState;
    use crate::provider_proxy::{LocalProviderProxyRuntime, LocalProviderProxyRuntimeInner};
    use std::sync::{Arc, Mutex};
    use tauri::Manager;
    let app = tauri::test::mock_builder()
        .manage(DbState(Mutex::new(database())))
        .manage(LocalProviderProxyRuntime(Arc::new(Mutex::new(
            LocalProviderProxyRuntimeInner::default(),
        ))))
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let draft = super::super::get_proxy_advanced_config(app.state()).unwrap();
    let mut config = draft.config.clone();
    config.non_streaming_timeout = 42;
    let revision = super::super::set_proxy_advanced_config(
        app.handle().clone(),
        app.state(),
        config,
        draft.rectifier_config.clone(),
        draft.revision.clone(),
    )
    .unwrap();
    assert_eq!(
        super::super::get_proxy_advanced_config(app.state())
            .unwrap()
            .revision,
        revision
    );
    assert_eq!(
        app.state::<LocalProviderProxyRuntime>()
            .0
            .lock()
            .unwrap()
            .optimizer_config
            .as_ref()
            .unwrap()
            .non_streaming_timeout,
        42
    );
    assert!(super::super::set_proxy_advanced_config(
        app.handle().clone(),
        app.state(),
        draft.config,
        draft.rectifier_config,
        draft.revision
    )
    .is_err());
    assert_eq!(
        app.state::<LocalProviderProxyRuntime>()
            .0
            .lock()
            .unwrap()
            .optimizer_config
            .as_ref()
            .unwrap()
            .non_streaming_timeout,
        42
    );
    let db = app.state::<DbState>();
    put(
        &db.0.lock().unwrap(),
        RECTIFIER_CONFIG_SETTINGS_KEY,
        "private-error",
    );
    assert_eq!(
        super::super::get_rectifier_config(app.state()).unwrap_err(),
        READ_ERROR
    );
}

#[test]
fn editing_supported_controls_preserves_unknown_saved_fields() {
    let mut conn = database();
    let mut config = serde_json::to_value(OptimizerConfig::default()).unwrap();
    let mut rectifier = serde_json::to_value(RectifierConfig::default()).unwrap();
    let extra = serde_json::json!({"nested": ["opaque-user-value", 7, null], "enabled": true});
    config["futureOptimizerSetting"] = extra.clone();
    rectifier["futureRectifierSetting"] = extra.clone();
    put(&conn, OPTIMIZER_CONFIG_SETTINGS_KEY, &config.to_string());
    put(&conn, RECTIFIER_CONFIG_SETTINGS_KEY, &rectifier.to_string());
    let draft = changed(&conn);
    let saved = commit(&mut conn, draft).unwrap();
    let raw = read_raw(&conn).unwrap();
    let config: serde_json::Value = serde_json::from_str(raw.0.as_deref().unwrap()).unwrap();
    let rectifier: serde_json::Value = serde_json::from_str(raw.1.as_deref().unwrap()).unwrap();
    assert_eq!(config["futureOptimizerSetting"], extra);
    assert_eq!(rectifier["futureRectifierSetting"], extra);
    assert_eq!(config["nonStreamingTimeout"], 42);
    assert_eq!(rectifier["thinkingBudget"], false);
    assert_eq!(read(&conn).unwrap().revision, saved.revision);
}
