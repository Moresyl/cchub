use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::db::DbState;

const SETTING_KEY: &str = "claude_desktop_direct_providers";
const PROFILE_ID: &str = "00000000-0000-4000-8000-0000000cc8ab";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectProvider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    models: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectProviderSummary {
    id: String,
    name: String,
    base_url: String,
    has_api_key: bool,
    models: Vec<String>,
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
        return Ok(paths_from_dirs(base.join("Claude"), base.join("Claude-3p")));
    }
    #[cfg(target_os = "macos")]
    {
        let base = dirs::home_dir()
            .ok_or("Cannot locate home directory")?
            .join("Library/Application Support");
        return Ok(paths_from_dirs(base.join("Claude"), base.join("Claude-3p")));
    }
    #[cfg(target_os = "linux")]
    {
        let base = dirs::config_dir().ok_or("Cannot locate config directory")?;
        return Ok(paths_from_dirs(base.join("Claude"), base.join("Claude-3p")));
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    Err("Claude Desktop is unsupported on this platform".to_string())
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

fn read_providers(conn: &Connection) -> Result<Vec<DirectProvider>, String> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [SETTING_KEY],
            |row| row.get(0),
        )
        .ok();
    stored.map_or_else(
        || Ok(Vec::new()),
        |raw| serde_json::from_str(&raw).map_err(|e| e.to_string()),
    )
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
pub fn save_claude_desktop_provider(
    id: Option<String>,
    name: String,
    base_url: String,
    api_key: String,
    models: Vec<String>,
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
    };
    validate_provider(&provider)?;
    if let Some(index) = existing {
        providers[index] = provider.clone();
    } else {
        providers.push(provider.clone());
    }
    if active_id(&paths).as_deref() == Some(&provider.id) {
        apply_at_paths(&paths, &provider)?;
    }
    store_providers(&conn, &providers)?;
    state(&conn, &paths)
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
        "inferenceGatewayBaseUrl": provider.base_url,
        "inferenceGatewayApiKey": provider.api_key,
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
    db: State<'_, DbState>,
) -> Result<DesktopProviderState, String> {
    let paths = platform_paths()?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let provider = read_providers(&conn)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or("Claude Desktop provider not found")?;
    apply_at_paths(&paths, &provider)?;
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
    db: State<'_, DbState>,
) -> Result<DesktopProviderState, String> {
    let paths = platform_paths()?;
    restore_at_paths(&paths)?;
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
    let count = providers.len();
    providers.retain(|item| item.id != id);
    if providers.len() == count {
        return Err("Claude Desktop provider not found".to_string());
    }
    store_providers(&conn, &providers)?;
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
}
