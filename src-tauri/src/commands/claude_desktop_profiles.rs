use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::db::DbState;

const SETTING_KEY: &str = "claude_desktop_direct_providers";
const GATEWAY_TOKEN_KEY: &str = "claude_desktop_gateway_token";
const KEYRING_SERVICE: &str = "com.cchub.claude-desktop-provider";
const PROFILE_ID: &str = "00000000-0000-4000-8000-0000000cc8ab";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectProvider {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) base_url: String,
    #[serde(default, skip_serializing)]
    pub(crate) api_key: String,
    pub(crate) models: Vec<String>,
    #[serde(default)]
    pub(crate) mode: DesktopMode,
    #[serde(default = "default_api_format")]
    pub(crate) api_format: String,
    #[serde(default)]
    pub(crate) model_routes: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DesktopMode {
    #[default]
    Direct,
    Proxy,
}

fn default_api_format() -> String {
    "anthropic".to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectProviderSummary {
    id: String,
    name: String,
    base_url: String,
    has_api_key: bool,
    models: Vec<String>,
    mode: DesktopMode,
    api_format: String,
    model_routes: BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProviderState {
    providers: Vec<DirectProviderSummary>,
    active_id: Option<String>,
    profile_path: String,
}

struct DesktopPaths {
    normal: PathBuf,
    threep: PathBuf,
    profile: PathBuf,
    meta: PathBuf,
}

fn paths_from_dirs(normal: PathBuf, threep: PathBuf) -> DesktopPaths {
    let library = threep.join("configLibrary");
    DesktopPaths {
        normal: normal.join("claude_desktop_config.json"),
        threep: threep.join("claude_desktop_config.json"),
        profile: library.join(format!("{PROFILE_ID}.json")),
        meta: library.join("_meta.json"),
    }
}

fn platform_paths() -> Result<DesktopPaths, String> {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join("AppData/Local")))
            .ok_or("Cannot locate local application data")?;
        Ok(paths_from_dirs(base.join("Claude"), base.join("Claude-3p")))
    }
    #[cfg(target_os = "macos")]
    {
        let base = dirs::home_dir()
            .ok_or("Cannot locate home directory")?
            .join("Library/Application Support");
        Ok(paths_from_dirs(base.join("Claude"), base.join("Claude-3p")))
    }
    #[cfg(target_os = "linux")]
    {
        let base = dirs::config_dir().ok_or("Cannot locate config directory")?;
        Ok(paths_from_dirs(base.join("Claude"), base.join("Claude-3p")))
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    Err("Claude Desktop is unsupported on this platform".to_string())
}

pub(crate) fn backup_path(key: &str) -> Result<PathBuf, String> {
    let paths = platform_paths()?;
    match key {
        "normal-config" => Ok(paths.normal),
        "threep-config" => Ok(paths.threep),
        "managed-profile" => Ok(paths.profile),
        "profile-meta" => Ok(paths.meta),
        _ => Err(format!("Unknown Claude Desktop backup path: {key}")),
    }
}

fn read_object(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|error| format!("Invalid Claude Desktop JSON at {}: {error}", path.display()))?;
    if !value.is_object() {
        return Err(format!(
            "Claude Desktop JSON must be an object: {}",
            path.display()
        ));
    }
    Ok(value)
}

fn write_object(path: &Path, value: &Value) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("Invalid Claude Desktop path")?)
        .map_err(|error| error.to_string())?;
    let content = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    crate::utils::atomic_write_string(path, &content)
        .map_err(|error| format!("Cannot write {}: {error}", path.display()))
}

pub(crate) fn read_providers(conn: &Connection) -> Result<Vec<DirectProvider>, String> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [SETTING_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let mut providers: Vec<DirectProvider> = stored.map_or_else(
        || Ok(Vec::new()),
        |raw| serde_json::from_str(&raw).map_err(|e| e.to_string()),
    )?;
    let mut migrated_legacy_secret = false;
    for provider in &mut providers {
        if provider.api_key.is_empty() {
            provider.api_key = credential_get(&provider.id)?.unwrap_or_default();
        } else {
            credential_set(&provider.id, &provider.api_key)?;
            migrated_legacy_secret = true;
        }
    }
    if migrated_legacy_secret {
        store_providers(conn, &providers)?;
    }
    Ok(providers)
}

