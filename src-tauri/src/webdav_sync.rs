use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::{Method, StatusCode, Url};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::extra_commands::{
    generate_sql_backup, get_json_app_setting, import_backup_from_path_impl, set_json_app_setting,
};
use crate::db::DbState;
use crate::utils::configure_background_command;

const WEBDAV_SYNC_SETTINGS_KEY: &str = "webdav_sync_settings";
const WEBDAV_MANIFEST_FILE: &str = "manifest.json";
const WEBDAV_FORMAT: &str = "cchub-webdav-sync";
const WEBDAV_PROTOCOL_VERSION: u32 = 1;
const WEBDAV_DB_COMPAT_VERSION: u32 = 1;
const MAX_WEBDAV_SYNC_BYTES: usize = 15 * 1024 * 1024;
const AUTO_SYNC_INTERVAL_SECS: u64 = 15 * 60;
const WEBDAV_KEYRING_SERVICE: &str = "cchub";
const WEBDAV_KEYRING_ACCOUNT: &str = "webdav_sync_password";

static WEBDAV_SYNC_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn webdav_sync_lock() -> &'static tokio::sync::Mutex<()> {
    WEBDAV_SYNC_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebDavSyncSettings {
    pub enabled: bool,
    pub base_url: String,
    pub username: String,
    #[serde(default, skip_serializing)]
    pub password: String,
    pub has_password: bool,
    pub remote_root: String,
    pub profile: String,
    pub auto_sync: bool,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
}

impl Default for WebDavSyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: String::new(),
            username: String::new(),
            password: String::new(),
            has_password: false,
            remote_root: "cchub-sync".to_string(),
            profile: "default".to_string(),
            auto_sync: false,
            last_sync_at: None,
            last_error: None,
        }
    }
}

impl WebDavSyncSettings {
    pub fn normalize(&mut self) {
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        self.username = self.username.trim().to_string();
        self.remote_root = normalize_segment(&self.remote_root, "cchub-sync");
        self.profile = normalize_segment(&self.profile, "default");
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        validate_base_url(&self.base_url)?;
        if self.username.is_empty() {
            return Err("WebDAV username is required".to_string());
        }
        if self.password.trim().is_empty() {
            return Err("WebDAV password is required".to_string());
        }
        Ok(())
    }

    pub fn masked_for_frontend(&self) -> Self {
        let mut masked = self.clone();
        masked.has_password = masked.has_password || !masked.password.trim().is_empty();
        masked.password.clear();
        masked
    }
}

trait WebDavCredentialStore {
    fn get_password(&self) -> Result<Option<String>, String>;
    fn set_password(&self, password: &str) -> Result<(), String>;
    fn delete_password(&self) -> Result<(), String>;
}

struct KeyringWebDavCredentialStore;

impl KeyringWebDavCredentialStore {
    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(WEBDAV_KEYRING_SERVICE, WEBDAV_KEYRING_ACCOUNT)
            .map_err(|error| format!("Failed to open WebDAV credential store: {error}"))
    }
}

