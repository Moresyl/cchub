use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::{Mutex, RwLock};

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const DEVICE_START_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
const DEVICE_POLL_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const DEVICE_VERIFY_URL: &str = "https://auth.openai.com/codex/device";
const REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";
const TOKEN_REFRESH_BUFFER_MS: i64 = 60_000;
const DEFAULT_DEVICE_EXPIRY_SECS: u64 = 900;
const KEYRING_SERVICE: &str = "CCHub Codex OAuth";

#[derive(Debug, thiserror::Error)]
pub enum CodexOAuthError {
    #[error("OAuth authorization is pending")]
    AuthorizationPending,
    #[error("OAuth device code expired")]
    ExpiredToken,
    #[error("OAuth token exchange failed: {0}")]
    TokenFetchFailed(String),
    #[error("OAuth refresh token is invalid or expired")]
    RefreshTokenInvalid,
    #[error("Network error: {0}")]
    Network(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("OAuth account not found: {0}")]
    AccountNotFound(String),
}

impl From<reqwest::Error> for CodexOAuthError {
    fn from(error: reqwest::Error) -> Self {
        Self::Network(error.to_string())
    }
}

impl From<std::io::Error> for CodexOAuthError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexDeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccount {
    pub id: String,
    pub login: String,
    pub authenticated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthStatus {
    pub accounts: Vec<CodexAccount>,
    pub default_account_id: Option<String>,
    pub authenticated: bool,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeviceCodePayload {
    device_auth_id: String,
    user_code: String,
    #[serde(default)]
    interval: Option<Value>,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DevicePollPayload {
    authorization_code: String,
    code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct TokenPayload {
    access_token: String,
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TokenClaims {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    organizations: Vec<OrganizationClaim>,
    #[serde(default, rename = "https://api.openai.com/auth")]
    openai_auth: Option<OpenAiAuthClaim>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct OrganizationClaim {
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct OpenAiAuthClaim {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountData {
    id: String,
    email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    authenticated_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    accounts: HashMap<String, AccountData>,
    #[serde(default)]
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
    user_code: String,
    expires_at_ms: i64,
}

pub struct CodexOAuthManager {
    accounts: RwLock<HashMap<String, AccountData>>,
    default_account_id: RwLock<Option<String>>,
    tokens: RwLock<HashMap<String, CachedToken>>,
    refresh_locks: RwLock<HashMap<String, Arc<Mutex<()>>>>,
    pending: RwLock<HashMap<String, PendingDeviceCode>>,
    http_client: reqwest::Client,
    storage_path: PathBuf,
}

pub struct CodexOAuthState(pub Arc<CodexOAuthManager>);

impl CodexOAuthManager {
    pub fn new(storage_path: PathBuf, proxy_url: Option<String>) -> Self {
        let manager = Self {
            accounts: RwLock::new(HashMap::new()),
            default_account_id: RwLock::new(None),
            tokens: RwLock::new(HashMap::new()),
            refresh_locks: RwLock::new(HashMap::new()),
            pending: RwLock::new(HashMap::new()),
            http_client: crate::shared::http_client::build_http_client(
                proxy_url.as_deref(),
                Some("CCHub OAuth"),
                std::time::Duration::from_secs(30),
            )
            .unwrap_or_else(|_| crate::shared::http_client::default_http_client()),
            storage_path,
        };
        if let Err(error) = manager.load_from_disk_sync() {
            crate::utils::append_runtime_log(
                "warn",
                "oauth",
                &format!("Failed to load OAuth accounts: {error}"),
            );
        }
        manager
    }

    pub async fn start_device_flow(&self) -> Result<CodexDeviceCodeResponse, CodexOAuthError> {
        let response = self
            .http_client
            .post(DEVICE_START_URL)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "client_id": CLIENT_ID }))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(CodexOAuthError::TokenFetchFailed(format!(
                "Device flow returned HTTP {}",
                response.status()
            )));
        }
        let payload: DeviceCodePayload = response
            .json()
            .await
            .map_err(|error| CodexOAuthError::Parse(error.to_string()))?;
        let interval = parse_interval(payload.interval.as_ref());
        let expires_in = payload.expires_in.unwrap_or(DEFAULT_DEVICE_EXPIRY_SECS);
        let expires_at_ms = chrono::Utc::now().timestamp_millis() + expires_in as i64 * 1000;
        let mut pending = self.pending.write().await;
        let now = chrono::Utc::now().timestamp_millis();
        pending.retain(|_, item| item.expires_at_ms > now);
        pending.insert(
            payload.device_auth_id.clone(),
            PendingDeviceCode {
                user_code: payload.user_code.clone(),
                expires_at_ms,
            },
        );
        Ok(CodexDeviceCodeResponse {
            device_code: payload.device_auth_id,
            user_code: payload.user_code,
            verification_uri: DEVICE_VERIFY_URL.to_string(),
            expires_in,
            interval,
        })
    }

    pub async fn poll_for_account(
        &self,
        device_code: &str,
    ) -> Result<Option<CodexAccount>, CodexOAuthError> {
        let pending = self
            .pending
            .read()
            .await
            .get(device_code)
            .cloned()
            .ok_or_else(|| {
                CodexOAuthError::TokenFetchFailed("Device flow not found".to_string())
            })?;
        if pending.expires_at_ms <= chrono::Utc::now().timestamp_millis() {
            self.pending.write().await.remove(device_code);
            return Err(CodexOAuthError::ExpiredToken);
        }
        let response = self
            .http_client
            .post(DEVICE_POLL_URL)
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "device_auth_id": device_code,
                "user_code": pending.user_code,
            }))
            .send()
            .await?;
        let status = response.status();
        if matches!(
            status,
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND
        ) {
            return Err(CodexOAuthError::AuthorizationPending);
        }
        if status == reqwest::StatusCode::GONE {
            self.pending.write().await.remove(device_code);
            return Err(CodexOAuthError::ExpiredToken);
        }
        if !status.is_success() {
            return Err(CodexOAuthError::TokenFetchFailed(format!(
                "Device flow returned HTTP {status}"
            )));
        }
        let payload: DevicePollPayload = response
            .json()
            .await
            .map_err(|error| CodexOAuthError::Parse(error.to_string()))?;
        let tokens = self
            .exchange_code(&payload.authorization_code, &payload.code_verifier)
            .await?;
        self.pending.write().await.remove(device_code);
        let refresh_token = tokens.refresh_token.clone().ok_or_else(|| {
            CodexOAuthError::TokenFetchFailed("Refresh token missing".to_string())
        })?;
        let (account_id, email) = token_identity(&tokens);
        let account_id = account_id
            .ok_or_else(|| CodexOAuthError::Parse("Account identity missing".to_string()))?;
        self.tokens.write().await.insert(
            account_id.clone(),
            CachedToken {
                value: tokens.access_token,
                expires_at_ms: expires_at(tokens.expires_in),
            },
        );
        Ok(Some(
            self.add_account(account_id, refresh_token, email).await?,
        ))
    }

    pub async fn get_valid_token(
        &self,
        account_id: Option<&str>,
    ) -> Result<String, CodexOAuthError> {
        let id = self.resolve_account_id(account_id).await.ok_or_else(|| {
            CodexOAuthError::AccountNotFound("No OAuth account is available".to_string())
        })?;
        {
            let tokens = self.tokens.read().await;
            if let Some(token) = tokens.get(&id).filter(|token| token.usable()) {
                return Ok(token.value.clone());
            }
        }
        let lock = self.refresh_lock(&id).await;
        let _guard = lock.lock().await;
        {
            let tokens = self.tokens.read().await;
            if let Some(token) = tokens.get(&id).filter(|token| token.usable()) {
                return Ok(token.value.clone());
            }
        }
        let refresh_token = self
            .accounts
            .read()
            .await
            .get(&id)
            .and_then(|account| account.refresh_token.clone())
            .or_else(|| keyring_get(&id).ok().flatten())
            .ok_or_else(|| CodexOAuthError::AccountNotFound(id.clone()))?;
        let refreshed = self.refresh_token(&refresh_token).await?;
        if let Some(next_refresh) = refreshed
            .refresh_token
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            if next_refresh != refresh_token {
                let stored_in_keyring = keyring_set(&id, next_refresh).is_ok();
                if let Some(account) = self.accounts.write().await.get_mut(&id) {
                    account.refresh_token = (!stored_in_keyring).then(|| next_refresh.to_string());
                }
                self.save_to_disk().await?;
            }
        }
        let token = refreshed.access_token;
        self.tokens.write().await.insert(
            id,
            CachedToken {
                value: token.clone(),
                expires_at_ms: expires_at(refreshed.expires_in),
            },
        );
        Ok(token)
    }