fn store_providers(conn: &Connection, providers: &[DirectProvider]) -> Result<(), String> {
    let value = serde_json::to_string(providers).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![SETTING_KEY, value],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn credential_entry(provider_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, provider_id)
        .map_err(|error| format!("Failed to open Claude Desktop credential store: {error}"))
}

fn credential_get(provider_id: &str) -> Result<Option<String>, String> {
    match credential_entry(provider_id)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Failed to read Claude Desktop credential: {error}")),
    }
}

fn credential_set(provider_id: &str, api_key: &str) -> Result<(), String> {
    credential_entry(provider_id)?
        .set_password(api_key)
        .map_err(|error| format!("Failed to save Claude Desktop credential: {error}"))
}

fn credential_delete(provider_id: &str) -> Result<(), String> {
    match credential_entry(provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Failed to delete Claude Desktop credential: {error}"
        )),
    }
}

fn restore_credential(provider_id: &str, previous: Option<&str>) -> Result<(), String> {
    match previous.filter(|value| !value.is_empty()) {
        Some(value) => credential_set(provider_id, value),
        None => credential_delete(provider_id),
    }
}

pub(crate) fn gateway_token(conn: &Connection) -> Result<String, String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [GATEWAY_TOKEN_KEY],
        |row| row.get::<_, String>(0),
    )
    .map_err(|_| "Claude Desktop gateway token is unavailable".to_string())
}

fn ensure_gateway_token(conn: &Connection) -> Result<String, String> {
    if let Ok(token) = gateway_token(conn) {
        return Ok(token);
    }
    let token = format!("cchub-{}", uuid::Uuid::new_v4().simple());
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![GATEWAY_TOKEN_KEY, token],
    )
    .map_err(|error| error.to_string())?;
    Ok(token)
}

pub(crate) fn active_proxy_provider(conn: &Connection) -> Result<DirectProvider, String> {
    let id = active_id(&platform_paths()?).ok_or("Claude Desktop proxy provider is not active")?;
    read_providers(conn)?
        .into_iter()
        .find(|provider| provider.id == id && provider.mode == DesktopMode::Proxy)
        .ok_or_else(|| "Active Claude Desktop proxy provider was removed".to_string())
}

