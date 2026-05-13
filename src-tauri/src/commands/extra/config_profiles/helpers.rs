#![allow(clippy::too_many_arguments)]
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::*;

pub fn sync_launch_at_login(enabled: bool) -> Result<(), String> {
    let path = autostart_entry_path()?;

    if !enabled {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &autostart_entry_content(&exe))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn scan_environment_conflicts() -> Vec<EnvironmentConflict> {
    let env_groups = [
        (
            "claude",
            vec![
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "ANTHROPIC_BASE_URL",
                "ANTHROPIC_MODEL",
            ],
        ),
        (
            "codex",
            vec![
                "OPENAI_API_KEY",
                "OPENAI_BASE_URL",
                "OPENAI_ORG_ID",
                "OPENAI_MODEL",
            ],
        ),
        (
            "gemini",
            vec![
                "GEMINI_API_KEY",
                "GOOGLE_API_KEY",
                "GOOGLE_GEMINI_BASE_URL",
                "GEMINI_MODEL",
            ],
        ),
    ];

    let mut conflicts = Vec::new();
    let mut apps_with_overrides = Vec::new();
    let mut all_variables = Vec::new();

    for (app_id, keys) in env_groups {
        let variables: Vec<String> = keys
            .into_iter()
            .filter(|key| {
                std::env::var(key)
                    .ok()
                    .is_some_and(|value| !value.trim().is_empty())
            })
            .map(str::to_string)
            .collect();

        if variables.is_empty() {
            continue;
        }

        all_variables.extend(variables.iter().cloned());
        apps_with_overrides.push(app_id.to_string());
        conflicts.push(EnvironmentConflict {
            id: format!("{app_id}_env_override"),
            kind: "tool_override".to_string(),
            variables,
            affected_apps: vec![app_id.to_string()],
        });
    }

    if apps_with_overrides.len() >= 2 {
        conflicts.insert(
            0,
            EnvironmentConflict {
                id: "shared_env_overrides".to_string(),
                kind: "multi_tool_override".to_string(),
                variables: all_variables,
                affected_apps: apps_with_overrides,
            },
        );
    }

    conflicts
}

fn candidate_home_dirs() -> Vec<PathBuf> {
    let mut homes = Vec::new();

    if let Some(home) = dirs::home_dir() {
        homes.push(home);
    }

    for key in ["USERPROFILE", "HOME"] {
        if let Ok(value) = std::env::var(key) {
            let path = PathBuf::from(value);
            if !homes.iter().any(|item| item == &path) {
                homes.push(path);
            }
        }
    }

    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        let home = PathBuf::from(format!("{}{}", drive, path));
        if !homes.iter().any(|item| item == &home) {
            homes.push(home);
        }
    }

    #[cfg(target_family = "unix")]
    {
        let mnt_root = PathBuf::from("/mnt");
        if mnt_root.exists() {
            if let Ok(drives) = std::fs::read_dir(&mnt_root) {
                for drive in drives.flatten() {
                    let users_dir = drive.path().join("Users");
                    if !users_dir.exists() {
                        continue;
                    }
                    if let Ok(users) = std::fs::read_dir(users_dir) {
                        for user in users.flatten() {
                            let home = user.path();
                            if !homes.iter().any(|item| item == &home) {
                                homes.push(home);
                            }
                        }
                    }
                }
            }
        }
    }

    homes
}

pub fn compatible_db_paths() -> Vec<PathBuf> {
    let compat_dir = [".cc", "switch"].join("-");
    let compat_db = ["cc", "switch.db"].join("-");

    candidate_home_dirs()
        .into_iter()
        .map(|home| home.join(&compat_dir).join(&compat_db))
        .filter(|path| path.exists())
        .collect()
}

pub fn current_profile_setting_key(tool_id: &str) -> String {
    format!("current_config_profile:{}", tool_id)
}