impl WebDavCredentialStore for KeyringWebDavCredentialStore {
    fn get_password(&self) -> Result<Option<String>, String> {
        match self.entry()?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!(
                "Failed to read WebDAV password from keyring: {error}"
            )),
        }
    }

    fn set_password(&self, password: &str) -> Result<(), String> {
        self.entry()?
            .set_password(password)
            .map_err(|error| format!("Failed to save WebDAV password to keyring: {error}"))
    }

    fn delete_password(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!(
                "Failed to delete WebDAV password from keyring: {error}"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavRemoteInfo {
    pub exists: bool,
    pub remote_url: String,
    pub snapshot_path: Option<String>,
    pub updated_at: Option<String>,
    pub size_bytes: Option<u64>,
    pub app_version: Option<String>,
    pub device_name: Option<String>,
    pub layout: Option<String>,
    pub compatible: bool,
    pub protocol_version: Option<u32>,
    pub db_compat_version: Option<u32>,
    pub profile_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavSyncEvent {
    pub status: String,
    pub message: String,
    pub synced_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct WebDavManifest {
    format: String,
    protocol_version: Option<u32>,
    db_compat_version: Option<u32>,
    app_version: String,
    created_at: String,
    snapshot_path: String,
    size_bytes: u64,
    device_name: String,
    profile_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebDavRemoteLayout {
    Current,
    Legacy,
}

impl WebDavRemoteLayout {
    fn label(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Legacy => "legacy",
        }
    }
}

pub fn migrate_webdav_password_to_keyring(conn: &rusqlite::Connection) -> Result<(), String> {
    let _ = read_settings_with_store(conn, &KeyringWebDavCredentialStore)?;
    Ok(())
}

pub fn read_settings(conn: &rusqlite::Connection) -> Result<WebDavSyncSettings, String> {
    read_settings_with_store(conn, &KeyringWebDavCredentialStore)
}

fn read_settings_with_store(
    conn: &rusqlite::Connection,
    credential_store: &impl WebDavCredentialStore,
) -> Result<WebDavSyncSettings, String> {
    let mut settings: WebDavSyncSettings =
        get_json_app_setting(conn, WEBDAV_SYNC_SETTINGS_KEY)?.unwrap_or_default();

    let stored_password = settings.password.trim().to_string();
    if !stored_password.is_empty() {
        credential_store.set_password(&stored_password)?;
        settings.password.clear();
        settings.has_password = true;
        set_json_app_setting(conn, WEBDAV_SYNC_SETTINGS_KEY, &settings)?;
        settings.password = stored_password;
    } else if settings.has_password {
        settings.password = credential_store.get_password()?.unwrap_or_default();
        settings.has_password = !settings.password.trim().is_empty();
    }

    settings.normalize();
    Ok(settings)
}

pub fn write_settings(
    conn: &rusqlite::Connection,
    incoming: WebDavSyncSettings,
    password_touched: bool,
) -> Result<WebDavSyncSettings, String> {
    write_settings_with_store(
        conn,
        incoming,
        password_touched,
        &KeyringWebDavCredentialStore,
    )
}

fn write_settings_with_store(
    conn: &rusqlite::Connection,
    mut incoming: WebDavSyncSettings,
    password_touched: bool,
    credential_store: &impl WebDavCredentialStore,
) -> Result<WebDavSyncSettings, String> {
    let existing = read_settings_with_store(conn, credential_store).unwrap_or_default();
    incoming.normalize();
    if !password_touched && incoming.password.is_empty() && !existing.password.is_empty() {
        incoming.password = existing.password;
    }
    if incoming.last_sync_at.is_none() {
        incoming.last_sync_at = existing.last_sync_at;
    }
    if incoming.last_error.is_none() {
        incoming.last_error = existing.last_error;
    }
    incoming.validate()?;
    if incoming.password.trim().is_empty() {
        credential_store.delete_password()?;
        incoming.has_password = false;
    } else {
        credential_store.set_password(incoming.password.trim())?;
        incoming.has_password = true;
    }
    incoming.password.clear();
    set_json_app_setting(conn, WEBDAV_SYNC_SETTINGS_KEY, &incoming)?;
    Ok(incoming.masked_for_frontend())
}

pub fn update_sync_status(
    conn: &rusqlite::Connection,
    last_sync_at: Option<String>,
    last_error: Option<String>,
) -> Result<WebDavSyncSettings, String> {
    let mut settings = read_settings(conn)?;
    settings.last_sync_at = last_sync_at;
    settings.last_error = last_error;
    settings.has_password = settings.has_password || !settings.password.trim().is_empty();
    settings.password.clear();
    set_json_app_setting(conn, WEBDAV_SYNC_SETTINGS_KEY, &settings)?;
    Ok(settings.masked_for_frontend())
}

pub async fn test_connection(
    mut settings: WebDavSyncSettings,
    existing: Option<WebDavSyncSettings>,
    preserve_empty_password: bool,
) -> Result<(), String> {
    if preserve_empty_password && settings.password.trim().is_empty() {
        if let Some(existing_settings) = existing {
            settings.password = existing_settings.password;
        }
    }
    settings.normalize();
    let was_enabled = settings.enabled;
    settings.enabled = true;
    let validation = settings.validate();
    settings.enabled = was_enabled;
    validation?;

    let client = build_client()?;
    let response = auth_request(
        client
            .request(method_propfind()?, normalize_base_url(&settings.base_url))
            .header("Depth", "0"),
        &settings,
    )
    .send()
    .await
    .map_err(|error| format!("WebDAV connection failed: {error}"))?;

    let status = response.status();
    if status.is_success() || status.as_u16() == 207 {
        return Ok(());
    }

    Err(format!("WebDAV server returned {status}"))
}

pub async fn fetch_remote_info(db: &State<'_, DbState>) -> Result<WebDavRemoteInfo, String> {
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_settings(&conn)?
    };

    let default_url =
        manifest_url_for_layout(&settings, WebDavRemoteLayout::Current).unwrap_or_default();
    if !settings.enabled {
        return Ok(WebDavRemoteInfo {
            exists: false,
            remote_url: default_url,
            snapshot_path: None,
            updated_at: None,
            size_bytes: None,
            app_version: None,
            device_name: None,
            layout: Some(WebDavRemoteLayout::Current.label().to_string()),
            compatible: true,
            protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
            db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
            profile_path: Some(remote_profile_path(&settings, WebDavRemoteLayout::Current)),
        });
    }

    let _guard = webdav_sync_lock().lock().await;
    let client = build_client()?;
    match fetch_manifest_with_fallback(&client, &settings).await? {
        Some((manifest, layout)) => {
            let compatible = validate_manifest_compatibility(&manifest, layout).is_ok();
            Ok(WebDavRemoteInfo {
                exists: true,
                remote_url: manifest_url_for_layout(&settings, layout)?,
                snapshot_path: Some(manifest.snapshot_path.clone()),
                updated_at: Some(manifest.created_at.clone()),
                size_bytes: Some(manifest.size_bytes),
                app_version: Some(manifest.app_version.clone()),
                device_name: Some(manifest.device_name.clone()),
                layout: Some(layout.label().to_string()),
                compatible,
                protocol_version: manifest.protocol_version,
                db_compat_version: manifest.db_compat_version,
                profile_path: manifest
                    .profile_path
                    .clone()
                    .or_else(|| Some(remote_profile_path(&settings, layout))),
            })
        }
        None => Ok(WebDavRemoteInfo {
            exists: false,
            remote_url: default_url,
            snapshot_path: None,
            updated_at: None,
            size_bytes: None,
            app_version: None,
            device_name: None,
            layout: Some(WebDavRemoteLayout::Current.label().to_string()),
            compatible: true,
            protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
            db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
            profile_path: Some(remote_profile_path(&settings, WebDavRemoteLayout::Current)),
        }),
    }
}

pub async fn upload(db: &State<'_, DbState>) -> Result<WebDavRemoteInfo, String> {
    let _guard = webdav_sync_lock().lock().await;
    match upload_inner(db).await {
        Ok(info) => Ok(info),
        Err(error) => {
            if let Ok(conn) = db.0.lock() {
                let _ = update_sync_status(&conn, None, Some(error.clone()));
            }
            Err(error)
        }
    }
}

async fn upload_inner(db: &State<'_, DbState>) -> Result<WebDavRemoteInfo, String> {
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let settings = read_settings(&conn)?;
        if !settings.enabled {
            return Err("WebDAV sync is not enabled".to_string());
        }
        settings
    };

    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let sql_bytes = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        generate_sql_backup(&conn, &home).into_bytes()
    };
    let size_bytes = sql_bytes.len() as u64;
    if sql_bytes.len() > MAX_WEBDAV_SYNC_BYTES {
        return Err(format!(
            "WebDAV upload aborted because backup size exceeds {} MB",
            MAX_WEBDAV_SYNC_BYTES / (1024 * 1024)
        ));
    }

    let client = build_client()?;
    ensure_remote_directories(&client, &settings, WebDavRemoteLayout::Current).await?;

    let created_at = chrono::Utc::now().to_rfc3339();
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let snapshot_name = format!("cchub-sync-{timestamp}.sql");
    let snapshot_path = format!("snapshots/{snapshot_name}");
    let snapshot_target = remote_file_url(&settings, WebDavRemoteLayout::Current, &snapshot_path)?;
    upload_bytes(
        &client,
        &settings,
        &snapshot_target,
        "application/sql",
        sql_bytes,
    )
    .await?;

    let app_version = env!("CARGO_PKG_VERSION").to_string();
    let device_name = device_name();
    let manifest = WebDavManifest {
        format: WEBDAV_FORMAT.to_string(),
        protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
        db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
        app_version: app_version.clone(),
        created_at: created_at.clone(),
        snapshot_path: snapshot_path.clone(),
        size_bytes,
        device_name: device_name.clone(),
        profile_path: Some(remote_profile_path(&settings, WebDavRemoteLayout::Current)),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    let manifest_url = manifest_url_for_layout(&settings, WebDavRemoteLayout::Current)?;
    upload_bytes(
        &client,
        &settings,
        &manifest_url,
        "application/json",
        manifest_bytes,
    )
    .await?;

    let saved = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        update_sync_status(&conn, Some(created_at.clone()), None)?
    };

    Ok(WebDavRemoteInfo {
        exists: true,
        remote_url: manifest_url,
        snapshot_path: Some(snapshot_path),
        updated_at: saved.last_sync_at,
        size_bytes: Some(size_bytes),
        app_version: Some(app_version),
        device_name: Some(device_name),
        layout: Some(WebDavRemoteLayout::Current.label().to_string()),
        compatible: true,
        protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
        db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
        profile_path: manifest.profile_path,
    })
}

pub async fn download(db: &State<'_, DbState>) -> Result<String, String> {
    let _guard = webdav_sync_lock().lock().await;
    match download_inner(db).await {
        Ok(message) => Ok(message),
        Err(error) => {
            if let Ok(conn) = db.0.lock() {
                let _ = update_sync_status(&conn, None, Some(error.clone()));
            }
            Err(error)
        }
    }
}

async fn download_inner(db: &State<'_, DbState>) -> Result<String, String> {
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let settings = read_settings(&conn)?;
        if !settings.enabled {
            return Err("WebDAV sync is not enabled".to_string());
        }
        settings
    };

    let client = build_client()?;
    let (manifest, layout) = fetch_manifest_with_fallback(&client, &settings)
        .await?
        .ok_or_else(|| "No remote WebDAV sync manifest found".to_string())?;
    validate_manifest_compatibility(&manifest, layout)?;

    if manifest.size_bytes > MAX_WEBDAV_SYNC_BYTES as u64 {
        return Err(format!(
            "Remote backup is too large to restore automatically ({} MB)",
            manifest.size_bytes / (1024 * 1024)
        ));
    }

    let snapshot_target = remote_file_url(&settings, layout, &manifest.snapshot_path)?;
    let response = auth_request(client.get(snapshot_target), &settings)
        .send()
        .await
        .map_err(|error| format!("Failed to download WebDAV snapshot: {error}"))?
        .error_for_status()
        .map_err(|error| format!("WebDAV snapshot download failed: {error}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read WebDAV snapshot body: {error}"))?;

    if bytes.len() > MAX_WEBDAV_SYNC_BYTES {
        return Err(format!(
            "Remote backup body exceeds the safety limit of {} MB",
            MAX_WEBDAV_SYNC_BYTES / (1024 * 1024)
        ));
    }

    let temp_dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let temp_file = temp_dir.path().join("cchub-webdav-sync.sql");
    std::fs::write(&temp_file, &bytes).map_err(|error| error.to_string())?;
    let message = import_backup_from_path_impl(db, &temp_file)?;

    {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let _ = update_sync_status(&conn, Some(chrono::Utc::now().to_rfc3339()), None)?;
    }

    Ok(message)
}

