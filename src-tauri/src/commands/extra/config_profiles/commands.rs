#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;
use tauri::State;

use crate::db::DbState;

use super::super::log_command_timing;
use super::super::types::*;
use super::*;

#[tauri::command]
pub fn sync_config_profiles(db: State<'_, DbState>) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        let imported_counts = sync_profiles_from_compatible_databases(&conn, &now)?;
        sync_live_profiles(&conn, &imported_counts, &now)?;
        Ok(())
    })();
    log_command_timing("sync_config_profiles", started_at);
    result
}

#[tauri::command]
pub fn get_config_profiles(db: State<'_, DbState>) -> Result<Vec<ConfigProfile>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_all_config_profiles_from_conn(&conn)
    })();
    log_command_timing("get_config_profiles", started_at);
    result
}

#[tauri::command]
pub fn get_provider_config_fragments(
    db: State<'_, DbState>,
) -> Result<Vec<ProviderConfigFragment>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_provider_config_fragments_from_conn(&conn)
    })();
    log_command_timing("get_provider_config_fragments", started_at);
    result
}

#[tauri::command]
pub fn save_provider_config_fragment(
    id: Option<String>,
    name: String,
    target_tools: Vec<String>,
    fields: serde_json::Value,
    db: State<'_, DbState>,
) -> Result<ProviderConfigFragment, String> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err("Fragment name is required".to_string());
    }
    if !fields.is_object() {
        return Err("Fragment fields must be a JSON object".to_string());
    }

    let normalized_tools = normalize_provider_fragment_target_tools(target_tools);
    if normalized_tools.is_empty() {
        return Err("At least one target app is required".to_string());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut fragments = read_provider_config_fragments_from_conn(&conn)?;
    let now = chrono::Utc::now().to_rfc3339();
    let next_id = id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let saved = if let Some(existing) = fragments.iter_mut().find(|fragment| fragment.id == next_id)
    {
        existing.name = trimmed_name.to_string();
        existing.target_tools = normalized_tools.clone();
        existing.fields = fields.clone();
        existing.updated_at = now.clone();
        existing.clone()
    } else {
        let fragment = ProviderConfigFragment {
            id: next_id.clone(),
            name: trimmed_name.to_string(),
            target_tools: normalized_tools.clone(),
            fields: fields.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        fragments.push(fragment.clone());
        fragment
    };

    fragments.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    set_json_app_setting(&conn, PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY, &fragments)?;
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!(
            "Saved provider config fragment {} for apps {}",
            saved.id,
            saved.target_tools.join(",")
        ),
    );

    Ok(saved)
}

#[tauri::command]
pub fn delete_provider_config_fragment(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut fragments = read_provider_config_fragments_from_conn(&conn)?;
    let initial_len = fragments.len();
    fragments.retain(|fragment| fragment.id != id);
    if fragments.len() == initial_len {
        return Err("Provider fragment not found".to_string());
    }

    set_json_app_setting(&conn, PROVIDER_CONFIG_FRAGMENTS_SETTING_KEY, &fragments)?;
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!("Deleted provider config fragment {id}"),
    );
    Ok(())
}

