#![allow(clippy::too_many_arguments)]

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::extra_commands::{
    apply_snapshot_if_profile_active, delete_profile_record, get_json_app_setting,
    next_profile_sort_order, set_json_app_setting, MANAGED_APP_IDS,
};
use crate::db::DbState;

const SETTING_KEY: &str = "universal_providers";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniversalProvider {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub provider_type: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub configs: HashMap<String, String>,
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UniversalProviderSyncResult {
    pub id: String,
    pub source_key: String,
    pub synced_apps: Vec<String>,
    pub removed_profiles: usize,
    pub synced_at: String,
}

fn normalize_apps(apps: &[String]) -> Vec<String> {
    let allowed: HashSet<&str> = MANAGED_APP_IDS.iter().copied().collect();
    let mut seen = HashSet::new();
    apps.iter()
        .map(|app| app.trim().to_lowercase())
        .filter(|app| allowed.contains(app.as_str()) && seen.insert(app.clone()))
        .collect()
}

fn normalize_provider(mut provider: UniversalProvider) -> Result<UniversalProvider, String> {
    provider.id = provider.id.trim().to_string();
    provider.name = provider.name.trim().to_string();
    provider.provider_type = provider.provider_type.trim().to_string();
    provider.base_url = provider.base_url.trim().trim_end_matches('/').to_string();
    provider.api_key = provider.api_key.trim().to_string();

    if provider.id.is_empty() {
        provider.id = uuid::Uuid::new_v4().to_string();
    }
    if provider.name.is_empty() {
        return Err("Provider name is required".to_string());
    }
    if provider.name.len() > 160 {
        return Err("Provider name is too long".to_string());
    }
    if provider.base_url.is_empty() {
        return Err("Provider base URL is required".to_string());
    }
    let parsed = reqwest::Url::parse(&provider.base_url)
        .map_err(|_| "Provider base URL must be a valid http(s) URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Provider base URL must be a valid http(s) URL".to_string());
    }

    provider.apps = normalize_apps(&provider.apps);
    if provider.apps.is_empty() {
        return Err("At least one target app is required".to_string());
    }

    let selected_apps = provider.apps.clone();
    provider.configs.retain(|app, snapshot| {
        selected_apps.iter().any(|target| target == app)
            && serde_json::from_str::<serde_json::Value>(snapshot).is_ok()
    });
    let missing = provider
        .apps
        .iter()
        .filter(|app| !provider.configs.contains_key(*app))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "Missing configuration snapshot for: {}",
            missing.join(", ")
        ));
    }

    Ok(provider)
}

fn read_providers(conn: &rusqlite::Connection) -> Result<Vec<UniversalProvider>, String> {
    let mut providers =
        get_json_app_setting::<Vec<UniversalProvider>>(conn, SETTING_KEY)?.unwrap_or_default();
    providers = providers
        .into_iter()
        .filter_map(|provider| normalize_provider(provider).ok())
        .collect();
    providers.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(providers)
}

fn write_providers(
    conn: &rusqlite::Connection,
    providers: &[UniversalProvider],
) -> Result<(), String> {
    set_json_app_setting(conn, SETTING_KEY, &providers.to_vec())
}

#[tauri::command]
pub fn get_universal_providers(db: State<'_, DbState>) -> Result<Vec<UniversalProvider>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    read_providers(&conn)
}

#[tauri::command]
pub fn get_universal_provider(
    id: String,
    db: State<'_, DbState>,
) -> Result<Option<UniversalProvider>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    Ok(read_providers(&conn)?
        .into_iter()
        .find(|provider| provider.id == id.trim()))
}

#[tauri::command]
pub fn upsert_universal_provider(
    provider: UniversalProvider,
    db: State<'_, DbState>,
) -> Result<UniversalProvider, String> {
    let mut provider = normalize_provider(provider)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut providers = read_providers(&conn)?;
    let now = chrono::Utc::now().to_rfc3339();
    if let Some(existing) = providers.iter().find(|item| item.id == provider.id) {
        provider.created_at = existing.created_at.clone();
    } else {
        provider.created_at = Some(now.clone());
    }
    provider.updated_at = Some(now);

    if let Some(existing) = providers.iter_mut().find(|item| item.id == provider.id) {
        *existing = provider.clone();
    } else {
        providers.push(provider.clone());
    }
    write_providers(&conn, &providers)?;
    Ok(provider)
}