pub fn spawn_auto_sync_loop(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(AUTO_SYNC_INTERVAL_SECS));
        interval.tick().await;
        loop {
            interval.tick().await;
            let event = match run_auto_sync_if_enabled(&app_handle).await {
                Ok(event) => event,
                Err(error) => Some(WebDavSyncEvent {
                    status: "error".to_string(),
                    message: "Automatic WebDAV sync failed".to_string(),
                    synced_at: None,
                    error: Some(error),
                }),
            };
            if let Some(payload) = event {
                let _ = app_handle.emit("webdav-sync-status-updated", &payload);
            }
        }
    });
}

pub async fn run_auto_sync_if_enabled(
    app_handle: &AppHandle,
) -> Result<Option<WebDavSyncEvent>, String> {
    let db = app_handle.state::<DbState>();
    let should_sync = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let settings = read_settings(&conn)?;
        settings.enabled && settings.auto_sync
    };

    if !should_sync {
        return Ok(None);
    }

    match upload(&db).await {
        Ok(info) => Ok(Some(WebDavSyncEvent {
            status: "success".to_string(),
            message: "Automatic WebDAV sync completed".to_string(),
            synced_at: info.updated_at,
            error: None,
        })),
        Err(error) => Ok(Some(WebDavSyncEvent {
            status: "error".to_string(),
            message: "Automatic WebDAV sync failed".to_string(),
            synced_at: None,
            error: Some(error),
        })),
    }
}

