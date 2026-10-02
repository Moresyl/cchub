use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::cloud_credentials::{self, CredentialStore, KeyringStore};
use crate::cloud_revision::{self, UploadReview, WriteCondition};
use crate::commands::extra_commands::{
    generate_sql_backup, get_json_app_setting, get_text_app_setting,
    import_cloud_backup_from_path_impl, set_json_app_setting,
};
use crate::db::DbState;

mod auto_sync;
pub(crate) use auto_sync::auto_upload;
pub use auto_sync::{run_auto_sync_if_enabled, spawn_auto_sync_loop};

const WEBDAV_SYNC_SETTINGS_KEY: &str = "webdav_sync_settings";
const WEBDAV_MANIFEST_FILE: &str = "manifest.json";
const WEBDAV_FORMAT: &str = "cchub-webdav-sync";
const WEBDAV_PROTOCOL_VERSION: u32 = 1;
const WEBDAV_DB_COMPAT_VERSION: u32 = 1;
const MAX_WEBDAV_SYNC_BYTES: usize = crate::cloud_transfer::SNAPSHOT_LIMIT;
const AUTO_SYNC_INTERVAL_SECS: u64 = 15 * 60;
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
    pub credential_scope: Option<String>,
    pub backup_encryption: crate::cloud_backup::BackupEncryption,
    pub remote_root: String,
    pub profile: String,
    pub auto_sync: bool,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    #[serde(skip)]
    pub proxy_url: Option<String>,
}

impl Default for WebDavSyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: String::new(),
            username: String::new(),
            password: String::new(),
            has_password: false,
            credential_scope: None,
            backup_encryption: Default::default(),
            remote_root: "cchub-sync".to_string(),
            profile: "default".to_string(),
            auto_sync: false,
            last_sync_at: None,
            last_error: None,
            proxy_url: None,
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
        masked.backup_encryption = masked.backup_encryption.masked();
        masked
    }
}

fn credential_scope(settings: &WebDavSyncSettings) -> String {
    cloud_credentials::scope("webdav_password", &settings.base_url, &settings.username)
}

fn backup_scope(settings: &WebDavSyncSettings) -> String {
    let location =
        manifest_url_for_layout(settings, WebDavRemoteLayout::Current).unwrap_or_else(|_| {
            serde_json::to_string(&(&settings.base_url, &settings.remote_root, &settings.profile))
                .unwrap_or_default()
        });
    cloud_credentials::scope("webdav_backup", &location, &settings.username)
}

fn same_remote(left: &WebDavSyncSettings, right: &WebDavSyncSettings) -> bool {
    credential_scope(left) == credential_scope(right)
        && left.remote_root == right.remote_root
        && left.profile == right.profile
}

fn prepare_connection(
    settings: &mut WebDavSyncSettings,
    existing: Option<&WebDavSyncSettings>,
    preserve: bool,
) -> Result<(), String> {
    settings.normalize();
    if let Some(existing) = existing {
        if preserve
            && settings.password.is_empty()
            && credential_scope(settings) == credential_scope(existing)
        {
            settings.password = existing.password.clone();
        }
        if settings.proxy_url.is_none() {
            settings.proxy_url = existing.proxy_url.clone();
        }
    }
    let mut validation = settings.clone();
    validation.enabled = true;
    validation.validate()
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
    pub encrypted: bool,
    pub upload_review: Option<UploadReview>,
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
    sha256: String,
    payload_format: String,
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
    let _ = read_settings_with_store(conn, &KeyringStore)?;
    Ok(())
}

pub fn read_settings(conn: &rusqlite::Connection) -> Result<WebDavSyncSettings, String> {
    read_settings_with_store(conn, &KeyringStore)
}

fn read_settings_with_store(
    conn: &rusqlite::Connection,
    credential_store: &impl CredentialStore,
) -> Result<WebDavSyncSettings, String> {
    let stored: Option<WebDavSyncSettings> = get_json_app_setting(conn, WEBDAV_SYNC_SETTINGS_KEY)?;
    let configured = stored.is_some();
    let mut settings = stored.unwrap_or_default();
    settings.normalize();
    let scope = credential_scope(&settings);
    if configured {
        if settings.credential_scope.is_none() {
            let legacy = if !settings.password.trim().is_empty() {
                Some(settings.password.clone())
            } else if settings.has_password {
                credential_store.get(WEBDAV_KEYRING_ACCOUNT)?
            } else {
                None
            };
            let secret = credential_store.get(&scope)?.or(legacy).unwrap_or_default();
            settings.password.clear();
            settings.has_password = !secret.trim().is_empty();
            settings.credential_scope = Some(scope.clone());
            cloud_credentials::save(credential_store, &scope, &secret, || {
                set_json_app_setting(conn, WEBDAV_SYNC_SETTINGS_KEY, &settings)
            })?;
            credential_store.delete(WEBDAV_KEYRING_ACCOUNT)?;
            settings.password = secret;
        } else {
            settings.password = credential_store.get(&scope)?.unwrap_or_default();
            settings.has_password = !settings.password.trim().is_empty();
        }
        let backup_scope = backup_scope(&settings);
        settings
            .backup_encryption
            .load(credential_store, &backup_scope)?;
    }
    settings.proxy_url =
        get_text_app_setting(conn, "proxy_url")?.filter(|value| !value.trim().is_empty());
    Ok(settings)
}

