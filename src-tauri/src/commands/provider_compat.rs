use serde_json::{json, Value};
use tauri::State;

use crate::commands::extra_commands::{
    current_profile_setting_key, next_profile_sort_order, read_all_config_profiles_from_conn,
    ConfigProfile,
};
use crate::db::DbState;

fn app_id(value: &str) -> Result<&str, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "claude" => Ok("claude"),
        "codex" => Ok("codex"),
        "gemini" => Ok("gemini"),
        "grokbuild" | "grok-build" | "grok" => Ok("grokbuild"),
        "opencode" => Ok("opencode"),
        "openclaw" => Ok("openclaw"),
        "hermes" => Ok("hermes"),
        "pi" => Ok("pi"),
        _ => Err(format!("Unsupported app: {value}")),
    }
}

fn normalize_custom_endpoint(url: &str) -> Result<String, String> {
    let value = url.trim();
    if value.is_empty() || value.len() > 2048 {
        return Err("Endpoint URL must be between 1 and 2048 characters".to_string());
    }
    let parsed =
        reqwest::Url::parse(value).map_err(|error| format!("Invalid endpoint URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Only HTTP(S) endpoint URLs with a host are supported".to_string());
    }
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

fn custom_endpoint_url(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| value.get("url").and_then(Value::as_str))
        .and_then(|url| normalize_custom_endpoint(url).ok())
}

/// Ensure the optional Grok-compatible official profile exists. The profile
/// is data-only because this build does not own a separate Grok CLI config
/// file; authentication is handled by the xAI account integration.
#[tauri::command]
pub fn ensure_grokbuild_official_provider(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let id = "grokbuild-official";
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM config_profiles
             WHERE tool_id = 'grokbuild' AND (id = ?1 OR name = 'Grok Official')",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if exists > 0 {
        return Ok(true);
    }
    let now = chrono::Utc::now().to_rfc3339();
    let snapshot = serde_json::json!({
        "baseUrl": "https://api.x.ai/v1",
        "api": "openai-responses",
        "providerType": "xai_oauth",
        "metadata": {
            "category": "official",
            "websiteUrl": "https://x.ai/",
        }
    })
    .to_string();
    conn.execute(
        "INSERT INTO config_profiles (
            id, name, tool_id, config_snapshot, sort_order, source_type,
            source_key, created_at, updated_at
        ) VALUES (?1, 'Grok Official', 'grokbuild', ?2, ?3, 'official', NULL, ?4, ?4)",
        rusqlite::params![
            id,
            snapshot,
            next_profile_sort_order(&conn, "grokbuild"),
            now
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(true)
}

fn snapshot_from_provider(provider: &Value) -> Result<String, String> {
    if let Some(snapshot) = provider.get("configSnapshot").and_then(Value::as_str) {
        serde_json::from_str::<Value>(snapshot).map_err(|error| error.to_string())?;
        return Ok(snapshot.to_string());
    }
    if let Some(settings) = provider.get("settings") {
        return serde_json::to_string_pretty(settings).map_err(|error| error.to_string());
    }
    let mut snapshot = provider.clone();
    if let Some(object) = snapshot.as_object_mut() {
        for key in ["id", "name", "description", "category", "sortOrder"] {
            object.remove(key);
        }
    }
    serde_json::to_string_pretty(&snapshot).map_err(|error| error.to_string())
}

fn profile_to_provider(profile: &ConfigProfile) -> Value {
    let settings = serde_json::from_str::<Value>(&profile.config_snapshot)
        .unwrap_or_else(|_| json!({"raw": profile.config_snapshot}));
    json!({
        "id": profile.id,
        "name": profile.name,
        "settings": settings,
        "configSnapshot": profile.config_snapshot,
        "sortOrder": profile.sort_order,
        "sourceType": profile.source_type,
        "sourceKey": profile.source_key,
        "createdAt": profile.created_at,
        "updatedAt": profile.updated_at,
    })
}

#[tauri::command]
pub fn get_providers(app: String, db: State<'_, DbState>) -> Result<Vec<Value>, String> {
    let tool = app_id(&app)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(read_all_config_profiles_from_conn(&conn)?
        .iter()
        .filter(|profile| profile.tool_id == tool)
        .map(profile_to_provider)
        .collect())
}

#[tauri::command(rename_all = "camelCase")]
pub fn add_provider(
    app: String,
    provider: Value,
    add_to_live: Option<bool>,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let tool = app_id(&app)?;
    let name = provider
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or("Provider name is required")?;
    let snapshot = snapshot_from_provider(&provider)?;
    let id = provider
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = chrono::Utc::now().to_rfc3339();
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'manual', NULL, ?6, ?6)",
        rusqlite::params![&id, name, tool, snapshot, next_profile_sort_order(&conn, tool), &now],
    )
    .map_err(|error| error.to_string())?;
    if add_to_live.unwrap_or(false) {
        let profile = read_all_config_profiles_from_conn(&conn)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or("Provider was created but could not be loaded")?;
        apply_config_profile_from_conn(&conn, &profile)?;
    }
    Ok(true)
}

fn apply_config_profile_from_conn(
    conn: &rusqlite::Connection,
    profile: &ConfigProfile,
) -> Result<(), String> {
    crate::commands::extra_commands::apply_tool_snapshot(
        conn,
        &profile.tool_id,
        &profile.config_snapshot,
    )
}