fn normalize_segment(value: &str, fallback: &str) -> String {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_base_url(value: &str) -> String {
    format!("{}/", value.trim().trim_end_matches('/'))
}

fn validate_base_url(value: &str) -> Result<(), String> {
    let url = Url::parse(&normalize_base_url(value))
        .map_err(|_| "WebDAV base URL is invalid".to_string())?;
    match url.scheme() {
        "http" | "https" => Ok(()),
        _ => Err("WebDAV base URL must use http or https".to_string()),
    }
}

fn method_propfind() -> Result<Method, String> {
    Method::from_bytes(b"PROPFIND")
        .map_err(|error| format!("Failed to construct PROPFIND method: {error}"))
}

fn method_mkcol() -> Result<Method, String> {
    Method::from_bytes(b"MKCOL")
        .map_err(|error| format!("Failed to construct MKCOL method: {error}"))
}

fn remote_prefix_segments(
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> Vec<String> {
    match layout {
        WebDavRemoteLayout::Current => vec![
            settings.remote_root.clone(),
            format!("v{WEBDAV_PROTOCOL_VERSION}"),
            format!("db-v{WEBDAV_DB_COMPAT_VERSION}"),
            settings.profile.clone(),
        ],
        WebDavRemoteLayout::Legacy => {
            vec![settings.remote_root.clone(), settings.profile.clone()]
        }
    }
}

fn remote_profile_path(settings: &WebDavSyncSettings, layout: WebDavRemoteLayout) -> String {
    remote_prefix_segments(settings, layout).join("/")
}

fn split_relative_path(relative_path: &str) -> Vec<String> {
    relative_path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn build_remote_url(
    base_url: &str,
    segments: &[String],
    trailing_slash: bool,
) -> Result<String, String> {
    validate_base_url(base_url)?;
    let mut url = Url::parse(&normalize_base_url(base_url))
        .map_err(|error| format!("WebDAV base URL is invalid: {error}"))?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| "WebDAV base URL cannot be used to append path segments".to_string())?;
        path.pop_if_empty();
        for segment in segments {
            path.push(segment);
        }
    }

    let mut output = url.to_string();
    if trailing_slash && !output.ends_with('/') {
        output.push('/');
    }
    Ok(output)
}

fn remote_file_url(
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
    relative_path: &str,
) -> Result<String, String> {
    let mut segments = remote_prefix_segments(settings, layout);
    segments.extend(split_relative_path(relative_path));
    build_remote_url(&settings.base_url, &segments, false)
}

fn manifest_url_for_layout(
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> Result<String, String> {
    remote_file_url(settings, layout, WEBDAV_MANIFEST_FILE)
}

fn build_client() -> Result<reqwest::Client, String> {
    crate::shared::http_client::build_http_client(
        None,
        Some(&format!("CCHub/{} WebDAV", env!("CARGO_PKG_VERSION"))),
        Duration::from_secs(30),
    )
    .map_err(|error| format!("Failed to build WebDAV HTTP client: {error}"))
}

fn auth_request(
    builder: reqwest::RequestBuilder,
    settings: &WebDavSyncSettings,
) -> reqwest::RequestBuilder {
    builder.basic_auth(&settings.username, Some(&settings.password))
}

async fn ensure_remote_directories(
    client: &reqwest::Client,
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> Result<(), String> {
    let mkcol = method_mkcol()?;
    let mut path_segments = remote_prefix_segments(settings, layout);
    path_segments.push("snapshots".to_string());

    for depth in 1..=path_segments.len() {
        let target = build_remote_url(&settings.base_url, &path_segments[..depth], true)?;
        let response = auth_request(client.request(mkcol.clone(), target), settings)
            .send()
            .await
            .map_err(|error| format!("Failed to create remote WebDAV directory: {error}"))?;
        let status = response.status();
        if !(status.is_success()
            || status == StatusCode::METHOD_NOT_ALLOWED
            || status == StatusCode::CONFLICT
            || status.is_redirection())
        {
            return Err(format!(
                "WebDAV directory creation failed with status {status}"
            ));
        }
    }
    Ok(())
}

async fn upload_bytes(
    client: &reqwest::Client,
    settings: &WebDavSyncSettings,
    url: &str,
    content_type: &str,
    bytes: Vec<u8>,
) -> Result<(), String> {
    auth_request(
        client
            .put(url)
            .header("Content-Type", content_type)
            .body(bytes),
        settings,
    )
    .send()
    .await
    .map_err(|error| format!("WebDAV upload failed: {error}"))?
    .error_for_status()
    .map_err(|error| format!("WebDAV upload returned error: {error}"))?;
    Ok(())
}

async fn fetch_manifest_with_fallback(
    client: &reqwest::Client,
    settings: &WebDavSyncSettings,
) -> Result<Option<(WebDavManifest, WebDavRemoteLayout)>, String> {
    if let Some(manifest) =
        fetch_manifest_for_layout(client, settings, WebDavRemoteLayout::Current).await?
    {
        return Ok(Some((manifest, WebDavRemoteLayout::Current)));
    }
    if let Some(manifest) =
        fetch_manifest_for_layout(client, settings, WebDavRemoteLayout::Legacy).await?
    {
        return Ok(Some((manifest, WebDavRemoteLayout::Legacy)));
    }
    Ok(None)
}

async fn fetch_manifest_for_layout(
    client: &reqwest::Client,
    settings: &WebDavSyncSettings,
    layout: WebDavRemoteLayout,
) -> Result<Option<WebDavManifest>, String> {
    let response = auth_request(
        client.get(manifest_url_for_layout(settings, layout)?),
        settings,
    )
    .send()
    .await
    .map_err(|error| format!("Failed to fetch WebDAV manifest: {error}"))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let response = response
        .error_for_status()
        .map_err(|error| format!("WebDAV manifest request failed: {error}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read WebDAV manifest body: {error}"))?;
    let manifest = serde_json::from_slice::<WebDavManifest>(&bytes)
        .map_err(|error| format!("Invalid WebDAV manifest: {error}"))?;
    Ok(Some(manifest))
}

fn validate_manifest_compatibility(
    manifest: &WebDavManifest,
    layout: WebDavRemoteLayout,
) -> Result<(), String> {
    if manifest.snapshot_path.trim().is_empty() {
        return Err("WebDAV manifest is missing snapshot path".to_string());
    }

    if layout == WebDavRemoteLayout::Current {
        if manifest.format != WEBDAV_FORMAT {
            return Err(format!(
                "WebDAV manifest format mismatch: expected {WEBDAV_FORMAT}, got {}",
                manifest.format
            ));
        }
        if manifest.protocol_version != Some(WEBDAV_PROTOCOL_VERSION) {
            return Err(format!(
                "WebDAV protocol version mismatch: expected v{WEBDAV_PROTOCOL_VERSION}, got {:?}",
                manifest.protocol_version
            ));
        }
        if manifest.db_compat_version != Some(WEBDAV_DB_COMPAT_VERSION) {
            return Err(format!(
                "WebDAV DB compatibility mismatch: expected db-v{WEBDAV_DB_COMPAT_VERSION}, got {:?}",
                manifest.db_compat_version
            ));
        }
    }

    Ok(())
}

fn device_name() -> String {
    for key in ["CC_SWITCH_DEVICE_NAME", "COMPUTERNAME", "HOSTNAME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.chars().take(64).collect();
            }
        }
    }

    let mut command = Command::new("hostname");
    configure_background_command(&mut command);
    if let Ok(output) = command.output() {
        if output.status.success() {
            if let Ok(hostname) = String::from_utf8(output.stdout) {
                let trimmed = hostname.trim();
                if !trimmed.is_empty() {
                    return trimmed.chars().take(64).collect();
                }
            }
        }
    }

    "cchub".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MemoryCredentialStore {
        password: RefCell<Option<String>>,
    }

    impl WebDavCredentialStore for MemoryCredentialStore {
        fn get_password(&self) -> Result<Option<String>, String> {
            Ok(self.password.borrow().clone())
        }

        fn set_password(&self, password: &str) -> Result<(), String> {
            *self.password.borrow_mut() = Some(password.to_string());
            Ok(())
        }

        fn delete_password(&self) -> Result<(), String> {
            *self.password.borrow_mut() = None;
            Ok(())
        }
    }

    fn memory_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn stored_webdav_json(conn: &rusqlite::Connection) -> serde_json::Value {
        let raw: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                rusqlite::params![WEBDAV_SYNC_SETTINGS_KEY],
                |row| row.get(0),
            )
            .unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn webdav_password_save_read_delete_uses_credential_store() {
        let conn = memory_conn();
        let store = MemoryCredentialStore::default();
        let settings = WebDavSyncSettings {
            enabled: true,
            base_url: "https://dav.example.com/".to_string(),
            username: " alice ".to_string(),
            password: "secret-token".to_string(),
            has_password: false,
            remote_root: " /configs/ ".to_string(),
            profile: " main ".to_string(),
            auto_sync: true,
            last_sync_at: None,
            last_error: None,
        };

        let frontend =
            write_settings_with_store(&conn, settings, true, &store).expect("save settings");

        assert!(frontend.password.is_empty());
        assert!(frontend.has_password);
        assert_eq!(
            store.get_password().unwrap(),
            Some("secret-token".to_string())
        );
        let raw = stored_webdav_json(&conn);
        assert!(raw.get("password").is_none());
        assert_eq!(raw["has_password"], true);

        let loaded = read_settings_with_store(&conn, &store).expect("read settings");
        assert_eq!(loaded.password, "secret-token");
        assert!(loaded.has_password);

        let cleared = WebDavSyncSettings {
            enabled: false,
            base_url: "https://dav.example.com".to_string(),
            username: "alice".to_string(),
            password: String::new(),
            has_password: true,
            remote_root: "configs".to_string(),
            profile: "main".to_string(),
            auto_sync: false,
            last_sync_at: None,
            last_error: None,
        };

        let frontend =
            write_settings_with_store(&conn, cleared, true, &store).expect("delete password");

        assert!(!frontend.has_password);
        assert_eq!(store.get_password().unwrap(), None);
        let raw = stored_webdav_json(&conn);
        assert!(raw.get("password").is_none());
        assert_eq!(raw["has_password"], false);
    }

    #[test]
    fn read_settings_migrates_legacy_plaintext_password() {
        let conn = memory_conn();
        let store = MemoryCredentialStore::default();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![
                WEBDAV_SYNC_SETTINGS_KEY,
                r#"{
                    "enabled": true,
                    "base_url": "https://dav.example.com",
                    "username": "alice",
                    "password": "legacy-secret",
                    "remote_root": "configs",
                    "profile": "main",
                    "auto_sync": false
                }"#
            ],
        )
        .unwrap();

        let loaded = read_settings_with_store(&conn, &store).expect("migrate settings");

        assert_eq!(loaded.password, "legacy-secret");
        assert!(loaded.has_password);
        assert_eq!(
            store.get_password().unwrap(),
            Some("legacy-secret".to_string())
        );
        let raw = stored_webdav_json(&conn);
        assert!(raw.get("password").is_none());
        assert_eq!(raw["has_password"], true);
    }

    #[test]
    fn webdav_settings_deserialize_and_normalize_paths() {
        let mut settings: WebDavSyncSettings = serde_json::from_str(
            r#"{
                "enabled": false,
                "base_url": "https://dav.example.com/root///",
                "username": " alice ",
                "remote_root": " /configs// ",
                "profile": " /main/ ",
                "auto_sync": true
            }"#,
        )
        .unwrap();

        settings.normalize();

        assert_eq!(settings.base_url, "https://dav.example.com/root");
        assert_eq!(settings.username, "alice");
        assert_eq!(settings.remote_root, "configs");
        assert_eq!(settings.profile, "main");
        assert_eq!(settings.password, "");
        assert!(!settings.has_password);
        assert_eq!(
            remote_profile_path(&settings, WebDavRemoteLayout::Current),
            "configs/v1/db-v1/main"
        );
        assert_eq!(
            remote_file_url(&settings, WebDavRemoteLayout::Current, "/snapshots//db.sql").unwrap(),
            "https://dav.example.com/root/configs/v1/db-v1/main/snapshots/db.sql"
        );
    }
}
