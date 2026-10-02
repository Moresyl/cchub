mod claude;
mod codex;
mod storage;

use crate::db::DbState;
use tauri::State;

pub use claude::ClaudeSettings;
pub use codex::CodexSettings;

#[tauri::command]
pub fn get_claude_settings(db: State<'_, DbState>) -> Result<ClaudeSettings, String> {
    let conn =
        db.0.lock()
            .map_err(|_| "Settings database is unavailable")?;
    claude::read(&storage::config_path(&conn, "claude")?)
}

#[tauri::command]
pub fn set_claude_setting(
    key: String,
    value: String,
    expected_revision: String,
    db: State<'_, DbState>,
) -> Result<ClaudeSettings, String> {
    let conn =
        db.0.lock()
            .map_err(|_| "Settings database is unavailable")?;
    claude::write(
        &storage::config_path(&conn, "claude")?,
        &key,
        &value,
        &expected_revision,
    )
}

#[tauri::command]
pub fn get_codex_settings(db: State<'_, DbState>) -> Result<CodexSettings, String> {
    let conn =
        db.0.lock()
            .map_err(|_| "Settings database is unavailable")?;
    codex::read(&storage::config_path(&conn, "codex")?)
}

#[tauri::command]
pub fn set_codex_setting(
    key: String,
    value: String,
    expected_revision: Option<String>,
    db: State<'_, DbState>,
) -> Result<CodexSettings, String> {
    let conn =
        db.0.lock()
            .map_err(|_| "Settings database is unavailable")?;
    codex::write(
        &storage::config_path(&conn, "codex")?,
        &key,
        &value,
        expected_revision.as_deref(),
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod claude_tests;
