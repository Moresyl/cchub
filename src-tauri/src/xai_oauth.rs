//! xAI OAuth device-flow authentication with multi-account support.
//!
//! Tokens are kept in the OS credential store when available.  The JSON file only
//! contains non-secret account metadata and falls back to an encrypted-store
//! implementation provided by the operating system when keyring access succeeds.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::{Mutex, RwLock};

mod storage;

const XAI_ISSUER: &str = "https://auth.x.ai";
const XAI_DISCOVERY_URL: &str = "https://auth.x.ai/.well-known/openid-configuration";
const XAI_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const XAI_SCOPE: &str = "openid profile email offline_access grok-cli:access api:access";
const XAI_USER_AGENT: &str = "CCHub-xAI-OAuth";
const KEYRING_SERVICE: &str = "CCHub xAI OAuth";
const TOKEN_REFRESH_BUFFER_MS: i64 = 60_000;
const DEFAULT_TOKEN_LIFETIME_SECS: i64 = 3_600;
const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;
const MAX_POLL_INTERVAL_SECS: u64 = 60;
const MAX_DEVICE_LIFETIME_SECS: u64 = 24 * 60 * 60;
const MAX_RESPONSE_BYTES: usize = 128 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum XaiOAuthError {
    #[error("xAI authorization is pending")]
    AuthorizationPending,
    #[error("xAI authorization was denied")]
    AccessDenied,
    #[error("xAI device code expired")]
    ExpiredToken,
    #[error("xAI token exchange failed: {0}")]
    TokenFetchFailed(String),
    #[error("xAI refresh token is invalid or expired")]
    RefreshTokenInvalid,
    #[error("xAI account requires re-authentication: {0}")]
    ReauthRequired(String),
    #[error("xAI network error: {0}")]
    Network(String),
    #[error("xAI response parse error: {0}")]
    Parse(String),
    #[error("xAI storage error: {0}")]
    Io(String),
    #[error("xAI account not found: {0}")]
    AccountNotFound(String),
}

impl From<reqwest::Error> for XaiOAuthError {
    fn from(error: reqwest::Error) -> Self {
        Self::Network(error.to_string())
    }
}