pub fn write_settings(
    conn: &rusqlite::Connection,
    incoming: WebDavSyncSettings,
    password_touched: bool,
) -> Result<WebDavSyncSettings, String> {
    write_settings_with_store(conn, incoming, password_touched, &KeyringStore)
}

fn write_settings_with_store(
    conn: &rusqlite::Connection,
    mut incoming: WebDavSyncSettings,
    password_touched: bool,
    credential_store: &impl CredentialStore,
) -> Result<WebDavSyncSettings, String> {
    let existing = read_settings_with_store(conn, credential_store)?;
    incoming.normalize();
    let scope = credential_scope(&incoming);
    if !password_touched && incoming.password.is_empty() {
        incoming.password = credential_store.get(&scope)?.unwrap_or_default();
    }
    let unchanged = same_remote(&incoming, &existing);
    incoming.last_sync_at = if unchanged {
        existing.last_sync_at
    } else {
        None
    };
    incoming.last_error = if unchanged { existing.last_error } else { None };
    incoming.proxy_url = existing.proxy_url;
    incoming.validate()?;
    let backup_scope = backup_scope(&incoming);
    incoming
        .backup_encryption
        .prepare_save(credential_store, &backup_scope, incoming.auto_sync)?;
    incoming.has_password = !incoming.password.trim().is_empty();
    let secret = std::mem::take(&mut incoming.password);
    incoming.credential_scope = Some(scope.clone());
    cloud_credentials::save(credential_store, &scope, &secret, || {
        cloud_credentials::save(
            credential_store,
            &backup_scope,
            &incoming.backup_encryption.passphrase,
            || set_json_app_setting(conn, WEBDAV_SYNC_SETTINGS_KEY, &incoming),
        )
    })?;
    Ok(incoming.masked_for_frontend())
}

mod status;
use status::{update_transfer_status, update_upload_status};

pub async fn test_connection(
    mut settings: WebDavSyncSettings,
    existing: Option<WebDavSyncSettings>,
    preserve_empty_password: bool,
) -> Result<(), String> {
    prepare_connection(&mut settings, existing.as_ref(), preserve_empty_password)?;

    let client = build_client(&settings)?;
    let response = crate::cloud_http::send(
        auth_request(
            client
                .request(method_propfind()?, normalize_base_url(&settings.base_url))
                .header("Depth", "0"),
            &settings,
        ),
        &credential_scope(&settings),
    )
    .await?;

    let status = response.status();
    if status.is_success() || status.as_u16() == 207 {
        crate::cloud_http::complete(&credential_scope(&settings));
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
            encrypted: false,
            upload_review: None,
            protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
            db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
            profile_path: Some(remote_profile_path(&settings, WebDavRemoteLayout::Current)),
        });
    }

    let _guard = webdav_sync_lock().lock().await;
    let client = build_client(&settings)?;
    match fetch_manifest_with_fallback(&client, &settings).await? {
        Some((manifest, layout, revision)) => {
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
                upload_review: Some(cloud_revision::review(
                    &KeyringStore,
                    &backup_scope(&settings),
                    Some(&revision),
                )?),
                encrypted: crate::cloud_backup::encrypted(
                    &manifest.payload_format,
                    &manifest.snapshot_path,
                ),
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
            encrypted: false,
            upload_review: Some(cloud_revision::review(
                &KeyringStore,
                &backup_scope(&settings),
                None,
            )?),
            protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
            db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
            profile_path: Some(remote_profile_path(&settings, WebDavRemoteLayout::Current)),
        }),
    }
}

