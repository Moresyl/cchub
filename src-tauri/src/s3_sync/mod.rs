//! S3-compatible snapshot synchronization.
//!
//! The transport uses AWS Signature V4 and works with AWS S3 as well as
//! S3-compatible services such as MinIO and other private object stores.
//! Credentials are kept in the OS keyring; only non-sensitive settings are
//! persisted in the application database.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::cloud_credentials::{self, CredentialStore, KeyringStore};
use crate::commands::extra_commands::{
    generate_sql_backup, get_json_app_setting, get_text_app_setting, import_backup_from_path_impl,
    set_json_app_setting,
};
use crate::db::DbState;

const SETTINGS_KEY: &str = "s3_sync_settings";
const KEYRING_ACCOUNT: &str = "s3_sync_secret_access_key";
const FORMAT: &str = "cchub-s3-sync";
const PROTOCOL_VERSION: u32 = 1;
const DB_COMPAT_VERSION: u32 = 1;
const MAX_SYNC_BYTES: usize = crate::cloud_transfer::SNAPSHOT_LIMIT;
const MANIFEST_NAME: &str = "manifest.json";

static SYNC_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

fn sync_lock() -> &'static tokio::sync::Mutex<()> {
    SYNC_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct S3SyncSettings {
    pub enabled: bool,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    #[serde(skip_serializing)]
    pub secret_access_key: String,
    pub has_secret_access_key: bool,
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

impl Default for S3SyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            region: "us-east-1".to_string(),
            bucket: String::new(),
            access_key_id: String::new(),
            secret_access_key: String::new(),
            has_secret_access_key: false,
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

impl S3SyncSettings {
    pub fn normalize(&mut self) {
        self.endpoint = self.endpoint.trim().trim_end_matches('/').to_string();
        self.region = normalize_segment(&self.region, "us-east-1");
        self.bucket = self.bucket.trim().to_string();
        self.access_key_id = self.access_key_id.trim().to_string();
        self.remote_root = normalize_segment(&self.remote_root, "cchub-sync");
        self.profile = normalize_segment(&self.profile, "default");
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if self.bucket.is_empty() {
            return Err("S3 bucket is required".to_string());
        }
        if self.access_key_id.is_empty() {
            return Err("S3 access key ID is required".to_string());
        }
        if self.secret_access_key.trim().is_empty() {
            return Err("S3 secret access key is required".to_string());
        }
        if self.region.is_empty() {
            return Err("S3 region is required".to_string());
        }
        if !self.endpoint.is_empty() {
            cloud_credentials::validate_url(&self.endpoint)?;
        }
        Ok(())
    }

    pub fn masked_for_frontend(&self) -> Self {
        let mut masked = self.clone();
        masked.has_secret_access_key =
            masked.has_secret_access_key || !masked.secret_access_key.is_empty();
        masked.secret_access_key.clear();
        masked.backup_encryption = masked.backup_encryption.masked();
        masked
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S3RemoteInfo {
    pub exists: bool,
    pub remote_url: String,
    pub snapshot_path: Option<String>,
    pub updated_at: Option<String>,
    pub size_bytes: Option<u64>,
    pub compatible: bool,
    pub encrypted: bool,
    pub protocol_version: Option<u32>,
    pub db_compat_version: Option<u32>,
    pub profile_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct S3Manifest {
    format: String,
    protocol_version: u32,
    db_compat_version: u32,
    app_version: String,
    created_at: String,
    snapshot_path: String,
    size_bytes: u64,
    sha256: String,
    payload_format: String,
    device_name: String,
    profile_path: String,
}

fn credential_scope(settings: &S3SyncSettings) -> String {
    cloud_credentials::scope("s3_secret", &endpoint(settings), &settings.access_key_id)
}

fn backup_scope(settings: &S3SyncSettings) -> String {
    let location = object_url(settings, &object_key(settings, MANIFEST_NAME))
        .map(|url| url.to_string())
        .unwrap_or_else(|_| {
            serde_json::to_string(&(
                &settings.endpoint,
                &settings.region,
                &settings.bucket,
                &settings.remote_root,
                &settings.profile,
            ))
            .unwrap_or_default()
        });
    cloud_credentials::scope("s3_backup", &location, &settings.access_key_id)
}

fn same_remote(left: &S3SyncSettings, right: &S3SyncSettings) -> bool {
    credential_scope(left) == credential_scope(right)
        && left.bucket == right.bucket
        && left.region == right.region
        && left.remote_root == right.remote_root
        && left.profile == right.profile
}

fn prepare_connection(
    settings: &mut S3SyncSettings,
    existing: Option<&S3SyncSettings>,
    preserve: bool,
) -> Result<(), String> {
    settings.normalize();
    if let Some(existing) = existing {
        if preserve
            && settings.secret_access_key.is_empty()
            && credential_scope(settings) == credential_scope(existing)
        {
            settings.secret_access_key = existing.secret_access_key.clone();
        }
        if settings.proxy_url.is_none() {
            settings.proxy_url = existing.proxy_url.clone();
        }
    }
    let mut validation = settings.clone();
    validation.enabled = true;
    validation.validate()
}

pub fn migrate_secret_to_keyring(conn: &rusqlite::Connection) -> Result<(), String> {
    let _ = read_settings_with_store(conn, &KeyringStore)?;
    Ok(())
}

pub fn read_settings(conn: &rusqlite::Connection) -> Result<S3SyncSettings, String> {
    read_settings_with_store(conn, &KeyringStore)
}

fn read_settings_with_store(
    conn: &rusqlite::Connection,
    store: &impl CredentialStore,
) -> Result<S3SyncSettings, String> {
    let stored: Option<S3SyncSettings> = get_json_app_setting(conn, SETTINGS_KEY)?;
    let configured = stored.is_some();
    let mut settings = stored.unwrap_or_default();
    settings.normalize();
    let scope = credential_scope(&settings);
    if configured {
        if settings.credential_scope.is_none() {
            let legacy = if !settings.secret_access_key.trim().is_empty() {
                Some(settings.secret_access_key.clone())
            } else {
                store.get(KEYRING_ACCOUNT)?
            };
            let secret = store.get(&scope)?.or(legacy).unwrap_or_default();
            settings.secret_access_key.clear();
            settings.has_secret_access_key = !secret.trim().is_empty();
            settings.credential_scope = Some(scope.clone());
            cloud_credentials::save(store, &scope, &secret, || {
                set_json_app_setting(conn, SETTINGS_KEY, &settings)
            })?;
            store.delete(KEYRING_ACCOUNT)?;
            settings.secret_access_key = secret;
        } else {
            settings.secret_access_key = store.get(&scope)?.unwrap_or_default();
            settings.has_secret_access_key = !settings.secret_access_key.trim().is_empty();
        }
    }
    if configured {
        let backup_scope = backup_scope(&settings);
        settings.backup_encryption.load(store, &backup_scope)?;
    }
    settings.proxy_url =
        get_text_app_setting(conn, "proxy_url")?.filter(|value| !value.trim().is_empty());
    Ok(settings)
}

pub fn write_settings(
    conn: &rusqlite::Connection,
    incoming: S3SyncSettings,
    secret_touched: bool,
) -> Result<S3SyncSettings, String> {
    write_settings_with_store(conn, incoming, secret_touched, &KeyringStore)
}

fn write_settings_with_store(
    conn: &rusqlite::Connection,
    mut incoming: S3SyncSettings,
    secret_touched: bool,
    store: &impl CredentialStore,
) -> Result<S3SyncSettings, String> {
    let existing = read_settings_with_store(conn, store)?;
    incoming.normalize();
    let scope = credential_scope(&incoming);
    if !secret_touched && incoming.secret_access_key.is_empty() {
        incoming.secret_access_key = store.get(&scope)?.unwrap_or_default();
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
        .prepare_save(store, &backup_scope, incoming.auto_sync)?;
    incoming.has_secret_access_key = !incoming.secret_access_key.trim().is_empty();
    let secret = std::mem::take(&mut incoming.secret_access_key);
    incoming.credential_scope = Some(scope.clone());
    cloud_credentials::save(store, &scope, &secret, || {
        cloud_credentials::save(
            store,
            &backup_scope,
            &incoming.backup_encryption.passphrase,
            || set_json_app_setting(conn, SETTINGS_KEY, &incoming),
        )
    })?;
    Ok(incoming.masked_for_frontend())
}

fn normalize_segment(value: &str, fallback: &str) -> String {
    let normalized = value.trim().trim_matches('/');
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized.to_string()
    }
}

fn endpoint(settings: &S3SyncSettings) -> String {
    if settings.endpoint.is_empty() {
        format!("https://s3.{}.amazonaws.com", settings.region)
    } else if settings.endpoint.starts_with("http://") || settings.endpoint.starts_with("https://")
    {
        settings.endpoint.clone()
    } else {
        format!("https://{}", settings.endpoint)
    }
}

fn profile_path(settings: &S3SyncSettings) -> String {
    format!(
        "{}/v{}/db-v{}/{}",
        settings.remote_root, PROTOCOL_VERSION, DB_COMPAT_VERSION, settings.profile
    )
}

fn object_key(settings: &S3SyncSettings, name: &str) -> String {
    format!(
        "{}/{}",
        profile_path(settings),
        name.trim_start_matches('/')
    )
}

fn object_url(settings: &S3SyncSettings, key: &str) -> Result<url::Url, String> {
    let mut url = cloud_credentials::validate_url(&endpoint(settings))?;
    let mut path = format!(
        "{}/{}/",
        url.path().trim_end_matches('/'),
        settings.bucket.trim_matches('/')
    );
    path.push_str(key.trim_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    crate::cloud_transfer::sha256(bytes)
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut block = [0u8; 64];
    if key.len() > block.len() {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = [0u8; 64];
    let mut outer = [0u8; 64];
    for index in 0..64 {
        inner[index] = block[index] ^ 0x36;
        outer[index] = block[index] ^ 0x5c;
    }
    let mut inner_hash = Sha256::new();
    inner_hash.update(inner);
    inner_hash.update(message);
    let mut outer_hash = Sha256::new();
    outer_hash.update(outer);
    outer_hash.update(inner_hash.finalize());
    outer_hash.finalize().to_vec()
}

fn signed_request(
    client: &reqwest::Client,
    settings: &S3SyncSettings,
    method: reqwest::Method,
    key: &str,
    body: Vec<u8>,
) -> Result<reqwest::RequestBuilder, String> {
    let url = object_url(settings, key)?;
    let host = host_header(&url)?;
    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let short_date = now.format("%Y%m%d").to_string();
    let payload_hash = sha256_hex(&body);
    let canonical_uri = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    let canonical_headers =
        format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";
    let canonical_request = format!(
        "{}\n{}\n\n{}\n{}\n{}",
        method.as_str(),
        canonical_uri,
        canonical_headers,
        signed_headers,
        payload_hash
    );
    let scope = format!("{short_date}/{}/{}/aws4_request", settings.region, "s3");
    let credential_scope = scope.clone();
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let k_date = hmac_sha256(
        format!("AWS4{}", settings.secret_access_key).as_bytes(),
        short_date.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, settings.region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex(&hmac_sha256(&k_signing, string_to_sign.as_bytes()));
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        settings.access_key_id, scope, signed_headers, signature
    );
    Ok(client
        .request(method, url)
        .header("host", host)
        .header("x-amz-date", amz_date)
        .header("x-amz-content-sha256", payload_hash)
        .header("Authorization", authorization)
        .body(body))
}

fn host_header(url: &url::Url) -> Result<String, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "S3 endpoint has no host".to_string())?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

fn client(settings: &S3SyncSettings) -> Result<reqwest::Client, String> {
    crate::shared::http_client::build_http_client(
        settings.proxy_url.as_deref(),
        Some(&format!("CCHub/{} S3", env!("CARGO_PKG_VERSION"))),
        Duration::from_secs(30),
    )
    .map_err(|error| format!("Failed to build S3 HTTP client: {error}"))
}

async fn request_object(
    settings: &S3SyncSettings,
    method: reqwest::Method,
    key: &str,
    body: Vec<u8>,
) -> Result<reqwest::Response, String> {
    let request = signed_request(&client(settings)?, settings, method, key, body)?;
    request
        .send()
        .await
        .map_err(|error| format!("S3 request failed: {error}"))
}

async fn get_object(settings: &S3SyncSettings, key: &str) -> Result<Option<Vec<u8>>, String> {
    let response = request_object(settings, reqwest::Method::GET, key, Vec::new()).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response
        .error_for_status()
        .map_err(|error| format!("S3 download failed: {error}"))?;
    let limit = if key == object_key(settings, MANIFEST_NAME) {
        crate::cloud_transfer::MANIFEST_LIMIT
    } else {
        MAX_SYNC_BYTES
    };
    Ok(Some(
        crate::cloud_transfer::read_bounded(response, limit).await?,
    ))
}

async fn put_object(settings: &S3SyncSettings, key: &str, body: Vec<u8>) -> Result<(), String> {
    request_object(settings, reqwest::Method::PUT, key, body)
        .await?
        .error_for_status()
        .map_err(|error| format!("S3 upload failed: {error}"))?;
    Ok(())
}

fn device_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "cchub".to_string())
        .chars()
        .take(64)
        .collect()
}

fn validate_manifest(manifest: &S3Manifest) -> Result<(), String> {
    crate::cloud_backup::validate_format(&manifest.payload_format)?;
    if manifest.format != FORMAT {
        return Err("S3 manifest format is incompatible".to_string());
    }
    if manifest.protocol_version != PROTOCOL_VERSION {
        return Err("S3 manifest protocol version is incompatible".to_string());
    }
    if manifest.db_compat_version != DB_COMPAT_VERSION {
        return Err("S3 manifest database version is incompatible".to_string());
    }
    crate::cloud_transfer::validate_snapshot_path(&manifest.snapshot_path)?;
    crate::cloud_transfer::validate_size_and_digest(manifest.size_bytes, Some(&manifest.sha256))?;
    Ok(())
}

pub async fn test_connection(
    mut settings: S3SyncSettings,
    existing: Option<S3SyncSettings>,
    preserve_secret: bool,
) -> Result<(), String> {
    prepare_connection(&mut settings, existing.as_ref(), preserve_secret)?;
    let response = request_object(
        &settings,
        reqwest::Method::HEAD,
        &object_key(&settings, MANIFEST_NAME),
        Vec::new(),
    )
    .await?;
    if response.status().is_success()
        || response.status() == reqwest::StatusCode::NOT_FOUND
        || response.status() == reqwest::StatusCode::NO_CONTENT
    {
        return Ok(());
    }
    Err(format!("S3 server returned {}", response.status()))
}

pub async fn fetch_remote_info(db: &State<'_, DbState>) -> Result<S3RemoteInfo, String> {
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        read_settings(&conn)?
    };
    let remote_url = object_url(&settings, &object_key(&settings, MANIFEST_NAME))?.to_string();
    let profile = profile_path(&settings);
    if !settings.enabled {
        return Ok(S3RemoteInfo {
            exists: false,
            remote_url,
            snapshot_path: None,
            updated_at: None,
            size_bytes: None,
            compatible: true,
            encrypted: false,
            protocol_version: Some(PROTOCOL_VERSION),
            db_compat_version: Some(DB_COMPAT_VERSION),
            profile_path: profile,
        });
    }
    let _guard = sync_lock().lock().await;
    let Some(bytes) = get_object(&settings, &object_key(&settings, MANIFEST_NAME)).await? else {
        return Ok(S3RemoteInfo {
            exists: false,
            remote_url,
            snapshot_path: None,
            updated_at: None,
            size_bytes: None,
            compatible: true,
            encrypted: false,
            protocol_version: Some(PROTOCOL_VERSION),
            db_compat_version: Some(DB_COMPAT_VERSION),
            profile_path: profile,
        });
    };
    let manifest: S3Manifest =
        serde_json::from_slice(&bytes).map_err(|error| format!("Invalid S3 manifest: {error}"))?;
    let compatible = validate_manifest(&manifest).is_ok();
    let encrypted =
        crate::cloud_backup::encrypted(&manifest.payload_format, &manifest.snapshot_path);
    Ok(S3RemoteInfo {
        exists: true,
        remote_url,
        snapshot_path: Some(manifest.snapshot_path),
        updated_at: Some(manifest.created_at),
        size_bytes: Some(manifest.size_bytes),
        compatible,
        encrypted,
        protocol_version: Some(manifest.protocol_version),
        db_compat_version: Some(manifest.db_compat_version),
        profile_path: manifest.profile_path,
    })
}

pub async fn upload(db: &State<'_, DbState>) -> Result<S3RemoteInfo, String> {
    let _guard = sync_lock().lock().await;
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let settings = read_settings(&conn)?;
        if !settings.enabled {
            return Err("S3 sync is not enabled".to_string());
        }
        crate::cloud_backup::validate_new_passphrase(&settings.backup_encryption.passphrase)?;
        settings
    };
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let sql_bytes = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        generate_sql_backup(&conn, &home).into_bytes()
    };
    if sql_bytes.len() > crate::cloud_backup::PLAINTEXT_LIMIT {
        return Err("S3 upload aborted because backup exceeds the 15 MB safety limit".to_string());
    }
    let payload =
        crate::cloud_backup::seal_async(sql_bytes, settings.backup_encryption.passphrase.clone())
            .await?;
    let size_bytes = payload.len() as u64;
    let digest = sha256_hex(&payload);
    let snapshot_path = format!("snapshots/cchub-sync-{}.cchub-backup", uuid::Uuid::new_v4());
    put_object(&settings, &object_key(&settings, &snapshot_path), payload).await?;
    let created_at = Utc::now().to_rfc3339();
    let manifest = S3Manifest {
        format: FORMAT.to_string(),
        protocol_version: PROTOCOL_VERSION,
        db_compat_version: DB_COMPAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: created_at.clone(),
        snapshot_path: snapshot_path.clone(),
        size_bytes,
        sha256: digest,
        payload_format: crate::cloud_backup::PAYLOAD_FORMAT.into(),
        device_name: device_name(),
        profile_path: profile_path(&settings),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    put_object(
        &settings,
        &object_key(&settings, MANIFEST_NAME),
        manifest_bytes,
    )
    .await?;
    let info = S3RemoteInfo {
        exists: true,
        remote_url: object_url(&settings, &object_key(&settings, MANIFEST_NAME))?.to_string(),
        snapshot_path: Some(snapshot_path),
        updated_at: Some(created_at.clone()),
        size_bytes: Some(size_bytes),
        compatible: true,
        encrypted: true,
        protocol_version: Some(PROTOCOL_VERSION),
        db_compat_version: Some(DB_COMPAT_VERSION),
        profile_path: profile_path(&settings),
    };
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    update_upload_status(&conn, &settings, created_at)?;
    Ok(info)
}

pub async fn download(db: &State<'_, DbState>, allow_plaintext: bool) -> Result<String, String> {
    let _guard = sync_lock().lock().await;
    let settings = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let settings = read_settings(&conn)?;
        if !settings.enabled {
            return Err("S3 sync is not enabled".to_string());
        }
        settings
    };
    let manifest_bytes = get_object(&settings, &object_key(&settings, MANIFEST_NAME))
        .await?
        .ok_or("No remote S3 sync manifest found")?;
    let manifest: S3Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("Invalid S3 manifest: {error}"))?;
    validate_manifest(&manifest)?;
    let bytes = get_object(&settings, &object_key(&settings, &manifest.snapshot_path))
        .await?
        .ok_or("Remote S3 snapshot is missing")?;
    crate::cloud_transfer::verify_snapshot(&bytes, manifest.size_bytes, Some(&manifest.sha256))?;
    let bytes = crate::cloud_backup::open_for_restore(
        bytes,
        manifest.payload_format,
        manifest.snapshot_path,
        settings.backup_encryption.passphrase.clone(),
        allow_plaintext,
    )
    .await?;
    let temp_dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let temp_file = temp_dir.path().join("cchub-s3-sync.sql");
    std::fs::write(&temp_file, bytes.as_slice()).map_err(|error| error.to_string())?;
    let message = import_backup_from_path_impl(db, &temp_file)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut saved = settings.masked_for_frontend();
    saved.last_sync_at = Some(Utc::now().to_rfc3339());
    saved.last_error = None;
    set_json_app_setting(&conn, SETTINGS_KEY, &saved)?;
    Ok(message)
}