#[tauri::command]
pub fn save_config_profile(
    name: String,
    tool_id: String,
    config_snapshot: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let next_sort_order = next_profile_sort_order(&conn, &tool_id);

    conn.execute(
        "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'manual', NULL, ?6, ?6)",
        rusqlite::params![id, name, tool_id, config_snapshot, next_sort_order, now],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn save_shared_config_profiles(
    name: String,
    profiles: Vec<SharedConfigProfileInput>,
    group_key: Option<String>,
    replace_profile_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    if profiles.is_empty() {
        return Err("At least one target tool is required".to_string());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let shared_group_key = group_key.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut existing_by_tool: HashMap<String, (String, Option<String>)> = HashMap::new();
    let mut stale_manual_replace: Option<(String, String)> = None;

    {
        let mut stmt = conn
            .prepare(
                "SELECT id, tool_id, source_type
                 FROM config_profiles
                 WHERE source_type = 'shared' AND source_key = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![&shared_group_key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (id, tool_id, source_type) = row.map_err(|e| e.to_string())?;
            existing_by_tool.insert(tool_id, (id, source_type));
        }
    }

    if let Some(profile_id) = replace_profile_id.as_ref() {
        let existing = conn
            .query_row(
                "SELECT tool_id, source_type
                 FROM config_profiles
                 WHERE id = ?1",
                rusqlite::params![profile_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .ok();

        if let Some((tool_id, source_type)) = existing {
            if source_type.as_deref() != Some("shared") && !existing_by_tool.contains_key(&tool_id)
            {
                if profiles.iter().any(|item| item.tool_id == tool_id) {
                    existing_by_tool.insert(tool_id, (profile_id.clone(), source_type));
                } else {
                    stale_manual_replace = Some((tool_id, profile_id.clone()));
                }
            }
        }
    }

    for profile in &profiles {
        if let Some((existing_id, _)) = existing_by_tool.remove(&profile.tool_id) {
            conn.execute(
                "UPDATE config_profiles
                 SET name = ?1, tool_id = ?2, config_snapshot = ?3, source_type = 'shared', source_key = ?4, updated_at = ?5
                 WHERE id = ?6",
                rusqlite::params![
                    &name,
                    &profile.tool_id,
                    &profile.config_snapshot,
                    &shared_group_key,
                    &now,
                    &existing_id
                ],
            )
            .map_err(|e| e.to_string())?;
            apply_snapshot_if_profile_active(
                &conn,
                &existing_id,
                &profile.tool_id,
                &profile.config_snapshot,
            )?;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            let next_sort_order = next_profile_sort_order(&conn, &profile.tool_id);
            conn.execute(
                "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'shared', ?6, ?7, ?7)",
                rusqlite::params![
                    id,
                    &name,
                    &profile.tool_id,
                    &profile.config_snapshot,
                    next_sort_order,
                    &shared_group_key,
                    &now
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    if let Some((tool_id, profile_id)) = stale_manual_replace {
        delete_profile_record(&conn, &profile_id, &tool_id)?;
    }

    for (tool_id, (profile_id, _)) in existing_by_tool {
        delete_profile_record(&conn, &profile_id, &tool_id)?;
    }

    Ok(shared_group_key)
}

#[tauri::command]
pub fn update_config_profile(
    id: String,
    name: String,
    config_snapshot: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let tool_id: String = conn
        .query_row(
            "SELECT tool_id FROM config_profiles WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    conn.execute(
        "UPDATE config_profiles SET name = ?1, config_snapshot = ?2, source_type = 'manual', source_key = NULL, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![name, config_snapshot, now, id],
    )
    .map_err(|e| e.to_string())?;

    let setting_key = current_profile_setting_key(&tool_id);
    let active_profile_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
            |row| row.get(0),
        )
        .ok();

    if active_profile_id.as_deref() == Some(id.as_str()) {
        apply_tool_snapshot(&conn, &tool_id, &config_snapshot)?;
        if tool_id == "claude" {
            crate::commands::claude_extension::sync_for_profile(&conn, &config_snapshot)?;
        }
    }

    Ok(())
}

pub fn apply_config_profile_from_conn(
    conn: &rusqlite::Connection,
    id: &str,
    preserve_user_edits: bool,
) -> Result<(String, String), String> {
    let (tool_id, snapshot): (String, String) = conn
        .query_row(
            "SELECT tool_id, config_snapshot FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    apply_tool_snapshot_with_options(conn, &tool_id, &snapshot, preserve_user_edits)?;
    if tool_id == "claude" {
        crate::commands::claude_extension::sync_for_profile(conn, &snapshot)?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE config_profiles SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![current_profile_setting_key(&tool_id), id],
    )
    .map_err(|e| e.to_string())?;

    crate::db::record_activity(conn, &tool_id, "profile_switch", "success", None);
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!("Applied profile {id} for tool {tool_id}"),
    );
    Ok((tool_id, snapshot))
}

#[tauri::command]
pub fn apply_config_profile(
    id: String,
    db: State<'_, DbState>,
) -> Result<ApplyConfigProfileResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (tool_id, _) = apply_config_profile_from_conn(&conn, &id, false)?;
    let active_profile_ids = get_active_config_profile_ids_from_conn(&conn)?;
    Ok(ApplyConfigProfileResult {
        tool_id,
        profile_id: id,
        active_profile_ids,
        applied_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub fn delete_config_profile(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (tool_id, source_type): (String, Option<String>) = conn
        .query_row(
            "SELECT tool_id, source_type FROM config_profiles WHERE id = ?1",
            rusqlite::params![&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Profile not found: {}", e))?;

    if source_type.as_deref() != Some("manual") {
        return Err("Only manual profiles can be deleted".to_string());
    }

    delete_profile_record(&conn, &id, &tool_id)?;

    Ok(())
}

#[tauri::command]
pub fn delete_config_profile_group(
    source_key: String,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, tool_id
             FROM config_profiles
             WHERE source_type = 'shared' AND source_key = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![&source_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(row.map_err(|e| e.to_string())?);
    }

    if profiles.is_empty() {
        return Err("Shared profile group not found".to_string());
    }

    for (profile_id, tool_id) in &profiles {
        delete_profile_record(&conn, profile_id, tool_id)?;
    }

    Ok(profiles.len())
}

#[tauri::command]
pub fn reorder_config_profiles(
    tool_id: String,
    ordered_ids: Vec<String>,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    for (index, profile_id) in ordered_ids.iter().enumerate() {
        let belongs_to_tool: Option<String> = conn
            .query_row(
                "SELECT tool_id FROM config_profiles WHERE id = ?1",
                rusqlite::params![profile_id],
                |row| row.get(0),
            )
            .ok();

        if belongs_to_tool.as_deref() != Some(tool_id.as_str()) {
            return Err(format!(
                "Profile does not belong to tool {tool_id}: {profile_id}"
            ));
        }

        conn.execute(
            "UPDATE config_profiles SET sort_order = ?1 WHERE id = ?2",
            rusqlite::params![index as i64, profile_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_active_config_profile_ids(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        get_active_config_profile_ids_from_conn(&conn)
    })();
    log_command_timing("get_active_config_profile_ids", started_at);
    result
}

#[tauri::command]
pub fn refresh_tray_provider_menu(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::tray::refresh_menu(&app_handle).map_err(|e| e.to_string())
}

pub fn parse_toml_assignment(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(key) {
            return None;
        }
        let (_, raw_value) = trimmed.split_once('=')?;
        let value = raw_value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

pub fn parse_toml_section_assignment(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed.trim_matches(['[', ']']) == section;
            continue;
        }
        if !in_section || !trimmed.starts_with(key) {
            continue;
        }
        let (_, raw_value) = trimmed.split_once('=')?;
        let value = raw_value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            return None;
        }
        return Some(value.to_string());
    }
    None
}