pub async fn upload(
    db: &State<'_, DbState>,
    reviewed_revision: Option<String>,
) -> Result<WebDavRemoteInfo, String> {
    let _guard = webdav_sync_lock().lock().await;
    let _workflow = crate::cloud_sync::workflow_lock().lock().await;
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_settings(&conn)?
    };
    upload_for_settings(db, &settings, reviewed_revision).await
}

async fn upload_for_settings(
    db: &State<'_, DbState>,
    settings: &WebDavSyncSettings,
    reviewed_revision: Option<String>,
) -> Result<WebDavRemoteInfo, String> {
    match upload_inner(db, settings, reviewed_revision).await {
        Ok(info) => {
            crate::cloud_http::complete(&credential_scope(settings));
            Ok(info)
        }
        Err(error) => {
            if let Ok(conn) = db.0.lock() {
                let _ = update_transfer_status(&conn, settings, None, Some(error.clone()));
            }
            Err(error)
        }
    }
}

async fn upload_inner(
    db: &State<'_, DbState>,
    settings: &WebDavSyncSettings,
    reviewed_revision: Option<String>,
) -> Result<WebDavRemoteInfo, String> {
    if !settings.enabled {
        return Err("WebDAV sync is not enabled".to_string());
    }
    crate::cloud_backup::validate_new_passphrase(&settings.backup_encryption.passphrase)?;

    let client = build_client(&settings)?;
    let remote = fetch_manifest_with_fallback(&client, &settings).await?;
    if let Some((manifest, layout, _)) = &remote {
        validate_manifest_compatibility(manifest, *layout)?;
    }
    let scope = backup_scope(&settings);
    let condition = cloud_revision::authorize(
        &KeyringStore,
        &scope,
        remote.as_ref().map(|(_, _, revision)| revision),
        reviewed_revision.as_deref(),
    )?;
    // Legacy snapshots are kept in their original directory. A current manifest
    // is created conditionally so another client creating it wins safely.
    let condition = if remote
        .as_ref()
        .is_some_and(|(_, layout, _)| matches!(layout, WebDavRemoteLayout::Legacy))
    {
        WriteCondition::Absent
    } else {
        condition
    };

    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let sql_bytes = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        generate_sql_backup(&conn, &home).into_bytes()
    };
    if sql_bytes.len() > crate::cloud_backup::PLAINTEXT_LIMIT {
        return Err(format!(
            "WebDAV upload aborted because backup size exceeds {} MB",
            MAX_WEBDAV_SYNC_BYTES / (1024 * 1024)
        ));
    }

    let payload =
        crate::cloud_backup::seal_async(sql_bytes, settings.backup_encryption.passphrase.clone())
            .await?;
    let size_bytes = payload.len() as u64;

    ensure_remote_directories(&client, &settings, WebDavRemoteLayout::Current).await?;

    let created_at = chrono::Utc::now().to_rfc3339();
    let snapshot_name = format!("cchub-sync-{}.cchub-backup", uuid::Uuid::new_v4());
    let snapshot_path = format!("snapshots/{snapshot_name}");
    let snapshot_target = remote_file_url(&settings, WebDavRemoteLayout::Current, &snapshot_path)?;
    let digest = crate::cloud_transfer::sha256(&payload);
    upload_bytes(
        &client,
        &settings,
        &snapshot_target,
        "application/octet-stream",
        payload,
        &WriteCondition::Absent,
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
        sha256: digest,
        payload_format: crate::cloud_backup::PAYLOAD_FORMAT.into(),
        device_name: device_name.clone(),
        profile_path: Some(remote_profile_path(&settings, WebDavRemoteLayout::Current)),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    let manifest_url = manifest_url_for_layout(&settings, WebDavRemoteLayout::Current)?;
    let headers = upload_bytes(
        &client,
        &settings,
        &manifest_url,
        "application/json",
        manifest_bytes.clone(),
        &condition,
    )
    .await?;
    let written = if cloud_revision::strong_etag(&headers).is_some() {
        Some(cloud_revision::observe(
            &format!("{scope}:{}", WebDavRemoteLayout::Current.label()),
            &manifest_bytes,
            &headers,
        ))
    } else {
        fetch_manifest_for_layout(&client, &settings, WebDavRemoteLayout::Current)
            .await?
            .map(|(_, revision)| revision)
    };
    let written = cloud_revision::verify_written(&manifest_bytes, written)?;
    cloud_revision::accept(&KeyringStore, &scope, &written)?;

    {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        update_upload_status(&conn, &settings, created_at.clone())?;
    }

    Ok(WebDavRemoteInfo {
        exists: true,
        remote_url: manifest_url,
        snapshot_path: Some(snapshot_path),
        updated_at: Some(created_at),
        size_bytes: Some(size_bytes),
        app_version: Some(app_version),
        device_name: Some(device_name),
        layout: Some(WebDavRemoteLayout::Current.label().to_string()),
        compatible: true,
        encrypted: true,
        upload_review: Some(cloud_revision::review(
            &KeyringStore,
            &scope,
            Some(&written),
        )?),
        protocol_version: Some(WEBDAV_PROTOCOL_VERSION),
        db_compat_version: Some(WEBDAV_DB_COMPAT_VERSION),
        profile_path: manifest.profile_path,
    })
}