pub fn next_profile_sort_order(conn: &rusqlite::Connection, tool_id: &str) -> i64 {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM config_profiles WHERE tool_id = ?1",
        rusqlite::params![tool_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub(crate) fn ensure_official_config_profiles_seeded(
    conn: &rusqlite::Connection,
) -> Result<(), String> {
    let seeded_at = chrono::Utc::now().to_rfc3339();
    let codex_config = [
        r#"model_provider = "custom""#,
        r#"model = "gpt-5.4""#,
        r#"model_reasoning_effort = "high""#,
        "disable_response_storage = true",
        "",
        "[model_providers.custom]",
        r#"name = "custom""#,
        r#"base_url = "https://api.openai.com/v1""#,
        r#"wire_api = "responses""#,
        "requires_openai_auth = true",
    ]
    .join("\n");

    let seeds = vec![
        (
            "claude",
            "Claude Official",
            serde_json::json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
                },
                "includeCoAuthoredBy": false,
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://www.anthropic.com/api",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "codex",
            "OpenAI Official",
            serde_json::json!({
                "auth": {},
                "config": codex_config,
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://platform.openai.com/",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "gemini",
            "Google Official",
            serde_json::json!({
                "env": {
                    "GOOGLE_GEMINI_BASE_URL": "https://generativelanguage.googleapis.com/v1beta",
                    "GEMINI_MODEL": "gemini-2.5-pro",
                },
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://ai.google.dev/",
                    "seededAt": seeded_at,
                },
                "config": {},
            })
            .to_string(),
        ),
        (
            "openclaw",
            "Anthropic Direct",
            serde_json::json!({
                "baseUrl": "https://api.anthropic.com",
                "apiKey": "",
                "api": "anthropic-messages",
                "models": [{
                    "id": "claude-sonnet-4-5",
                    "name": "claude-sonnet-4-5",
                }],
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://www.anthropic.com/api",
                    "seededAt": seeded_at,
                },
            })
            .to_string(),
        ),
        (
            "opencode",
            "OpenAI Responses",
            serde_json::json!({
                "npm": "@ai-sdk/openai",
                "name": "custom",
                "metadata": {
                    "category": "official",
                    "websiteUrl": "https://platform.openai.com/",
                    "seededAt": seeded_at,
                },
                "options": {
                    "baseURL": "https://api.openai.com/v1",
                    "apiKey": "",
                },
                "models": {
                    "gpt-5.4": {
                        "name": "gpt-5.4",
                    },
                },
            })
            .to_string(),
        ),
    ];

    for (tool_id, name, config_snapshot) in seeds {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM config_profiles WHERE tool_id = ?1 AND name = ?2",
                rusqlite::params![tool_id, name],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let next_sort_order = next_profile_sort_order(conn, tool_id);
        conn.execute(
            "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'manual', NULL, ?6, ?6)",
            rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn clear_active_profile_if_selected(
    conn: &rusqlite::Connection,
    tool_id: &str,
    profile_id: &str,
) -> Result<(), String> {
    let setting_key = current_profile_setting_key(tool_id);
    let stored_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![&setting_key],
            |row| row.get(0),
        )
        .ok();
    if stored_id.as_deref() == Some(profile_id) {
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn apply_snapshot_if_profile_active(
    conn: &rusqlite::Connection,
    profile_id: &str,
    tool_id: &str,
    config_snapshot: &str,
) -> Result<(), String> {
    let setting_key = current_profile_setting_key(tool_id);
    let active_profile_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
            |row| row.get(0),
        )
        .ok();

    if active_profile_id.as_deref() == Some(profile_id) {
        apply_tool_snapshot(conn, tool_id, config_snapshot)?;
    }

    Ok(())
}

pub fn delete_profile_record(
    conn: &rusqlite::Connection,
    profile_id: &str,
    tool_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM config_profiles WHERE id = ?1",
        rusqlite::params![profile_id],
    )
    .map_err(|e| e.to_string())?;
    clear_active_profile_if_selected(conn, tool_id, profile_id)?;
    Ok(())
}

fn get_stored_current_profile_ids(
    conn: &rusqlite::Connection,
) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM app_settings WHERE key LIKE 'current_config_profile:%'")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut current = HashMap::new();
    for row in rows {
        let (key, value) = row.map_err(|e| e.to_string())?;
        if let Some(tool_id) = key.strip_prefix("current_config_profile:") {
            current.insert(tool_id.to_string(), value);
        }
    }

    Ok(current)
}

pub fn normalize_provider_fragment_target_tools(target_tools: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    target_tools
        .into_iter()
        .map(|tool_id| tool_id.trim().to_string())
        .filter(|tool_id| !tool_id.is_empty())
        .filter(|tool_id| MANAGED_APP_IDS.contains(&tool_id.as_str()))
        .filter(|tool_id| seen.insert(tool_id.clone()))
        .collect()
}

pub fn read_provider_config_fragments_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<ProviderConfigFragment>, String> {
    let mut fragments = get_json_app_setting::<Vec<ProviderConfigFragment>>(
        conn,
        PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY,
    )?
    .unwrap_or_default();

    for fragment in &mut fragments {
        fragment.name = fragment.name.trim().to_string();
        fragment.target_tools =
            normalize_provider_fragment_target_tools(fragment.target_tools.clone());
    }

    fragments.retain(|fragment| {
        !fragment.id.trim().is_empty()
            && !fragment.name.is_empty()
            && !fragment.target_tools.is_empty()
            && fragment.fields.is_object()
    });

    fragments.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(fragments)
}

fn get_compatible_current_profile_ids() -> Result<HashMap<String, String>, String> {
    let mut current = HashMap::new();

    for db_path in compatible_db_paths() {
        let external = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;

        let mut stmt = external
            .prepare(
                "SELECT id, app_type
                 FROM providers
                 WHERE is_current = 1 AND app_type IN ('claude', 'codex', 'gemini')",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (provider_id, tool_id) = row.map_err(|e| e.to_string())?;
            current.insert(
                tool_id.clone(),
                format!("compat-{}-{}", tool_id, provider_id),
            );
        }
    }

    Ok(current)
}

pub fn read_all_config_profiles_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<ConfigProfile>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, tool_id, config_snapshot, COALESCE(sort_order, 0), source_type, source_key, created_at, updated_at
             FROM config_profiles
             ORDER BY tool_id ASC, COALESCE(sort_order, 0) ASC, updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let profiles = stmt
        .query_map([], |row| {
            Ok(ConfigProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                tool_id: row.get(2)?,
                config_snapshot: row.get(3)?,
                sort_order: row.get(4)?,
                source_type: row.get(5)?,
                source_key: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    Ok(profiles)
}

pub fn get_active_config_profile_ids_from_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<String>, String> {
    let profiles = read_all_config_profiles_from_conn(conn)?;
    let mut active_ids = Vec::new();
    let stored_current = get_stored_current_profile_ids(conn)?;
    let compatible_current = get_compatible_current_profile_ids().unwrap_or_default();
    let mut cache: HashMap<String, Option<String>> = HashMap::new();
    let mut resolved_tools = std::collections::HashSet::new();

    for profile in &profiles {
        if resolved_tools.contains(&profile.tool_id) {
            continue;
        }

        let preferred_id = stored_current
            .get(&profile.tool_id)
            .or_else(|| compatible_current.get(&profile.tool_id));

        if let Some(preferred_id) = preferred_id {
            if profiles
                .iter()
                .any(|item| item.tool_id == profile.tool_id && item.id == *preferred_id)
            {
                active_ids.push(preferred_id.clone());
                resolved_tools.insert(profile.tool_id.clone());
            }
        }
    }

    for profile in profiles {
        if resolved_tools.contains(&profile.tool_id) {
            continue;
        }

        if !cache.contains_key(&profile.tool_id) {
            let content = read_tool_snapshot(conn, &profile.tool_id).ok();
            cache.insert(profile.tool_id.clone(), content);
        }

        if cache
            .get(&profile.tool_id)
            .and_then(|value| value.as_ref())
            .is_some_and(|value| config_contents_match(value, &profile.config_snapshot))
        {
            active_ids.push(profile.id);
            resolved_tools.insert(profile.tool_id.clone());
        }
    }

    Ok(active_ids)
}

pub fn read_config_profiles_for_tray(
    conn: &rusqlite::Connection,
) -> Result<Vec<ConfigProfile>, String> {
    read_all_config_profiles_from_conn(conn)
}

pub fn read_active_config_profile_ids_for_tray(
    conn: &rusqlite::Connection,
) -> Result<Vec<String>, String> {
    get_active_config_profile_ids_from_conn(conn)
}

pub fn normalize_external_profile_snapshot(tool_id: &str, settings_config: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(settings_config).ok()?;

    match tool_id {
        "claude" | "codex" | "gemini" => serde_json::to_string_pretty(&value).ok(),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn upsert_synced_profile(
    conn: &rusqlite::Connection,
    id: &str,
    name: &str,
    tool_id: &str,
    config_snapshot: &str,
    source_type: &str,
    source_key: Option<&str>,
    now: &str,
) -> Result<(), String> {
    let existing_source_type: Option<String> = conn
        .query_row(
            "SELECT source_type FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .ok();

    if existing_source_type.as_deref() == Some("manual") {
        return Ok(());
    }

    if existing_source_type.is_some() {
        conn.execute(
            "UPDATE config_profiles
             SET name = ?1, tool_id = ?2, config_snapshot = ?3, source_type = ?4, source_key = ?5, updated_at = ?6
             WHERE id = ?7",
            rusqlite::params![name, tool_id, config_snapshot, source_type, source_key, now, id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let next_sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM config_profiles WHERE tool_id = ?1",
                rusqlite::params![tool_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO config_profiles
             (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, source_type, source_key, now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}