    pub async fn list_accounts(&self) -> Vec<CodexAccount> {
        let accounts = self.accounts.read().await.clone();
        let default = self.resolve_default_account_id().await;
        let mut result = accounts
            .values()
            .map(|account| CodexAccount {
                id: account.id.clone(),
                login: account
                    .email
                    .clone()
                    .unwrap_or_else(|| format!("ChatGPT ({})", account.id)),
                authenticated_at: account.authenticated_at,
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

    pub async fn get_status(&self) -> CodexAuthStatus {
        let accounts = self.list_accounts().await;
        let default = self.resolve_default_account_id().await;
        let username = if let Some(id) = default.as_ref() {
            self.accounts
                .read()
                .await
                .get(id)
                .and_then(|account| account.email.clone())
        } else {
            None
        };
        let username = username.or_else(|| accounts.first().map(|account| account.login.clone()));
        CodexAuthStatus {
            authenticated: !accounts.is_empty(),
            accounts,
            default_account_id: default,
            username,
        }
    }

    pub async fn default_account_id(&self) -> Option<String> {
        self.resolve_default_account_id().await
    }

    pub async fn remove_account(&self, account_id: &str) -> Result<(), CodexOAuthError> {
        if self.accounts.write().await.remove(account_id).is_none() {
            return Err(CodexOAuthError::AccountNotFound(account_id.to_string()));
        }
        self.tokens.write().await.remove(account_id);
        self.refresh_locks.write().await.remove(account_id);
        keyring_delete(account_id)?;
        if self.default_account_id.read().await.as_deref() == Some(account_id) {
            let fallback = self.fallback_default_account_id().await;
            *self.default_account_id.write().await = fallback;
        }
        self.save_to_disk().await
    }

    pub async fn set_default_account(&self, account_id: &str) -> Result<(), CodexOAuthError> {
        if !self.accounts.read().await.contains_key(account_id) {
            return Err(CodexOAuthError::AccountNotFound(account_id.to_string()));
        }
        *self.default_account_id.write().await = Some(account_id.to_string());
        self.save_to_disk().await
    }

    pub async fn clear_auth(&self) -> Result<(), CodexOAuthError> {
        let account_ids = self
            .accounts
            .read()
            .await
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        self.accounts.write().await.clear();
        self.tokens.write().await.clear();
        self.refresh_locks.write().await.clear();
        self.pending.write().await.clear();
        *self.default_account_id.write().await = None;
        for account_id in account_ids {
            keyring_delete(&account_id)?;
        }
        if self.storage_path.exists() {
            fs::remove_file(&self.storage_path)?;
        }
        Ok(())
    }

    async fn exchange_code(
        &self,
        code: &str,
        verifier: &str,
    ) -> Result<TokenPayload, CodexOAuthError> {
        let response = self
            .http_client
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", REDIRECT_URI),
                ("client_id", CLIENT_ID),
                ("code_verifier", verifier),
            ])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(CodexOAuthError::TokenFetchFailed(format!(
                "Token exchange returned HTTP {}",
                response.status()
            )));
        }
        response
            .json()
            .await
            .map_err(|error| CodexOAuthError::Parse(error.to_string()))
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPayload, CodexOAuthError> {
        let response = self
            .http_client
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", CLIENT_ID),
                ("scope", "openid profile email"),
            ])
            .send()
            .await?;
        if matches!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            return Err(CodexOAuthError::RefreshTokenInvalid);
        }
        if !response.status().is_success() {
            return Err(CodexOAuthError::TokenFetchFailed(format!(
                "Token refresh returned HTTP {}",
                response.status()
            )));
        }
        response
            .json()
            .await
            .map_err(|error| CodexOAuthError::Parse(error.to_string()))
    }

    async fn add_account(
        &self,
        id: String,
        refresh_token: String,
        email: Option<String>,
    ) -> Result<CodexAccount, CodexOAuthError> {
        let stored_in_keyring = keyring_set(&id, &refresh_token).is_ok();
        let data = AccountData {
            id: id.clone(),
            email: email.clone(),
            refresh_token: (!stored_in_keyring).then_some(refresh_token),
            authenticated_at: chrono::Utc::now().timestamp(),
        };
        self.accounts.write().await.insert(id.clone(), data);
        if self.default_account_id.read().await.is_none() {
            *self.default_account_id.write().await = Some(id.clone());
        }
        self.save_to_disk().await?;
        Ok(CodexAccount {
            id: id.clone(),
            login: email.unwrap_or_else(|| format!("ChatGPT ({id})")),
            authenticated_at: chrono::Utc::now().timestamp(),
        })
    }

    async fn resolve_account_id(&self, requested: Option<&str>) -> Option<String> {
        if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
            return self
                .accounts
                .read()
                .await
                .contains_key(requested)
                .then(|| requested.to_string());
        }
        self.resolve_default_account_id().await
    }

    async fn resolve_default_account_id(&self) -> Option<String> {
        let stored = self.default_account_id.read().await.clone();
        let accounts = self.accounts.read().await;
        stored
            .filter(|id| accounts.contains_key(id))
            .or_else(|| self.fallback_from_map(&accounts))
    }

    async fn fallback_default_account_id(&self) -> Option<String> {
        let accounts = self.accounts.read().await;
        self.fallback_from_map(&accounts)
    }

    fn fallback_from_map(&self, accounts: &HashMap<String, AccountData>) -> Option<String> {
        accounts
            .values()
            .max_by_key(|account| account.authenticated_at)
            .map(|account| account.id.clone())
    }

    async fn refresh_lock(&self, account_id: &str) -> Arc<Mutex<()>> {
        if let Some(lock) = self.refresh_locks.read().await.get(account_id).cloned() {
            return lock;
        }
        self.refresh_locks
            .write()
            .await
            .entry(account_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn load_from_disk_sync(&self) -> Result<(), CodexOAuthError> {
        if !self.storage_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&self.storage_path)?;
        let store: Store = serde_json::from_str(&content)
            .map_err(|error| CodexOAuthError::Parse(error.to_string()))?;
        let mut sanitized_store = store;
        let mut migrated = false;
        for (id, account) in sanitized_store.accounts.iter_mut() {
            if let Some(refresh_token) = account.refresh_token.take() {
                if keyring_set(id, &refresh_token).is_err() {
                    account.refresh_token = Some(refresh_token);
                } else {
                    migrated = true;
                }
            }
        }
        if migrated {
            let content = serde_json::to_string_pretty(&sanitized_store)
                .map_err(|error| CodexOAuthError::Parse(error.to_string()))?;
            fs::write(&self.storage_path, content)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&self.storage_path, fs::Permissions::from_mode(0o600))?;
            }
        }
        let default_account_id = sanitized_store.default_account_id.clone();
        if let Ok(mut accounts) = self.accounts.try_write() {
            *accounts = sanitized_store.accounts;
        }
        if let Ok(mut default) = self.default_account_id.try_write() {
            *default = default_account_id;
        }
        Ok(())
    }

    async fn save_to_disk(&self) -> Result<(), CodexOAuthError> {
        let store = Store {
            accounts: self.accounts.read().await.clone(),
            default_account_id: self.resolve_default_account_id().await,
        };
        let content = serde_json::to_string_pretty(&store)
            .map_err(|error| CodexOAuthError::Parse(error.to_string()))?;
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
            }
        }
        let parent = self
            .storage_path
            .parent()
            .ok_or_else(|| CodexOAuthError::Io("Invalid OAuth storage path".to_string()))?;
        let file_name = self
            .storage_path
            .file_name()
            .ok_or_else(|| CodexOAuthError::Io("Invalid OAuth storage filename".to_string()))?
            .to_string_lossy();
        let temp = parent.join(format!("{file_name}.tmp.{}", uuid::Uuid::new_v4()));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
        }
        if self.storage_path.exists() {
            let _ = fs::remove_file(&self.storage_path);
        }
        fs::rename(temp, &self.storage_path)?;
        Ok(())
    }
}

