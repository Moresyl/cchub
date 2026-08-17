use tauri::State;

use crate::commands::extra_commands::ensure_official_config_profiles_seeded;
use crate::db::DbState;

/// Seed the complete built-in profile set on demand. Existing user profiles
/// are preserved; the app argument remains for clients that seed one tool at a
/// time in older versions.
#[tauri::command]
pub fn import_default_config(app: String, db: State<'_, DbState>) -> Result<bool, String> {
    let app = app.trim().to_ascii_lowercase();
    if !matches!(
        app.as_str(),
        "claude" | "codex" | "gemini" | "grokbuild" | "opencode" | "openclaw" | "hermes" | "pi"
    ) {
        return Err(format!("Unsupported app: {app}"));
    }
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    ensure_official_config_profiles_seeded(&conn)?;
    Ok(true)
}
