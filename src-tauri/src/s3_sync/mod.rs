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

use crate::commands::extra_commands::{
    generate_sql_backup, get_json_app_setting, get_text_app_setting, import_backup_from_path_impl,
    set_json_app_setting,
};
use crate::db::DbState;

const SETTINGS_KEY: &str = "s3_sync_settings";
const KEYRING_SERVICE: &str = "cchub";
const KEYRING_ACCOUNT: &str = "s3_sync_secret_access_key";
const FORMAT: &str = "cchub-s3-sync";
const PROTOCOL_VERSION: u32 = 1;
const DB_COMPAT_VERSION: u32 = 1;
const MAX_SYNC_BYTES: usize = 15 * 1024 * 1024;
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
            let url = url::Url::parse(&self.endpoint)
                .map_err(|_| "S3 endpoint must be a valid HTTP(S) URL".to_string())?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err("S3 endpoint must use http or https".to_string());
            }
        }
        Ok(())
    }

    pub fn masked_for_frontend(&self) -> Self {
        let mut masked = self.clone();
        masked.has_secret_access_key =
            masked.has_secret_access_key || !masked.secret_access_key.is_empty();
        masked.secret_access_key.clear();
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
    device_name: String,
    profile_path: String,
}

struct KeyringStore;

impl KeyringStore {
    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map_err(|error| format!("Failed to open S3 credential store: {error}"))
    }

    fn get(&self) -> Result<Option<String>, String> {
        match self.entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!("Failed to read S3 secret access key: {error}")),
        }
    }

    fn set(&self, secret: &str) -> Result<(), String> {
        self.entry()?
            .set_password(secret)
            .map_err(|error| format!("Failed to save S3 secret access key: {error}"))
    }

    fn delete(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("Failed to delete S3 secret access key: {error}")),
        }
    }
}

pub fn migrate_secret_to_keyring(conn: &rusqlite::Connection) -> Result<(), String> {
    let mut settings: S3SyncSettings =
        get_json_app_setting(conn, SETTINGS_KEY)?.unwrap_or_default();
    if !settings.secret_access_key.trim().is_empty() {
        KeyringStore.set(settings.secret_access_key.trim())?;
        settings.has_secret_access_key = true;
        settings.secret_access_key.clear();
        set_json_app_setting(conn, SETTINGS_KEY, &settings)?;
    } else if let Some(secret) = KeyringStore.get()? {
        settings.has_secret_access_key = !secret.is_empty();
        set_json_app_setting(conn, SETTINGS_KEY, &settings)?;
    }
    Ok(())
}

pub fn read_settings(conn: &rusqlite::Connection) -> Result<S3SyncSettings, String> {
    let mut settings: S3SyncSettings =
        get_json_app_setting(conn, SETTINGS_KEY)?.unwrap_or_default();
    if let Some(secret) = KeyringStore.get()? {
        settings.secret_access_key = secret;
        settings.has_secret_access_key = true;
    }
    settings.proxy_url =
        get_text_app_setting(conn, "proxy_url")?.filter(|value| !value.trim().is_empty());
    Ok(settings)
}

