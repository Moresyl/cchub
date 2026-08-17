use crate::db::DbState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::State;

const MAX_PROMPT_BYTES: usize = 1024 * 1024;
const SUPPORTED_APPS: &[&str] = &[
    "claude", "codex", "gemini", "opencode", "openclaw", "hermes", "pi",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptRecord {
    pub id: String,
    pub name: String,
    pub content: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptInput {
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: bool,
}

fn normalize_app(app: &str) -> Result<String, String> {
    let normalized = app.trim().to_ascii_lowercase();
    if SUPPORTED_APPS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(format!("Unsupported prompt app: {normalized}"))
    }
}

fn validate_prompt(id: &str, prompt: &PromptInput) -> Result<(), String> {
    if id.trim().is_empty() || id.len() > 128 {
        return Err("Prompt id must contain 1 to 128 characters".to_string());
    }
    let name = prompt.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err("Prompt name must contain 1 to 120 characters".to_string());
    }
    if prompt.content.len() > MAX_PROMPT_BYTES {
        return Err("Prompt content is too large".to_string());
    }
    if prompt
        .description
        .as_deref()
        .is_some_and(|description| description.chars().count() > 2_000)
    {
        return Err("Prompt description is too long".to_string());
    }
    Ok(())
}

fn prompt_path_for_home(home: &Path, app: &str) -> Result<PathBuf, String> {
    let path = match app {
        "claude" => home.join(".claude").join("CLAUDE.md"),
        "codex" => home.join(".codex").join("AGENTS.md"),
        "gemini" => home.join(".gemini").join("GEMINI.md"),
        "opencode" => home.join(".config").join("opencode").join("AGENTS.md"),
        "openclaw" => home.join(".openclaw").join("AGENTS.md"),
        "hermes" => home.join(".hermes").join("SOUL.md"),
        "pi" => home.join(".pi").join("agent").join("AGENTS.md"),
        _ => return Err(format!("Unsupported prompt app: {app}")),
    };
    Ok(path)
}

fn prompt_path(app: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;
    prompt_path_for_home(&home, app)
}

fn write_live_prompt(app: &str, content: &str) -> Result<(), String> {
    let path = prompt_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create prompt directory {}: {error}",
                parent.display()
            )
        })?;
    }
    crate::utils::atomic_write_string(&path, content)
        .map_err(|error| format!("Failed to write prompt file {}: {error}", path.display()))
}