pub async fn download(db: &State<'_, DbState>, allow_plaintext: bool) -> Result<String, String> {
    let _guard = webdav_sync_lock().lock().await;
    let _workflow = crate::cloud_sync::workflow_lock().lock().await;
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_settings(&conn)?
    };
    match download_inner(db, &settings, allow_plaintext).await {
        Ok(message) => {
            crate::cloud_http::complete(&credential_scope(&settings));
            Ok(message)
        }
        Err(error) => {
            if let Ok(conn) = db.0.lock() {
                let _ = update_transfer_status(&conn, &settings, None, Some(error.clone()));
            }
            Err(error)
        }
    }
}

async fn download_inner(
    db: &State<'_, DbState>,
    settings: &WebDavSyncSettings,
    allow_plaintext: bool,
) -> Result<String, String> {
    if !settings.enabled {
        return Err("WebDAV sync is not enabled".to_string());
    }

    let client = build_client(&settings)?;
    let (manifest, layout, revision) = fetch_manifest_with_fallback(&client, &settings)
        .await?
        .ok_or_else(|| "No remote WebDAV sync manifest found".to_string())?;
    validate_manifest_compatibility(&manifest, layout)?;

    let snapshot_target = remote_file_url(&settings, layout, &manifest.snapshot_path)?;
    let response = crate::cloud_http::send(
        auth_request(client.get(snapshot_target), &settings),
        &credential_scope(settings),
    )
    .await?
    .error_for_status()
    .map_err(|error| format!("WebDAV snapshot download failed: {error}"))?;
    let bytes = crate::cloud_transfer::read_bounded(response, MAX_WEBDAV_SYNC_BYTES).await?;
    crate::cloud_transfer::verify_snapshot(
        &bytes,
        manifest.size_bytes,
        (!manifest.sha256.is_empty()).then_some(manifest.sha256.as_str()),
    )?;
    let bytes = crate::cloud_backup::open_for_restore(
        bytes,
        manifest.payload_format,
        manifest.snapshot_path,
        settings.backup_encryption.passphrase.clone(),
        allow_plaintext,
    )
    .await?;

    let temp_dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let temp_file = temp_dir.path().join("cchub-webdav-sync.sql");
    std::fs::write(&temp_file, bytes.as_slice()).map_err(|error| error.to_string())?;
    let message = import_cloud_backup_from_path_impl(db, &temp_file)?;

    {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        update_upload_status(&conn, &settings, chrono::Utc::now().to_rfc3339())?;
    }

    cloud_revision::accept(&KeyringStore, &backup_scope(&settings), &revision)?;

    Ok(message)
}

mod helpers;
use helpers::*;

#[cfg(test)]
mod credential_tests;

#[cfg(test)]
mod transfer_tests;

#[cfg(test)]
mod revision_tests;

#[cfg(test)]
mod rate_limit_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_credentials::tests::MemoryStore as MemoryCredentialStore;

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
            credential_scope: None,
            backup_encryption: Default::default(),
            remote_root: " /configs/ ".to_string(),
            profile: " main ".to_string(),
            auto_sync: false,
            proxy_url: None,
            last_sync_at: None,
            last_error: None,
        };

        let frontend =
            write_settings_with_store(&conn, settings, true, &store).expect("save settings");

        assert!(frontend.password.is_empty());
        assert!(frontend.has_password);
        assert_eq!(
            store.get(&credential_scope(&frontend)).unwrap(),
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
            credential_scope: None,
            backup_encryption: Default::default(),
            remote_root: "configs".to_string(),
            profile: "main".to_string(),
            auto_sync: false,
            proxy_url: None,
            last_sync_at: None,
            last_error: None,
        };

        let frontend =
            write_settings_with_store(&conn, cleared, true, &store).expect("delete password");

        assert!(!frontend.has_password);
        assert_eq!(store.get(&credential_scope(&frontend)).unwrap(), None);
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
            store.get(&credential_scope(&loaded)).unwrap(),
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
