use tauri::{Manager, State};

use crate::commands::extra_commands::WindowPreferences;
use crate::db::DbState;
use crate::provider_proxy::{
    get_local_provider_proxy_settings, get_local_provider_proxy_status,
    set_local_provider_proxy_settings, LocalProviderProxyStatus,
};

const APP_CONFIG_OVERRIDE_KEY: &str = "app_config_dir_override";
const THEME_KEY: &str = "window_theme";

fn read_preferences(db: &State<'_, DbState>) -> Result<WindowPreferences, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(crate::commands::extra_commands::read_window_preferences_from_conn(&conn))
}

fn save_preferences(
    db: &State<'_, DbState>,
    preferences: &WindowPreferences,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let value = serde_json::to_string(preferences).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('window_preferences', ?1)",
        rusqlite::params![value],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_auto_launch_status(db: State<'_, DbState>) -> Result<bool, String> {
    Ok(read_preferences(&db)?.launch_at_login)
}

#[tauri::command]
pub fn set_auto_launch(enabled: bool, db: State<'_, DbState>) -> Result<bool, String> {
    let mut preferences = read_preferences(&db)?;
    preferences.launch_at_login = enabled;
    crate::commands::extra_commands::sync_launch_at_login(enabled)?;
    save_preferences(&db, &preferences)?;
    Ok(enabled)
}

#[tauri::command]
pub fn is_lightweight_mode(db: State<'_, DbState>) -> Result<bool, String> {
    Ok(read_preferences(&db)?.lightweight_mode)
}

#[tauri::command]
pub fn enter_lightweight_mode(
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let mut preferences = read_preferences(&db)?;
    preferences.lightweight_mode = true;
    save_preferences(&db, &preferences)?;
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(true)
}

#[tauri::command]
pub fn exit_lightweight_mode(db: State<'_, DbState>) -> Result<bool, String> {
    let mut preferences = read_preferences(&db)?;
    preferences.lightweight_mode = false;
    save_preferences(&db, &preferences)?;
    Ok(true)
}

#[tauri::command]
pub fn is_portable_mode() -> bool {
    std::env::var_os("CCHUB_PORTABLE").is_some()
        || std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("portable.marker")))
            .is_some_and(|marker| marker.exists())
}

#[tauri::command]
pub fn set_window_theme(theme: String, db: State<'_, DbState>) -> Result<String, String> {
    let value = theme.trim().to_ascii_lowercase();
    if !matches!(value.as_str(), "light" | "dark" | "system") {
        return Err("Theme must be light, dark, or system".to_string());
    }
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![THEME_KEY, value],
    )
    .map_err(|error| error.to_string())?;
    Ok(value)
}

#[tauri::command]
pub fn get_app_config_dir_override(db: State<'_, DbState>) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![APP_CONFIG_OVERRIDE_KEY],
            |row| row.get::<_, String>(0),
        )
        .ok())
}

#[tauri::command]
pub fn set_app_config_dir_override(
    path: Option<String>,
    db: State<'_, DbState>,
) -> Result<Option<String>, String> {
    let normalized = path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(value) = &normalized {
        let path = std::path::Path::new(value);
        if !path.is_absolute() {
            return Err("App config override must be an absolute path".to_string());
        }
        std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
    }
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    match &normalized {
        Some(value) => conn
            .execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
                rusqlite::params![APP_CONFIG_OVERRIDE_KEY, value],
            )
            .map_err(|error| error.to_string())?,
        None => conn
            .execute(
                "DELETE FROM app_settings WHERE key = ?1",
                rusqlite::params![APP_CONFIG_OVERRIDE_KEY],
            )
            .map_err(|error| error.to_string())?,
    };
    Ok(normalized)
}

#[tauri::command]
pub fn set_proxy_takeover_for_app(
    app_type: String,
    enabled: bool,
    app_handle: tauri::AppHandle,
    db: State<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    let app = app_type.trim();
    if !crate::commands::extra_commands::MANAGED_APP_IDS.contains(&app) {
        return Err(format!("Unsupported app: {app}"));
    }
    let mut settings = get_local_provider_proxy_settings(db.clone())?;
    settings.enabled_apps.retain(|item| item != app);
    if enabled {
        settings.enabled_apps.push(app.to_string());
    }
    set_local_provider_proxy_settings(settings, app_handle.clone(), db)?;
    get_local_provider_proxy_status(app_handle.clone(), app_handle.state::<DbState>())
}

#[tauri::command]
pub fn get_default_cost_multiplier(
    app_type: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let key = format!("default_cost_multiplier:{}", app_type.trim());
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "1".to_string()))
}

#[tauri::command]
pub fn set_default_cost_multiplier(
    app_type: String,
    value: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let parsed = value
        .trim()
        .parse::<f64>()
        .map_err(|_| "Cost multiplier must be numeric".to_string())?;
    if !parsed.is_finite() || parsed < 0.0 || parsed > 100.0 {
        return Err("Cost multiplier must be between 0 and 100".to_string());
    }
    let normalized = format!("{parsed:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    let key = format!("default_cost_multiplier:{}", app_type.trim());
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, &normalized],
    )
    .map_err(|error| error.to_string())?;
    Ok(normalized)
}

#[tauri::command]
pub fn get_pricing_model_source(
    app_type: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let key = format!("pricing_model_source:{}", app_type.trim());
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "local".to_string()))
}

#[tauri::command]
pub fn set_pricing_model_source(
    app_type: String,
    source: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let value = source.trim().to_ascii_lowercase();
    if !matches!(value.as_str(), "local" | "provider" | "remote") {
        return Err("Pricing source must be local, provider, or remote".to_string());
    }
    let key = format!("pricing_model_source:{}", app_type.trim());
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, &value],
    )
    .map_err(|error| error.to_string())?;
    Ok(value)
}