impl From<std::io::Error> for XaiOAuthError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XaiDeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XaiAccount {
    pub id: String,
    pub login: String,
    pub authenticated_at: i64,
    pub requires_reauth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XaiAuthStatus {
    pub accounts: Vec<XaiAccount>,
    pub default_account_id: Option<String>,
    pub authenticated: bool,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XaiModel {
    pub id: String,
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DiscoveryDocument {
    issuer: String,
    token_endpoint: String,
    device_authorization_endpoint: String,
}

#[derive(Debug, Clone)]
struct OAuthEndpoints {
    token_endpoint: String,
    device_authorization_endpoint: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DeviceCodePayload {
    device_code: String,
    user_code: String,
    #[serde(alias = "verification_url")]
    verification_uri: Option<String>,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default = "default_poll_interval")]
    interval: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct TokenPayload {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TokenClaims {
    #[serde(default)]
    sub: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    preferred_username: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountData {
    id: String,
    login: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    authenticated_at: i64,
    #[serde(default)]
    requires_reauth: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    accounts: HashMap<String, AccountData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_account_id: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedToken {
    value: String,
    expires_at_ms: i64,
}

impl CachedToken {
    fn usable(&self) -> bool {
        self.expires_at_ms - chrono::Utc::now().timestamp_millis() > TOKEN_REFRESH_BUFFER_MS
    }
}

#[derive(Debug, Clone)]
struct PendingDeviceCode {
    token_endpoint: String,
    expires_at_ms: i64,
    interval_secs: u64,
    next_poll_at_ms: i64,
}

pub struct XaiOAuthManager {
    accounts: RwLock<HashMap<String, AccountData>>,
    default_account_id: RwLock<Option<String>>,
    access_tokens: RwLock<HashMap<String, CachedToken>>,
    refresh_locks: RwLock<HashMap<String, Arc<Mutex<()>>>>,
    pending: RwLock<HashMap<String, PendingDeviceCode>>,
    endpoints: RwLock<Option<OAuthEndpoints>>,
    mutation_lock: Mutex<()>,
    http_client: reqwest::Client,
    storage_path: PathBuf,
}

pub struct XaiOAuthState(pub Arc<XaiOAuthManager>);

impl XaiOAuthManager {
    pub fn new(storage_path: PathBuf, proxy_url: Option<String>) -> Self {
        let http_client = crate::shared::http_client::build_http_client(
            proxy_url.as_deref(),
            Some(XAI_USER_AGENT),
            std::time::Duration::from_secs(30),
        )
        .unwrap_or_else(|_| crate::shared::http_client::default_http_client());
        let manager = Self {
            accounts: RwLock::new(HashMap::new()),
            default_account_id: RwLock::new(None),
            access_tokens: RwLock::new(HashMap::new()),
            refresh_locks: RwLock::new(HashMap::new()),
            pending: RwLock::new(HashMap::new()),
            endpoints: RwLock::new(None),
            mutation_lock: Mutex::new(()),
            http_client,
            storage_path,
        };
        if let Err(error) = manager.load_from_disk_sync() {
            crate::utils::append_runtime_log(
                "warn",
                "xai_oauth",
                &format!("Failed to load xAI OAuth accounts: {error}"),
            );
        }
        manager
    }

    pub async fn start_device_flow(&self) -> Result<XaiDeviceCodeResponse, XaiOAuthError> {
        let endpoints = self.discover_endpoints().await?;
        let response = self
            .http_client
            .post(&endpoints.device_authorization_endpoint)
            .form(&[("client_id", XAI_CLIENT_ID), ("scope", XAI_SCOPE)])
            .send()
            .await?;
        let status = response.status();
        let value = read_json_response(response).await?;
        if !status.is_success() {
            return Err(XaiOAuthError::TokenFetchFailed(format_http_error(
                status, &value,
            )));
        }
        let payload: DeviceCodePayload = serde_json::from_value(value)
            .map_err(|error| XaiOAuthError::Parse(error.to_string()))?;
        let interval = payload
            .interval
            .clamp(1, MAX_POLL_INTERVAL_SECS)
            .saturating_add(1);
        let expires_in = payload.expires_in.clamp(1, MAX_DEVICE_LIFETIME_SECS);
        let now = chrono::Utc::now().timestamp_millis();
        self.pending.write().await.insert(
            payload.device_code.clone(),
            PendingDeviceCode {
                token_endpoint: endpoints.token_endpoint,
                expires_at_ms: now.saturating_add(expires_in as i64 * 1_000),
                interval_secs: interval,
                next_poll_at_ms: now,
            },
        );
        let verification_uri = payload
            .verification_uri_complete
            .or(payload.verification_uri)
            .unwrap_or_else(|| "https://auth.x.ai/activate".to_string());
        Ok(XaiDeviceCodeResponse {
            device_code: payload.device_code,
            user_code: payload.user_code,
            verification_uri,
            expires_in,
            interval,
        })
    }

    pub async fn poll_for_account(
        &self,
        device_code: &str,
    ) -> Result<Option<XaiAccount>, XaiOAuthError> {
        let now = chrono::Utc::now().timestamp_millis();
        let entry = self
            .pending
            .read()
            .await
            .get(device_code)
            .cloned()
            .ok_or_else(|| XaiOAuthError::TokenFetchFailed("Device flow not found".to_string()))?;
        if entry.expires_at_ms <= now {
            self.pending.write().await.remove(device_code);
            return Err(XaiOAuthError::ExpiredToken);
        }
        if entry.next_poll_at_ms > now {
            return Err(XaiOAuthError::AuthorizationPending);
        }
        self.schedule_next_poll(device_code, entry.interval_secs)
            .await;
        let response = self
            .http_client
            .post(&entry.token_endpoint)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", XAI_CLIENT_ID),
                ("device_code", device_code),
            ])
            .send()
            .await?;
        let status = response.status();
        let value = read_json_response(response).await?;
        if let Some(code) = value.get("error").and_then(Value::as_str) {
            return match code {
                "authorization_pending" => Err(XaiOAuthError::AuthorizationPending),
                "slow_down" => {
                    self.increase_poll_interval(device_code).await;
                    Err(XaiOAuthError::AuthorizationPending)
                }
                "access_denied" => {
                    self.pending.write().await.remove(device_code);
                    Err(XaiOAuthError::AccessDenied)
                }
                "expired_token" => {
                    self.pending.write().await.remove(device_code);
                    Err(XaiOAuthError::ExpiredToken)
                }
                _ => Err(XaiOAuthError::TokenFetchFailed(format_http_error(
                    status, &value,
                ))),
            };
        }
        if !status.is_success() {
            return Err(XaiOAuthError::TokenFetchFailed(format_http_error(
                status, &value,
            )));
        }
        let tokens: TokenPayload = serde_json::from_value(value)
            .map_err(|error| XaiOAuthError::Parse(error.to_string()))?;
        let refresh_token = tokens
            .refresh_token
            .clone()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| XaiOAuthError::TokenFetchFailed("Refresh token missing".to_string()))?;
        let (account_id, login) = token_identity(&tokens).ok_or_else(|| {
            XaiOAuthError::Parse("Stable xAI account identity missing".to_string())
        })?;
        let account = self
            .add_account(
                account_id,
                login,
                refresh_token,
                Some(CachedToken {
                    value: tokens.access_token,
                    expires_at_ms: expires_at(tokens.expires_in),
                }),
            )
            .await?;
        self.pending.write().await.remove(device_code);
        Ok(Some(account))
    }

    pub async fn list_accounts(&self) -> Vec<XaiAccount> {
        let accounts = self.accounts.read().await.clone();
        let default = self.resolve_default_account_id().await;
        let mut result = accounts
            .values()
            .map(|account| XaiAccount {
                id: account.id.clone(),
                login: account.login.clone(),
                authenticated_at: account.authenticated_at,
                requires_reauth: account.requires_reauth,
            })
            .collect::<Vec<_>>();
        result.sort_by(|left, right| {
            (default.as_deref() == Some(right.id.as_str()))
                .cmp(&(default.as_deref() == Some(left.id.as_str())))
                .then_with(|| right.authenticated_at.cmp(&left.authenticated_at))
                .then_with(|| left.login.cmp(&right.login))
        });
        result
    }

    pub async fn get_status(&self) -> XaiAuthStatus {
        let accounts = self.list_accounts().await;
        let default = self.resolve_default_account_id().await;
        let username = default.as_ref().and_then(|id| {
            self.accounts
                .try_read()
                .ok()
                .and_then(|accounts| accounts.get(id).map(|account| account.login.clone()))
        });
        XaiAuthStatus {
            authenticated: !accounts.is_empty(),
            accounts,
            default_account_id: default,
            username,
        }
    }

    pub async fn get_valid_token(&self, account_id: Option<&str>) -> Result<String, XaiOAuthError> {
        let id = self.resolve_account_id(account_id).await.ok_or_else(|| {
            XaiOAuthError::AccountNotFound("No xAI account is available".to_string())
        })?;
        if let Some(token) = self.cached_token(&id).await {
            return Ok(token);
        }
        let lock = self.refresh_lock(&id).await;
        let _guard = lock.lock().await;
        if let Some(token) = self.cached_token(&id).await {
            return Ok(token);
        }
        let account = self
            .accounts
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| XaiOAuthError::AccountNotFound(id.clone()))?;
        if account.requires_reauth {
            return Err(XaiOAuthError::ReauthRequired(id));
        }
        let refresh_token = account
            .refresh_token
            .or_else(|| keyring_get(&account.id).ok().flatten())
            .ok_or_else(|| XaiOAuthError::AccountNotFound(account.id.clone()))?;
        let tokens = self.refresh_token(&refresh_token).await.map_err(|error| {
            if matches!(error, XaiOAuthError::RefreshTokenInvalid) {
                XaiOAuthError::ReauthRequired(account.id.clone())
            } else {
                error
            }
        })?;
        let next_refresh = tokens
            .refresh_token
            .clone()
            .filter(|value| !value.trim().is_empty());
        if let Some(next_refresh) = next_refresh {
            if next_refresh != refresh_token {
                let stored = keyring_set(&account.id, &next_refresh).is_ok();
                if let Some(item) = self.accounts.write().await.get_mut(&account.id) {
                    item.refresh_token = (!stored).then_some(next_refresh);
                }
                self.save_to_disk().await?;
            }
        }
        let token = tokens.access_token;
        self.access_tokens.write().await.insert(
            account.id,
            CachedToken {
                value: token.clone(),
                expires_at_ms: expires_at(tokens.expires_in),
            },
        );
        Ok(token)
    }

    pub async fn fetch_models(
        &self,
        account_id: Option<&str>,
    ) -> Result<Vec<XaiModel>, XaiOAuthError> {
        let token = self.get_valid_token(account_id).await?;
        let response = self
            .http_client
            .get("https://api.x.ai/v1/models")
            .bearer_auth(token)
            .send()
            .await?;
        let status = response.status();
        let value = read_json_response(response).await?;
        if !status.is_success() {
            return Err(XaiOAuthError::TokenFetchFailed(format_http_error(
                status, &value,
            )));
        }
        let mut models = value
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                Some(XaiModel {
                    id: entry.get("id")?.as_str()?.to_string(),
                    owned_by: entry
                        .get("owned_by")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
            })
            .collect::<Vec<_>>();
        models.sort_by(|left, right| left.id.cmp(&right.id));
        models.dedup_by(|left, right| left.id == right.id);
        Ok(models)
    }

    pub async fn set_default_account(&self, account_id: &str) -> Result<(), XaiOAuthError> {
        if !self.accounts.read().await.contains_key(account_id) {
            return Err(XaiOAuthError::AccountNotFound(account_id.to_string()));
        }
        *self.default_account_id.write().await = Some(account_id.to_string());
        self.save_to_disk().await
    }

    pub async fn remove_account(&self, account_id: &str) -> Result<(), XaiOAuthError> {
        let _guard = self.mutation_lock.lock().await;
        if self.accounts.write().await.remove(account_id).is_none() {
            return Err(XaiOAuthError::AccountNotFound(account_id.to_string()));
        }
        self.access_tokens.write().await.remove(account_id);
        self.refresh_locks.write().await.remove(account_id);
        keyring_delete(account_id)?;
        let fallback = self.fallback_default_account_id().await;
        if self.default_account_id.read().await.as_deref() == Some(account_id) {
            *self.default_account_id.write().await = fallback;
        }
        self.save_to_disk().await
    }

    pub async fn clear_auth(&self) -> Result<(), XaiOAuthError> {
        let _guard = self.mutation_lock.lock().await;
        let ids = self
            .accounts
            .read()
            .await
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for id in ids {
            keyring_delete(&id)?;
        }
        self.accounts.write().await.clear();
        self.access_tokens.write().await.clear();
        self.refresh_locks.write().await.clear();
        self.pending.write().await.clear();
        *self.default_account_id.write().await = None;
        if self.storage_path.exists() {
            fs::remove_file(&self.storage_path)?;
        }
        Ok(())
    }

    async fn discover_endpoints(&self) -> Result<OAuthEndpoints, XaiOAuthError> {
        if let Some(endpoints) = self.endpoints.read().await.clone() {
            return Ok(endpoints);
        }
        let response = self.http_client.get(XAI_DISCOVERY_URL).send().await?;
        let status = response.status();
        let value = read_json_response(response).await?;
        if !status.is_success() {
            return Err(XaiOAuthError::Network(format_http_error(status, &value)));
        }
        let document: DiscoveryDocument = serde_json::from_value(value)
            .map_err(|error| XaiOAuthError::Parse(error.to_string()))?;
        if document.issuer.trim_end_matches('/') != XAI_ISSUER {
            return Err(XaiOAuthError::Parse(
                "xAI discovery issuer mismatch".to_string(),
            ));
        }
        validate_endpoint(&document.token_endpoint)?;
        validate_endpoint(&document.device_authorization_endpoint)?;
        let endpoints = OAuthEndpoints {
            token_endpoint: document.token_endpoint,
            device_authorization_endpoint: document.device_authorization_endpoint,
        };
        *self.endpoints.write().await = Some(endpoints.clone());
        Ok(endpoints)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPayload, XaiOAuthError> {
        let endpoints = self.discover_endpoints().await?;
        let response = self
            .http_client
            .post(&endpoints.token_endpoint)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", XAI_CLIENT_ID),
                ("refresh_token", refresh_token),
                ("scope", XAI_SCOPE),
            ])
            .send()
            .await?;
        let status = response.status();
        let value = read_json_response(response).await?;
        if status == reqwest::StatusCode::UNAUTHORIZED
            || value.get("error").and_then(Value::as_str) == Some("invalid_grant")
        {
            return Err(XaiOAuthError::RefreshTokenInvalid);
        }
        if !status.is_success() {
            return Err(XaiOAuthError::TokenFetchFailed(format_http_error(
                status, &value,
            )));
        }
        serde_json::from_value(value).map_err(|error| XaiOAuthError::Parse(error.to_string()))
    }

    async fn add_account(
        &self,
        id: String,
        login: String,
        refresh_token: String,
        cached_token: Option<CachedToken>,
    ) -> Result<XaiAccount, XaiOAuthError> {
        let _guard = self.mutation_lock.lock().await;
        let stored_in_keyring = keyring_set(&id, &refresh_token).is_ok();
        let data = AccountData {
            id: id.clone(),
            login: login.clone(),
            refresh_token: (!stored_in_keyring).then_some(refresh_token),
            authenticated_at: chrono::Utc::now().timestamp(),
            requires_reauth: false,
        };
        let account = XaiAccount {
            id: id.clone(),
            login,
            authenticated_at: data.authenticated_at,
            requires_reauth: false,
        };
        self.accounts.write().await.insert(id.clone(), data);
        if self.default_account_id.read().await.is_none() {
            *self.default_account_id.write().await = Some(id.clone());
        }
        self.save_to_disk().await?;
        if let Some(cached_token) = cached_token {
            self.access_tokens.write().await.insert(id, cached_token);
        }
        Ok(account)
    }

    async fn cached_token(&self, id: &str) -> Option<String> {
        self.access_tokens
            .read()
            .await
            .get(id)
            .filter(|token| token.usable())
            .map(|token| token.value.clone())
    }

    async fn refresh_lock(&self, id: &str) -> Arc<Mutex<()>> {
        if let Some(lock) = self.refresh_locks.read().await.get(id).cloned() {
            return lock;
        }
        self.refresh_locks
            .write()
            .await
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn resolve_account_id(&self, requested: Option<&str>) -> Option<String> {
        if let Some(id) = requested.map(str::trim).filter(|value| !value.is_empty()) {
            return self
                .accounts
                .read()
                .await
                .contains_key(id)
                .then(|| id.to_string());
        }
        self.resolve_default_account_id().await
    }

    async fn resolve_default_account_id(&self) -> Option<String> {
        let stored = self.default_account_id.read().await.clone();
        let accounts = self.accounts.read().await;
        stored.filter(|id| accounts.contains_key(id)).or_else(|| {
            accounts
                .values()
                .max_by_key(|account| account.authenticated_at)
                .map(|account| account.id.clone())
        })
    }

    async fn fallback_default_account_id(&self) -> Option<String> {
        self.accounts
            .read()
            .await
            .values()
            .max_by_key(|account| account.authenticated_at)
            .map(|account| account.id.clone())
    }

    async fn schedule_next_poll(&self, device_code: &str, interval: u64) {
        if let Some(entry) = self.pending.write().await.get_mut(device_code) {
            entry.next_poll_at_ms = chrono::Utc::now()
                .timestamp_millis()
                .saturating_add(interval as i64 * 1_000);
        }
    }

    async fn increase_poll_interval(&self, device_code: &str) {
        if let Some(entry) = self.pending.write().await.get_mut(device_code) {
            entry.interval_secs = (entry.interval_secs + 2).min(MAX_POLL_INTERVAL_SECS);
        }
    }
}

fn default_poll_interval() -> u64 {
    DEFAULT_POLL_INTERVAL_SECS
}

fn expires_at(expires_in: Option<i64>) -> i64 {
    chrono::Utc::now().timestamp_millis()
        + expires_in.unwrap_or(DEFAULT_TOKEN_LIFETIME_SECS).max(60) * 1_000
}

async fn read_json_response(response: reqwest::Response) -> Result<Value, XaiOAuthError> {
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(XaiOAuthError::Parse(
            "xAI response was too large".to_string(),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| XaiOAuthError::Parse(error.to_string()))
}

fn format_http_error(status: reqwest::StatusCode, value: &Value) -> String {
    let code = value
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!("HTTP {status} ({code})")
}

fn validate_endpoint(endpoint: &str) -> Result<(), XaiOAuthError> {
    let parsed =
        url::Url::parse(endpoint).map_err(|error| XaiOAuthError::Parse(error.to_string()))?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("auth.x.ai") {
        return Err(XaiOAuthError::Parse(
            "xAI OAuth endpoint is not trusted".to_string(),
        ));
    }
    Ok(())
}

fn parse_claims(token: &str) -> Option<TokenClaims> {
    let segment = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(segment).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn token_identity(tokens: &TokenPayload) -> Option<(String, String)> {
    let claims = tokens
        .id_token
        .as_deref()
        .and_then(parse_claims)
        .or_else(|| parse_claims(&tokens.access_token))?;
    let id = claims.sub?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let login = claims
        .email
        .or(claims.preferred_username)
        .or(claims.name)
        .unwrap_or_else(|| format!("xAI ({})", id.chars().take(12).collect::<String>()));
    Some((id, login))
}

fn keyring_entry(account_id: &str) -> Result<keyring::Entry, XaiOAuthError> {
    keyring::Entry::new(KEYRING_SERVICE, account_id)
        .map_err(|error| XaiOAuthError::Io(format!("Failed to open OAuth keyring: {error}")))
}

fn keyring_get(account_id: &str) -> Result<Option<String>, XaiOAuthError> {
    match keyring_entry(account_id)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(XaiOAuthError::Io(format!(
            "Failed to read OAuth keyring: {error}"
        ))),
    }
}

fn keyring_set(account_id: &str, token: &str) -> Result<(), XaiOAuthError> {
    keyring_entry(account_id)?
        .set_password(token)
        .map_err(|error| XaiOAuthError::Io(format!("Failed to save OAuth keyring: {error}")))
}

fn keyring_delete(account_id: &str) -> Result<(), XaiOAuthError> {
    match keyring_entry(account_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(XaiOAuthError::Io(format!(
            "Failed to delete OAuth keyring: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_claims, token_identity, TokenPayload};
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

    #[test]
    fn rejects_non_jwt_identity() {
        let payload = TokenPayload {
            access_token: "not-a-jwt".to_string(),
            refresh_token: Some("refresh".to_string()),
            id_token: None,
            expires_in: Some(3600),
        };
        assert!(token_identity(&payload).is_none());
    }

    #[test]
    fn reads_sub_and_email_from_jwt_payload() {
        let payload = serde_json::json!({"sub":"acct-1","email":"user@example.com"});
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        let token = format!("header.{encoded}.signature");
        assert_eq!(parse_claims(&token).unwrap().sub.as_deref(), Some("acct-1"));
        let tokens = TokenPayload {
            access_token: token,
            refresh_token: None,
            id_token: None,
            expires_in: None,
        };
        assert_eq!(token_identity(&tokens).unwrap().1, "user@example.com");
    }
}

pub(crate) fn init_xai_oauth_state(app_handle: &tauri::AppHandle) {
    let storage_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cchub")
        .join("xai_oauth_auth.json");
    let proxy_url = app_handle
        .state::<crate::db::DbState>()
        .0
        .lock()
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT value FROM app_settings WHERE key = 'proxy_url'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
        })
        .filter(|value| !value.trim().is_empty());
    app_handle.manage(XaiOAuthState(Arc::new(XaiOAuthManager::new(
        storage_path,
        proxy_url,
    ))));
}