fn parse_interval(value: Option<&Value>) -> u64 {
    let seconds = match value {
        Some(Value::Number(number)) => number.as_u64().unwrap_or(5),
        Some(Value::String(text)) => text.parse::<u64>().unwrap_or(5),
        _ => 5,
    };
    seconds.max(1) + 2
}

fn expires_at(expires_in: Option<i64>) -> i64 {
    chrono::Utc::now().timestamp_millis() + expires_in.unwrap_or(3600).max(60) * 1000
}

fn parse_claims(token: &str) -> Option<TokenClaims> {
    let part = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(part).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn token_identity(tokens: &TokenPayload) -> (Option<String>, Option<String>) {
    let claims = tokens
        .id_token
        .as_deref()
        .and_then(parse_claims)
        .or_else(|| parse_claims(&tokens.access_token));
    let Some(claims) = claims else {
        return (None, None);
    };
    let account_id = claims
        .chatgpt_account_id
        .or_else(|| claims.openai_auth.and_then(|auth| auth.chatgpt_account_id))
        .or_else(|| claims.organizations.into_iter().find_map(|org| org.id));
    (account_id, claims.email)
}

fn keyring_entry(account_id: &str) -> Result<keyring::Entry, CodexOAuthError> {
    keyring::Entry::new(KEYRING_SERVICE, account_id)
        .map_err(|error| CodexOAuthError::Io(format!("Failed to open OAuth keyring: {error}")))
}

fn keyring_get(account_id: &str) -> Result<Option<String>, CodexOAuthError> {
    match keyring_entry(account_id)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(CodexOAuthError::Io(format!(
            "Failed to read OAuth keyring: {error}"
        ))),
    }
}