pub fn update_error(conn: &rusqlite::Connection, error: &str) -> Result<(), String> {
    let mut settings = read_settings(conn)?;
    settings.last_error = Some(error.to_string());
    settings.secret_access_key.clear();
    set_json_app_setting(conn, SETTINGS_KEY, &settings)
}

fn update_upload_status(
    conn: &rusqlite::Connection,
    expected: &S3SyncSettings,
    synced_at: String,
) -> Result<(), String> {
    let mut current: S3SyncSettings = get_json_app_setting(conn, SETTINGS_KEY)?.unwrap_or_default();
    current.normalize();
    if same_remote(&current, expected) {
        current.last_sync_at = Some(synced_at);
        current.last_error = None;
        set_json_app_setting(conn, SETTINGS_KEY, &current.masked_for_frontend())?;
    }
    Ok(())
}

pub fn spawn_auto_sync_loop(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            let db = app_handle.state::<DbState>();
            let enabled =
                db.0.lock()
                    .ok()
                    .and_then(|conn| read_settings(&conn).ok())
                    .is_some_and(|settings| settings.enabled && settings.auto_sync);
            if !enabled {
                continue;
            }
            let result = upload(&db).await;
            let payload = serde_json::json!({
                "status": if result.is_ok() { "success" } else { "error" },
                "message": result.as_ref().map(|_| "S3 sync completed").unwrap_or("S3 sync failed"),
                "error": result.err(),
            });
            let _ = app_handle.emit("s3-sync-status-updated", payload);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        hmac_sha256, host_header, object_key, profile_path, sha256_hex, S3Manifest, S3SyncSettings,
    };

    #[test]
    fn object_key_is_versioned_and_profile_scoped() {
        let settings = S3SyncSettings {
            remote_root: "team".to_string(),
            profile: "work".to_string(),
            ..Default::default()
        };
        assert_eq!(
            object_key(&settings, "manifest.json"),
            "team/v1/db-v1/work/manifest.json"
        );
        assert_eq!(profile_path(&settings), "team/v1/db-v1/work");
    }

    #[test]
    fn hmac_and_hash_are_deterministic() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(hmac_sha256(b"key", b"message").len(), 32);
        assert_eq!(
            hmac_sha256(b"key", b"message"),
            hmac_sha256(b"key", b"message")
        );
    }

    #[test]
    fn manifest_defaults_are_safe_to_deserialize() {
        let manifest: S3Manifest = serde_json::from_str("{}").expect("manifest should deserialize");
        assert!(manifest.snapshot_path.is_empty());
    }

    #[test]
    fn host_header_keeps_minio_port_for_sigv4() {
        let url = url::Url::parse("http://127.0.0.1:9000").unwrap();
        assert_eq!(host_header(&url).unwrap(), "127.0.0.1:9000");
    }
}

#[cfg(test)]
mod credential_tests;

#[cfg(test)]
mod transfer_tests;