#[tauri::command(rename_all = "camelCase")]
pub fn update_provider(
    app: String,
    provider: Value,
    original_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let tool = app_id(&app)?;
    let id = original_id
        .or_else(|| {
            provider
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .ok_or("Provider id is required")?;
    let name = provider
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or("Provider name is required")?;
    let snapshot = snapshot_from_provider(&provider)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let belongs: Option<String> = conn
        .query_row(
            "SELECT tool_id FROM config_profiles WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        )
        .ok();
    if belongs.as_deref() != Some(tool) {
        return Err(format!("Provider does not belong to {tool}: {id}"));
    }
    conn.execute(
        "UPDATE config_profiles SET name = ?1, config_snapshot = ?2, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![name, snapshot, chrono::Utc::now().to_rfc3339(), &id],
    )
    .map_err(|error| error.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn delete_provider(app: String, id: String, db: State<'_, DbState>) -> Result<bool, String> {
    let tool = app_id(&app)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let belongs: Option<String> = conn
        .query_row(
            "SELECT tool_id FROM config_profiles WHERE id = ?1",
            rusqlite::params![&id],
            |row| row.get(0),
        )
        .ok();
    if belongs.as_deref() != Some(tool) {
        return Ok(false);
    }
    conn.execute(
        "DELETE FROM config_profiles WHERE id = ?1",
        rusqlite::params![&id],
    )
    .map_err(|error| error.to_string())?;
    let setting_key = current_profile_setting_key(tool);
    let active_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![&setting_key],
            |row| row.get(0),
        )
        .ok();
    if active_id.as_deref() == Some(id.as_str()) {
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            rusqlite::params![setting_key],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(true)
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_custom_endpoints(
    app: String,
    provider_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<Value>, String> {
    let tool = app_id(&app)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let snapshot: String = conn
        .query_row(
            "SELECT config_snapshot FROM config_profiles WHERE id = ?1 AND tool_id = ?2",
            rusqlite::params![provider_id.trim(), tool],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_str(&snapshot).map_err(|error| error.to_string())?;
    Ok(value
        .get("customEndpoints")
        .or_else(|| value.get("custom_endpoints"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

#[tauri::command(rename_all = "camelCase")]
pub fn add_custom_endpoint(
    app: String,
    provider_id: String,
    url: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    mutate_custom_endpoint(app, provider_id, url, true, db)
}

#[tauri::command(rename_all = "camelCase")]
pub fn remove_custom_endpoint(
    app: String,
    provider_id: String,
    url: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    mutate_custom_endpoint(app, provider_id, url, false, db)
}

fn mutate_custom_endpoint(
    app: String,
    provider_id: String,
    url: String,
    add: bool,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let tool = app_id(&app)?;
    let normalized = normalize_custom_endpoint(&url)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let (name, snapshot): (String, String) = conn
        .query_row(
            "SELECT name, config_snapshot FROM config_profiles WHERE id = ?1 AND tool_id = ?2",
            rusqlite::params![provider_id.trim(), tool],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let mut value: Value = serde_json::from_str(&snapshot).map_err(|error| error.to_string())?;
    let object = value
        .as_object_mut()
        .ok_or("Provider snapshot must be a JSON object")?;
    let key = if object.contains_key("customEndpoints") {
        "customEndpoints"
    } else {
        "custom_endpoints"
    };
    let endpoints = object
        .entry(key)
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or("Provider custom endpoints must be an array")?;
    if add && endpoints.len() >= 128 {
        return Err("A profile can contain at most 128 custom endpoints".to_string());
    }
    if add {
        if !endpoints
            .iter()
            .any(|item| custom_endpoint_url(item).as_deref() == Some(&normalized))
        {
            endpoints.push(Value::String(normalized));
        }
    } else {
        endpoints.retain(|item| custom_endpoint_url(item).as_deref() != Some(&normalized));
    }
    let next = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE config_profiles SET name = ?1, config_snapshot = ?2, updated_at = ?3 WHERE id = ?4 AND tool_id = ?5",
        rusqlite::params![name, next, chrono::Utc::now().to_rfc3339(), provider_id.trim(), tool],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{app_id, normalize_custom_endpoint, snapshot_from_provider};
    use serde_json::json;

    #[test]
    fn validates_supported_provider_apps_case_insensitively() {
        assert_eq!(
            app_id(" Claude ").expect("claude should be supported"),
            "claude"
        );
        assert!(app_id("unknown").is_err());
    }

    #[test]
    fn prefers_explicit_settings_snapshot() {
        let snapshot = snapshot_from_provider(&json!({
            "id": "p1",
            "name": "Example",
            "settings": {"env": {"TOKEN": "secret"}}
        }))
        .expect("settings should serialize");
        assert!(snapshot.contains("TOKEN"));
        assert!(!snapshot.contains("\"name\""));
    }

    #[test]
    fn rejects_invalid_explicit_snapshot() {
        assert!(snapshot_from_provider(&json!({"configSnapshot": "not-json"})).is_err());
    }

    #[test]
    fn custom_endpoint_validation_is_strict_and_normalized() {
        assert_eq!(
            normalize_custom_endpoint(" https://api.example.test/// ").unwrap(),
            "https://api.example.test"
        );
        assert!(normalize_custom_endpoint("file:///tmp/config").is_err());
        assert!(normalize_custom_endpoint("https://").is_err());
    }
}