fn keyring_set(account_id: &str, refresh_token: &str) -> Result<(), CodexOAuthError> {
    keyring_entry(account_id)?
        .set_password(refresh_token)
        .map_err(|error| CodexOAuthError::Io(format!("Failed to save OAuth keyring: {error}")))
}

fn keyring_delete(account_id: &str) -> Result<(), CodexOAuthError> {
    match keyring_entry(account_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CodexOAuthError::Io(format!(
            "Failed to delete OAuth keyring: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_interval, token_identity, TokenPayload};
    use serde_json::json;

    #[test]
    fn parses_string_and_number_intervals() {
        assert_eq!(parse_interval(Some(&json!("4"))), 6);
        assert_eq!(parse_interval(Some(&json!(8))), 10);
    }

    #[test]
    fn refuses_to_infer_identity_without_jwt_claims() {
        let payload = TokenPayload {
            access_token: "not-a-jwt".to_string(),
            refresh_token: Some("refresh".to_string()),
            id_token: None,
            expires_in: Some(3600),
        };
        assert_eq!(token_identity(&payload), (None, None));
    }
}

pub(crate) fn init_codex_oauth_state(app_handle: &tauri::AppHandle) {
    let storage_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cchub")
        .join("codex_oauth_auth.json");
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
    app_handle.manage(CodexOAuthState(Arc::new(CodexOAuthManager::new(
        storage_path,
        proxy_url,
    ))));
}