#[tauri::command]
pub fn delete_universal_provider(id: String, db: State<'_, DbState>) -> Result<bool, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Provider id is required".to_string());
    }
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut providers = read_providers(&conn)?;
    let initial_len = providers.len();
    providers.retain(|provider| provider.id != id);
    if providers.len() == initial_len {
        return Ok(false);
    }
    let source_key = format!("universal:{id}");
    let linked_profiles = {
        let mut statement = conn
            .prepare(
                "SELECT id, tool_id FROM config_profiles WHERE source_type = 'shared' AND source_key = ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(rusqlite::params![&source_key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        rows.filter_map(Result::ok).collect::<Vec<_>>()
    };
    for (profile_id, tool_id) in linked_profiles {
        delete_profile_record(&conn, &profile_id, &tool_id)?;
    }
    write_providers(&conn, &providers)?;
    Ok(true)
}

#[tauri::command]
pub fn sync_universal_provider(
    id: String,
    db: State<'_, DbState>,
) -> Result<UniversalProviderSyncResult, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let provider = read_providers(&conn)?
        .into_iter()
        .find(|item| item.id == id.trim())
        .ok_or_else(|| format!("Universal provider not found: {}", id.trim()))?;
    let source_key = format!("universal:{}", provider.id);
    let now = chrono::Utc::now().to_rfc3339();

    let mut existing = HashMap::<String, String>::new();
    {
        let mut statement = conn
            .prepare(
                "SELECT id, tool_id FROM config_profiles WHERE source_type = 'shared' AND source_key = ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(rusqlite::params![&source_key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (profile_id, tool_id) = row.map_err(|error| error.to_string())?;
            existing.insert(tool_id, profile_id);
        }
    }

    for app in &provider.apps {
        let snapshot = provider
            .configs
            .get(app)
            .ok_or_else(|| format!("Missing configuration snapshot for: {app}"))?;
        if let Some(profile_id) = existing.remove(app) {
            conn.execute(
                "UPDATE config_profiles SET name = ?1, config_snapshot = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![&provider.name, snapshot, &now, &profile_id],
            )
            .map_err(|error| error.to_string())?;
            apply_snapshot_if_profile_active(&conn, &profile_id, app, snapshot)?;
        } else {
            let profile_id = uuid::Uuid::new_v4().to_string();
            let sort_order = next_profile_sort_order(&conn, app);
            conn.execute(
                "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, sort_order, source_type, source_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'shared', ?6, ?7, ?7)",
                rusqlite::params![
                    &profile_id,
                    &provider.name,
                    app,
                    snapshot,
                    sort_order,
                    &source_key,
                    &now,
                ],
            )
            .map_err(|error| error.to_string())?;
        }
    }

    let mut removed_profiles = 0;
    for (tool_id, profile_id) in existing {
        delete_profile_record(&conn, &profile_id, &tool_id)?;
        removed_profiles += 1;
    }

    let synced_apps = provider.apps.clone();
    crate::utils::append_runtime_log(
        "info",
        "profiles",
        &format!(
            "Synced universal provider {} to {} apps",
            provider.id,
            synced_apps.len()
        ),
    );
    let mut updated = provider;
    updated.updated_at = Some(now.clone());
    let mut providers = read_providers(&conn)?;
    if let Some(existing) = providers.iter_mut().find(|item| item.id == updated.id) {
        *existing = updated;
        write_providers(&conn, &providers)?;
    }

    Ok(UniversalProviderSyncResult {
        id: id.trim().to_string(),
        source_key,
        synced_apps,
        removed_profiles,
        synced_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::{normalize_apps, normalize_provider, UniversalProvider};
    use std::collections::HashMap;

    #[test]
    fn normalize_apps_deduplicates_and_filters_unknown_apps() {
        let input = vec![
            " claude ".to_string(),
            "codex".to_string(),
            "claude".to_string(),
            "unknown".to_string(),
        ];
        assert_eq!(normalize_apps(&input), vec!["claude", "codex"]);
    }

    #[test]
    fn provider_validation_requires_snapshots_for_selected_apps() {
        let provider = UniversalProvider {
            id: String::new(),
            name: "demo".to_string(),
            provider_type: "custom".to_string(),
            base_url: "https://example.com".to_string(),
            api_key: String::new(),
            apps: vec!["claude".to_string()],
            configs: HashMap::new(),
            website_url: None,
            notes: None,
            created_at: None,
            updated_at: None,
        };
        assert!(normalize_provider(provider).is_err());
    }
}