fn active_id(paths: &DesktopPaths) -> Option<String> {
    let meta = read_object(&paths.meta).ok()?;
    (meta.get("appliedId").and_then(Value::as_str) == Some(PROFILE_ID))
        .then(|| {
            meta.get("cchubProviderId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .flatten()
}

fn state(conn: &Connection, paths: &DesktopPaths) -> Result<DesktopProviderState, String> {
    Ok(DesktopProviderState {
        providers: read_providers(conn)?
            .into_iter()
            .map(|provider| DirectProviderSummary {
                id: provider.id,
                name: provider.name,
                base_url: provider.base_url,
                has_api_key: !provider.api_key.is_empty(),
                models: provider.models,
                mode: provider.mode,
                api_format: provider.api_format,
                model_routes: provider.model_routes,
            })
            .collect(),
        active_id: active_id(paths),
        profile_path: paths.profile.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn get_claude_desktop_providers(
    db: State<'_, DbState>,
) -> Result<DesktopProviderState, String> {
    let paths = platform_paths()?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    state(&conn, &paths)
}

fn validate_provider(provider: &DirectProvider) -> Result<(), String> {
    if provider.name.trim().is_empty() || provider.name.len() > 128 {
        return Err("Provider name must contain 1-128 characters".to_string());
    }
    let url = url::Url::parse(&provider.base_url).map_err(|_| "Enter a valid gateway URL")?;
    if url.host_str().is_none() || url.fragment().is_some() || !url.username().is_empty() {
        return Err(
            "Gateway URL must have a host and no embedded credentials or fragment".to_string(),
        );
    }
    let localhost = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !(url.scheme() == "http" && localhost) {
        return Err("Gateway URL must use HTTPS (HTTP is allowed only on loopback)".to_string());
    }
    if provider.api_key.trim().is_empty() {
        return Err("Gateway API key is required".to_string());
    }
    if provider.models.len() > 50 || provider.models.iter().any(|model| !safe_model(model)) {
        return Err(
            "Models must be valid Claude Sonnet, Opus, Haiku or Fable IDs (up to 50)".to_string(),
        );
    }
    if provider.mode == DesktopMode::Proxy {
        if !matches!(
            provider.api_format.as_str(),
            "anthropic" | "openai_chat" | "openai_responses" | "gemini_native"
        ) {
            return Err("Unsupported local proxy API format".to_string());
        }
        if provider.model_routes.is_empty()
            || provider.model_routes.len() > 50
            || provider
                .model_routes
                .iter()
                .any(|(route, upstream)| !safe_model(route) || upstream.trim().is_empty())
            || provider
                .models
                .iter()
                .any(|model| !provider.model_routes.contains_key(model))
        {
            return Err("Map every Claude Desktop model to an upstream model".to_string());
        }
    } else if provider.api_format != "anthropic" || !provider.model_routes.is_empty() {
        return Err("Direct mode supports native Anthropic gateways only".to_string());
    }
    Ok(())
}

fn safe_model(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    [
        "claude-sonnet-",
        "claude-opus-",
        "claude-haiku-",
        "claude-fable-",
    ]
    .iter()
    .any(|prefix| {
        model
            .strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty())
    }) && model
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn save_claude_desktop_provider(
    id: Option<String>,
    name: String,
    base_url: String,
    api_key: String,
    models: Vec<String>,
    mode: DesktopMode,
    api_format: String,
    model_routes: BTreeMap<String, String>,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<DesktopProviderState, String> {
    let paths = platform_paths()?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut providers = read_providers(&conn)?;
    let existing = id
        .as_deref()
        .and_then(|id| providers.iter().position(|item| item.id == id));
    if id.is_some() && existing.is_none() {
        return Err("Claude Desktop provider not found".to_string());
    }
    let previous_api_key = existing.map(|index| providers[index].api_key.clone());
    let provider = DirectProvider {
        id: id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        name: name.trim().to_string(),
        base_url: base_url.trim().trim_end_matches('/').to_string(),
        api_key: if api_key.trim().is_empty() {
            existing
                .map(|index| providers[index].api_key.clone())
                .unwrap_or_default()
        } else {
            api_key.trim().to_string()
        },
        models: models
            .into_iter()
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .collect(),
        mode,
        api_format,
        model_routes,
    };
    validate_provider(&provider)?;
    credential_set(&provider.id, &provider.api_key)?;
    let credential_id = provider.id.clone();
    if let Some(index) = existing {
        providers[index] = provider.clone();
    } else {
        providers.push(provider.clone());
    }
    let result = (|| -> Result<DesktopProviderState, String> {
        if active_id(&paths).as_deref() == Some(&provider.id) {
            let settings =
                crate::provider_proxy::read_local_provider_proxy_settings_from_conn(&conn);
            let was_enabled = settings
                .enabled_apps
                .iter()
                .any(|app| app == "claude-desktop");
            let token = if provider.mode == DesktopMode::Proxy {
                Some(ensure_gateway_token(&conn)?)
            } else {
                None
            };
            drop(conn);
            if provider.mode == DesktopMode::Proxy {
                crate::provider_proxy::set_claude_desktop_proxy_enabled(&app_handle, true)?;
                if let Err(error) = apply_proxy_at_paths(
                    &paths,
                    &provider,
                    settings.port,
                    token.as_deref().unwrap_or_default(),
                ) {
                    if !was_enabled {
                        let _ = crate::provider_proxy::set_claude_desktop_proxy_enabled(
                            &app_handle,
                            false,
                        );
                    }
                    return Err(error);
                }
            } else {
                apply_at_paths(&paths, &provider)?;
                crate::provider_proxy::set_claude_desktop_proxy_enabled(&app_handle, false)?;
            }
            let conn = db.0.lock().map_err(|error| error.to_string())?;
            store_providers(&conn, &providers)?;
            return state(&conn, &paths);
        }
        store_providers(&conn, &providers)?;
        state(&conn, &paths)
    })();
    match result {
        Ok(state) => Ok(state),
        Err(error) => match restore_credential(&credential_id, previous_api_key.as_deref()) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "{error}; credential rollback failed: {rollback_error}"
            )),
        },
    }
}

fn with_rollback(
    paths: &DesktopPaths,
    operation: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let files = [&paths.normal, &paths.threep, &paths.profile, &paths.meta];
    let snapshots = files
        .iter()
        .map(|path| {
            if path.exists() {
                fs::read(path).map(Some).map_err(|e| e.to_string())
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if let Err(error) = operation() {
        for (path, content) in files.iter().zip(snapshots) {
            let result = match content {
                Some(bytes) => fs::create_dir_all(path.parent().ok_or("Invalid rollback path")?)
                    .and_then(|_| fs::write(path, bytes)),
                None => {
                    if path.exists() {
                        fs::remove_file(path)
                    } else {
                        Ok(())
                    }
                }
            };
            if let Err(rollback_error) = result {
                return Err(format!(
                    "{error}; rollback failed at {}: {rollback_error}",
                    path.display()
                ));
            }
        }
        return Err(error);
    }
    Ok(())
}

fn apply_at_paths(paths: &DesktopPaths, provider: &DirectProvider) -> Result<(), String> {
    validate_provider(provider)?;
    if provider.mode != DesktopMode::Direct {
        return Err("Proxy mode requires a local gateway".to_string());
    }
    write_gateway_at_paths(paths, provider, &provider.base_url, &provider.api_key)
}

fn apply_proxy_at_paths(
    paths: &DesktopPaths,
    provider: &DirectProvider,
    port: u16,
    token: &str,
) -> Result<(), String> {
    validate_provider(provider)?;
    if provider.mode != DesktopMode::Proxy {
        return Err("Select a proxy provider".to_string());
    }
    write_gateway_at_paths(
        paths,
        provider,
        &format!("http://127.0.0.1:{port}/proxy/claude-desktop"),
        token,
    )
}

fn write_gateway_at_paths(
    paths: &DesktopPaths,
    provider: &DirectProvider,
    base_url: &str,
    api_key: &str,
) -> Result<(), String> {
    let mut normal = read_object(&paths.normal)?;
    let mut threep = read_object(&paths.threep)?;
    let mut meta = read_object(&paths.meta)?;
    normal["deploymentMode"] = json!("3p");
    threep["deploymentMode"] = json!("3p");
    let mut entries = meta
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    entries.retain(|entry| entry.get("id").and_then(Value::as_str) != Some(PROFILE_ID));
    entries.push(json!({"id": PROFILE_ID, "name": "CCHub"}));
    meta["entries"] = json!(entries);
    meta["appliedId"] = json!(PROFILE_ID);
    meta["cchubProviderId"] = json!(provider.id);
    let mut profile = json!({
        "coworkEgressAllowedHosts": ["*"],
        "disableDeploymentModeChooser": true,
        "inferenceProvider": "gateway",
        "inferenceGatewayAuthScheme": "bearer",
        "inferenceGatewayBaseUrl": base_url,
        "inferenceGatewayApiKey": api_key,
    });
    if !provider.models.is_empty() {
        profile["inferenceModels"] = json!(provider.models);
    }
    with_rollback(paths, || {
        write_object(&paths.normal, &normal)?;
        write_object(&paths.threep, &threep)?;
        write_object(&paths.profile, &profile)?;
        write_object(&paths.meta, &meta)
    })
}

#[tauri::command]
pub fn apply_claude_desktop_provider(
    id: String,
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<DesktopProviderState, String> {
    let paths = platform_paths()?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let provider = read_providers(&conn)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or("Claude Desktop provider not found")?;
    if provider.mode == DesktopMode::Proxy {
        let token = ensure_gateway_token(&conn)?;
        let settings = crate::provider_proxy::read_local_provider_proxy_settings_from_conn(&conn);
        let was_enabled = settings
            .enabled_apps
            .iter()
            .any(|app| app == "claude-desktop");
        drop(conn);
        crate::provider_proxy::set_claude_desktop_proxy_enabled(&app_handle, true)?;
        if let Err(error) = apply_proxy_at_paths(&paths, &provider, settings.port, &token) {
            if !was_enabled {
                let _ = crate::provider_proxy::set_claude_desktop_proxy_enabled(&app_handle, false);
            }
            return Err(error);
        }
    } else {
        apply_at_paths(&paths, &provider)?;
        drop(conn);
        crate::provider_proxy::set_claude_desktop_proxy_enabled(&app_handle, false)?;
    }
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    state(&conn, &paths)
}

fn restore_at_paths(paths: &DesktopPaths) -> Result<(), String> {
    let mut normal = read_object(&paths.normal)?;
    let mut threep = read_object(&paths.threep)?;
    let mut meta = read_object(&paths.meta)?;
    normal["deploymentMode"] = json!("1p");
    threep["deploymentMode"] = json!("1p");
    let mut entries = meta
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    entries.retain(|entry| entry.get("id").and_then(Value::as_str) != Some(PROFILE_ID));
    meta["entries"] = json!(entries);
    if meta.get("appliedId").and_then(Value::as_str) == Some(PROFILE_ID) {
        meta.as_object_mut()
            .expect("validated object")
            .remove("appliedId");
    }
    meta.as_object_mut()
        .expect("validated object")
        .remove("cchubProviderId");
    with_rollback(paths, || {
        write_object(&paths.normal, &normal)?;
        write_object(&paths.threep, &threep)?;
        if paths.profile.exists() {
            fs::remove_file(&paths.profile).map_err(|e| e.to_string())?;
        }
        write_object(&paths.meta, &meta)
    })
}

#[tauri::command]
pub fn restore_claude_desktop_official(
    app_handle: AppHandle,
    db: State<'_, DbState>,
) -> Result<DesktopProviderState, String> {
    let paths = platform_paths()?;
    restore_at_paths(&paths)?;
    crate::provider_proxy::set_claude_desktop_proxy_enabled(&app_handle, false)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    state(&conn, &paths)
}

#[tauri::command]
pub fn delete_claude_desktop_provider(
    id: String,
    db: State<'_, DbState>,
) -> Result<DesktopProviderState, String> {
    let paths = platform_paths()?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    if active_id(&paths).as_deref() == Some(&id) {
        return Err(
            "Restore official mode or switch providers before deleting the active provider"
                .to_string(),
        );
    }
    let mut providers = read_providers(&conn)?;
    let previous_api_key = providers
        .iter()
        .find(|item| item.id == id)
        .map(|provider| provider.api_key.clone());
    let count = providers.len();
    providers.retain(|item| item.id != id);
    if providers.len() == count {
        return Err("Claude Desktop provider not found".to_string());
    }
    credential_delete(&id)?;
    if let Err(error) = store_providers(&conn, &providers) {
        let rollback = restore_credential(&id, previous_api_key.as_deref());
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "{error}; credential rollback failed: {rollback_error}"
            )),
        };
    }
    state(&conn, &paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> DirectProvider {
        DirectProvider {
            id: "first".into(),
            name: "Gateway".into(),
            base_url: "https://example.com".into(),
            api_key: "secret".into(),
            models: vec!["claude-sonnet-4-6".into()],
            mode: DesktopMode::Direct,
            api_format: "anthropic".into(),
            model_routes: BTreeMap::new(),
        }
    }

    #[test]
    fn rejects_unsafe_urls_and_model_ids() {
        let mut item = provider();
        item.base_url = "http://example.com".into();
        assert!(validate_provider(&item).is_err());
        item.base_url = "https://user:pass@example.com".into();
        assert!(validate_provider(&item).is_err());
        item.base_url = "http://127.0.0.1:3000".into();
        item.models = vec!["other-model".into()];
        assert!(validate_provider(&item).is_err());
        item.models = vec!["claude-haiku-4-5".into()];
        assert!(validate_provider(&item).is_ok());
    }

    #[test]
    fn applies_and_restores_without_touching_mcp_or_foreign_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths_from_dirs(temp.path().join("Claude"), temp.path().join("Claude-3p"));
        write_object(
            &paths.normal,
            &json!({"mcpServers": {"files": {"command": "node"}}}),
        )
        .unwrap();
        write_object(
            &paths.meta,
            &json!({"entries": [{"id": "other", "name": "Other"}], "custom": true}),
        )
        .unwrap();
        apply_at_paths(&paths, &provider()).unwrap();
        assert_eq!(read_object(&paths.normal).unwrap()["deploymentMode"], "3p");
        assert_eq!(
            read_object(&paths.normal).unwrap()["mcpServers"]["files"]["command"],
            "node"
        );
        assert_eq!(
            read_object(&paths.profile).unwrap()["inferenceModels"],
            json!(["claude-sonnet-4-6"])
        );
        assert_eq!(active_id(&paths).as_deref(), Some("first"));
        restore_at_paths(&paths).unwrap();
        assert_eq!(read_object(&paths.normal).unwrap()["deploymentMode"], "1p");
        assert!(read_object(&paths.meta).unwrap()["custom"] == true);
        assert_eq!(
            read_object(&paths.meta).unwrap()["entries"][0]["id"],
            "other"
        );
        assert!(!paths.profile.exists());
    }

    #[test]
    fn rejects_malformed_existing_files_without_mutating_them() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths_from_dirs(temp.path().join("Claude"), temp.path().join("Claude-3p"));
        fs::create_dir_all(paths.meta.parent().unwrap()).unwrap();
        fs::write(&paths.meta, "invalid").unwrap();
        assert!(apply_at_paths(&paths, &provider()).is_err());
        assert_eq!(fs::read_to_string(&paths.meta).unwrap(), "invalid");
        assert!(!paths.normal.exists());
    }

    #[test]
    fn proxy_profile_uses_local_token_and_safe_model_routes() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths_from_dirs(temp.path().join("Claude"), temp.path().join("Claude-3p"));
        let mut item = provider();
        item.mode = DesktopMode::Proxy;
        item.api_format = "openai_responses".into();
        assert!(validate_provider(&item).is_err());
        item.model_routes
            .insert("claude-sonnet-4-6".into(), "upstream-model".into());
        apply_proxy_at_paths(&paths, &item, 34567, "local-only-token").unwrap();
        let profile = read_object(&paths.profile).unwrap();
        assert_eq!(
            profile["inferenceGatewayBaseUrl"],
            "http://127.0.0.1:34567/proxy/claude-desktop"
        );
        assert_eq!(profile["inferenceGatewayApiKey"], "local-only-token");
        assert_eq!(profile["inferenceModels"], json!(["claude-sonnet-4-6"]));
        assert_ne!(profile["inferenceGatewayApiKey"], item.api_key);
    }

    #[test]
    fn persisted_provider_metadata_never_contains_api_keys() {
        let value = serde_json::to_value(provider()).unwrap();
        assert!(value.get("apiKey").is_none());
        assert!(!value.to_string().contains("secret"));

        let legacy: DirectProvider = serde_json::from_value(json!({
            "id": "legacy",
            "name": "Legacy",
            "baseUrl": "https://example.com",
            "apiKey": "legacy-secret",
            "models": []
        }))
        .unwrap();
        assert_eq!(legacy.api_key, "legacy-secret");
    }
}