pub fn write_settings(
    conn: &rusqlite::Connection,
    mut incoming: S3SyncSettings,
    secret_touched: bool,
) -> Result<S3SyncSettings, String> {
    let existing = read_settings(conn).unwrap_or_default();
    incoming.normalize();
    if !secret_touched && incoming.secret_access_key.is_empty() {
        incoming.secret_access_key = existing.secret_access_key;
    }
    incoming.proxy_url = existing.proxy_url;
    incoming.validate()?;
    if incoming.secret_access_key.trim().is_empty() {
        KeyringStore.delete()?;
        incoming.has_secret_access_key = false;
    } else {
        KeyringStore.set(incoming.secret_access_key.trim())?;
        incoming.has_secret_access_key = true;
    }
    incoming.secret_access_key.clear();
    set_json_app_setting(conn, SETTINGS_KEY, &incoming)?;
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
    let mut url = url::Url::parse(&endpoint(settings)).map_err(|error| error.to_string())?;
    let mut path = format!("/{}/", settings.bucket.trim_matches('/'));
    path.push_str(key.trim_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
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
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read S3 response: {error}"))?;
    if bytes.len() > MAX_SYNC_BYTES {
        return Err("Remote S3 snapshot exceeds the 15 MB safety limit".to_string());
    }
    Ok(Some(bytes.to_vec()))
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
    if manifest.format != FORMAT {
        return Err("S3 manifest format is incompatible".to_string());
    }
    if manifest.protocol_version != PROTOCOL_VERSION {
        return Err("S3 manifest protocol version is incompatible".to_string());
    }
    if manifest.db_compat_version != DB_COMPAT_VERSION {
        return Err("S3 manifest database version is incompatible".to_string());
    }
    if manifest.snapshot_path.trim().is_empty() || manifest.sha256.len() != 64 {
        return Err("S3 manifest is invalid".to_string());
    }
    Ok(())
}

pub async fn test_connection(
    mut settings: S3SyncSettings,
    existing: Option<S3SyncSettings>,
    preserve_secret: bool,
) -> Result<(), String> {
    if preserve_secret && settings.secret_access_key.is_empty() {
        settings.secret_access_key = existing
            .as_ref()
            .and_then(|value| {
                if value.secret_access_key.is_empty() {
                    None
                } else {
                    Some(value.secret_access_key.clone())
                }
            })
            .unwrap_or_default();
    }
    if settings.proxy_url.is_none() {
        settings.proxy_url = existing.and_then(|value| value.proxy_url);
    }
    settings.enabled = true;
    settings.normalize();
    settings.validate()?;
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
            protocol_version: Some(PROTOCOL_VERSION),
            db_compat_version: Some(DB_COMPAT_VERSION),
            profile_path: profile,
        });
    };
    let manifest: S3Manifest =
        serde_json::from_slice(&bytes).map_err(|error| format!("Invalid S3 manifest: {error}"))?;
    let compatible = validate_manifest(&manifest).is_ok();
    Ok(S3RemoteInfo {
        exists: true,
        remote_url,
        snapshot_path: Some(manifest.snapshot_path),
        updated_at: Some(manifest.created_at),
        size_bytes: Some(manifest.size_bytes),
        compatible,
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
        settings
    };
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let sql_bytes = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        generate_sql_backup(&conn, &home).into_bytes()
    };
    if sql_bytes.len() > MAX_SYNC_BYTES {
        return Err("S3 upload aborted because backup exceeds the 15 MB safety limit".to_string());
    }
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let snapshot_path = format!("snapshots/cchub-sync-{timestamp}.sql");
    put_object(
        &settings,
        &object_key(&settings, &snapshot_path),
        sql_bytes.clone(),
    )
    .await?;
    let created_at = Utc::now().to_rfc3339();
    let manifest = S3Manifest {
        format: FORMAT.to_string(),
        protocol_version: PROTOCOL_VERSION,
        db_compat_version: DB_COMPAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: created_at.clone(),
        snapshot_path: snapshot_path.clone(),
        size_bytes: sql_bytes.len() as u64,
        sha256: sha256_hex(&sql_bytes),
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
        size_bytes: Some(sql_bytes.len() as u64),
        compatible: true,
        protocol_version: Some(PROTOCOL_VERSION),
        db_compat_version: Some(DB_COMPAT_VERSION),
        profile_path: profile_path(&settings),
    };
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut saved = settings.masked_for_frontend();
    saved.last_sync_at = Some(created_at);
    saved.last_error = None;
    set_json_app_setting(&conn, SETTINGS_KEY, &saved)?;
    Ok(info)
}

pub async fn download(db: &State<'_, DbState>) -> Result<String, String> {
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
    if manifest.size_bytes as usize > MAX_SYNC_BYTES {
        return Err("Remote S3 snapshot exceeds the 15 MB safety limit".to_string());
    }
    let bytes = get_object(&settings, &object_key(&settings, &manifest.snapshot_path))
        .await?
        .ok_or("Remote S3 snapshot is missing")?;
    if bytes.len() as u64 != manifest.size_bytes || sha256_hex(&bytes) != manifest.sha256 {
        return Err("Remote S3 snapshot integrity verification failed".to_string());
    }
    let temp_dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let temp_file = temp_dir.path().join("cchub-s3-sync.sql");
    std::fs::write(&temp_file, bytes).map_err(|error| error.to_string())?;
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
