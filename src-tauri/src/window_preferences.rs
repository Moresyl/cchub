use tauri::Manager;

pub fn load_window_preferences(
    app_handle: &tauri::AppHandle,
) -> crate::commands::extra_commands::WindowPreferences {
    let db = app_handle.state::<crate::db::DbState>();
    let preferences = match db.0.lock() {
        Ok(conn) => crate::commands::extra_commands::read_window_preferences_from_conn(&conn),
        Err(_) => crate::commands::extra_commands::WindowPreferences::default(),
    };
    preferences
}