fn row_to_prompt(row: &rusqlite::Row<'_>) -> rusqlite::Result<PromptRecord> {
    Ok(PromptRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        content: row.get(2)?,
        description: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn load_prompts(conn: &Connection, app: &str) -> Result<BTreeMap<String, PromptRecord>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, name, content, description, enabled, created_at, updated_at
             FROM prompt_library WHERE app_id = ?1 ORDER BY updated_at DESC, id ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([app], row_to_prompt)
        .map_err(|error| error.to_string())?;
    let mut prompts = BTreeMap::new();
    for row in rows {
        let prompt = row.map_err(|error| error.to_string())?;
        prompts.insert(prompt.id.clone(), prompt);
    }
    Ok(prompts)
}

fn legacy_timestamp(value: Option<&Value>, fallback: i64) -> i64 {
    value
        .and_then(Value::as_str)
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
        .map(|date| date.timestamp_millis())
        .unwrap_or(fallback)
}

fn migrate_legacy_presets(conn: &Connection, app: &str) -> Result<(), String> {
    let has_rows: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM prompt_library WHERE app_id = ?1)",
            [app],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if has_rows {
        return Ok(());
    }
    let Some(raw) = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'prompt_presets'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
    else {
        return Ok(());
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Ok(());
    };
    let Some(presets) = value.as_array() else {
        return Ok(());
    };
    let active_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'active_prompt_preset'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let now = Utc::now().timestamp_millis();
    let transaction = conn
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    for preset in presets {
        let Some(id) = preset
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
        else {
            continue;
        };
        let Some(name) = preset
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
        else {
            continue;
        };
        let content = preset
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if content.len() > MAX_PROMPT_BYTES {
            continue;
        }
        let created_at = legacy_timestamp(preset.get("created_at"), now);
        let updated_at = legacy_timestamp(preset.get("updated_at"), created_at);
        transaction
            .execute(
                "INSERT OR IGNORE INTO prompt_library (app_id, id, name, content, description, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7)",
                params![app, id, name.trim(), content, i64::from(active_id.as_deref() == Some(id)), created_at, updated_at],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

fn save_prompt(
    conn: &mut Connection,
    app: &str,
    id: &str,
    prompt: PromptInput,
    sync_live: bool,
) -> Result<PromptRecord, String> {
    validate_prompt(id, &prompt)?;
    if prompt.enabled && sync_live {
        write_live_prompt(app, &prompt.content)?;
    }

    let now = chrono::Utc::now().timestamp_millis();
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    if prompt.enabled {
        transaction
            .execute(
                "UPDATE prompt_library SET enabled = 0, updated_at = ?2 WHERE app_id = ?1 AND enabled = 1",
                params![app, now],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction
        .execute(
            "INSERT INTO prompt_library
             (app_id, id, name, content, description, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(app_id, id) DO UPDATE SET
               name = excluded.name,
               content = excluded.content,
               description = excluded.description,
               enabled = excluded.enabled,
               updated_at = excluded.updated_at",
            params![
                app,
                id,
                prompt.name.trim(),
                prompt.content,
                prompt
                    .description
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                i64::from(prompt.enabled),
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;

    load_prompts(conn, app)?
        .remove(id)
        .ok_or_else(|| "Prompt was saved but could not be reloaded".to_string())
}

fn set_enabled(conn: &mut Connection, app: &str, id: &str, sync_live: bool) -> Result<(), String> {
    let content: Option<String> = conn
        .query_row(
            "SELECT content FROM prompt_library WHERE app_id = ?1 AND id = ?2",
            params![app, id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let content = content.ok_or_else(|| "Prompt not found".to_string())?;
    if sync_live {
        write_live_prompt(app, &content)?;
    }

    let now = chrono::Utc::now().timestamp_millis();
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE prompt_library SET enabled = 0, updated_at = ?2 WHERE app_id = ?1 AND enabled = 1",
            params![app, now],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE prompt_library SET enabled = 1, updated_at = ?3 WHERE app_id = ?1 AND id = ?2",
            params![app, id, now],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_prompts(
    app: String,
    db: State<'_, DbState>,
) -> Result<BTreeMap<String, PromptRecord>, String> {
    let app = normalize_app(&app)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    migrate_legacy_presets(&conn, &app)?;
    load_prompts(&conn, &app)
}

#[tauri::command]
pub fn upsert_prompt(
    app: String,
    id: String,
    prompt: PromptInput,
    db: State<'_, DbState>,
) -> Result<PromptRecord, String> {
    let app = normalize_app(&app)?;
    let mut conn = db.0.lock().map_err(|error| error.to_string())?;
    save_prompt(&mut conn, &app, id.trim(), prompt, true)
}

#[tauri::command]
pub fn delete_prompt(app: String, id: String, db: State<'_, DbState>) -> Result<(), String> {
    let app = normalize_app(&app)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let changed = conn
        .execute(
            "DELETE FROM prompt_library WHERE app_id = ?1 AND id = ?2",
            params![app, id.trim()],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("Prompt not found".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn enable_prompt(app: String, id: String, db: State<'_, DbState>) -> Result<(), String> {
    let app = normalize_app(&app)?;
    let mut conn = db.0.lock().map_err(|error| error.to_string())?;
    set_enabled(&mut conn, &app, id.trim(), true)
}

#[tauri::command]
pub fn get_current_prompt_file_content(app: String) -> Result<Option<String>, String> {
    let app = normalize_app(&app)?;
    let path = prompt_path(&app)?;
    if !path.is_file() {
        return Ok(None);
    }
    let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_PROMPT_BYTES as u64 {
        return Err("Prompt file is too large".to_string());
    }
    std::fs::read_to_string(&path)
        .map(Some)
        .map_err(|error| format!("Failed to read prompt file {}: {error}", path.display()))
}

#[tauri::command]
pub fn import_prompt_from_file(app: String, db: State<'_, DbState>) -> Result<String, String> {
    let app = normalize_app(&app)?;
    let path = prompt_path(&app)?;
    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("Prompt file does not exist at {}: {error}", path.display()))?;
    if metadata.len() > MAX_PROMPT_BYTES as u64 {
        return Err("Prompt file is too large".to_string());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read prompt file {}: {error}", path.display()))?;
    let mut conn = db.0.lock().map_err(|error| error.to_string())?;
    if let Some(existing_id) = conn
        .query_row(
            "SELECT id FROM prompt_library WHERE app_id = ?1 AND content = ?2 LIMIT 1",
            params![app, content],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
    {
        set_enabled(&mut conn, &app, &existing_id, false)?;
        return Ok(existing_id);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let display_name = match app.as_str() {
        "openclaw" => "OpenClaw",
        "opencode" => "OpenCode",
        "pi" => "Pi",
        _ => {
            let mut chars = app.chars();
            return save_prompt(
                &mut conn,
                &app,
                &id,
                PromptInput {
                    name: format!(
                        "Imported {} instructions",
                        chars
                            .next()
                            .map(|first| first.to_ascii_uppercase().to_string())
                            .unwrap_or_default()
                            + chars.as_str()
                    ),
                    content,
                    description: Some(format!("Imported from {}", path.display())),
                    enabled: true,
                },
                false,
            )
            .map(|_| id);
        }
    };
    save_prompt(
        &mut conn,
        &app,
        &id,
        PromptInput {
            name: format!("Imported {display_name} instructions"),
            content,
            description: Some(format!("Imported from {}", path.display())),
            enabled: true,
        },
        false,
    )?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&crate::db::schema::get_schema_sql())
            .unwrap();
        conn
    }

    fn prompt(name: &str, enabled: bool) -> PromptInput {
        PromptInput {
            name: name.to_string(),
            content: format!("# {name}"),
            description: None,
            enabled,
        }
    }

    #[test]
    fn rejects_unknown_apps_and_invalid_prompts() {
        assert!(normalize_app("unknown").is_err());
        assert!(validate_prompt("", &prompt("Valid", false)).is_err());
        assert!(validate_prompt("id", &prompt("", false)).is_err());
    }

    #[test]
    fn stores_prompts_per_app() {
        let mut conn = database();
        save_prompt(
            &mut conn,
            "claude",
            "shared",
            prompt("Claude", false),
            false,
        )
        .unwrap();
        save_prompt(&mut conn, "codex", "shared", prompt("Codex", false), false).unwrap();

        assert_eq!(
            load_prompts(&conn, "claude").unwrap()["shared"].name,
            "Claude"
        );
        assert_eq!(
            load_prompts(&conn, "codex").unwrap()["shared"].name,
            "Codex"
        );
    }

    #[test]
    fn enabling_is_exclusive_within_one_app() {
        let mut conn = database();
        save_prompt(&mut conn, "claude", "first", prompt("First", true), false).unwrap();
        save_prompt(&mut conn, "claude", "second", prompt("Second", true), false).unwrap();

        let prompts = load_prompts(&conn, "claude").unwrap();
        assert!(!prompts["first"].enabled);
        assert!(prompts["second"].enabled);
    }

    #[test]
    fn migrates_legacy_presets_once_per_app_without_overwriting_rows() {
        let conn = database();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('prompt_presets', ?1)",
            [r#"[{"id":"legacy","name":"Legacy","content":"content","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-02T00:00:00Z"}]"#],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('active_prompt_preset', 'legacy')",
            [],
        )
        .unwrap();

        migrate_legacy_presets(&conn, "claude").unwrap();
        migrate_legacy_presets(&conn, "claude").unwrap();
        let prompts = load_prompts(&conn, "claude").unwrap();
        assert_eq!(prompts.len(), 1);
        assert!(prompts["legacy"].enabled);
        assert_eq!(prompts["legacy"].updated_at, 1_767_312_000_000);
    }

    #[test]
    fn builds_expected_live_paths() {
        let home = Path::new("C:/Users/tester");
        assert!(prompt_path_for_home(home, "codex")
            .unwrap()
            .ends_with(Path::new(".codex/AGENTS.md")));
        assert!(prompt_path_for_home(home, "hermes")
            .unwrap()
            .ends_with(Path::new(".hermes/SOUL.md")));
    }
}
