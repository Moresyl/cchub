use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, RwLock};

const GITHUB_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_OAUTH_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_URL: &str = "https://api.github.com/user";
const COPILOT_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";
const COPILOT_USAGE_URL: &str = "https://api.github.com/copilot_internal/user";
const COPILOT_MODELS_URL: &str = "https://api.githubcopilot.com/models";
const TOKEN_REFRESH_BUFFER_SECONDS: i64 = 60;

pub const COPILOT_EDITOR_VERSION: &str = "vscode/1.96.0";
pub const COPILOT_PLUGIN_VERSION: &str = "copilot-chat/0.26.7";
pub const COPILOT_USER_AGENT: &str = "GitHubCopilotChat/0.26.7";
pub const COPILOT_API_VERSION: &str = "2025-04-01";
pub const COPILOT_INTEGRATION_ID: &str = "vscode-chat";

#[derive(Debug, thiserror::Error)]
pub enum CopilotAuthError {
    #[error("GitHub authorization was denied")]
    AccessDenied,
    #[error("GitHub device code expired")]
    ExpiredToken,
    #[error("GitHub token is invalid or expired")]
    GitHubTokenInvalid,
    #[error("GitHub Copilot subscription is unavailable for this account")]
    NoCopilotSubscription,
    #[error("GitHub account not found: {0}")]
    AccountNotFound(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Copilot token fetch failed: {0}")]
    Token(String),
}

impl From<reqwest::Error> for CopilotAuthError {
    fn from(value: reqwest::Error) -> Self {
        Self::Network(value.to_string())
    }
}

impl From<std::io::Error> for CopilotAuthError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubDeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
struct GitHubOAuthResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubUser {
    login: String,
    id: u64,
    avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAccount {
    pub id: String,
    pub login: String,
    pub avatar_url: Option<String>,
    pub authenticated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotAuthStatus {
    pub accounts: Vec<GitHubAccount>,
    pub default_account_id: Option<String>,
    pub authenticated: bool,
    pub username: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotQuotaDetail {
    pub entitlement: i64,
    pub remaining: i64,
    pub percent_remaining: f64,
    pub unlimited: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotQuotaSnapshots {
    pub chat: CopilotQuotaDetail,
    pub completions: CopilotQuotaDetail,
    pub premium_interactions: CopilotQuotaDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotUsage {
    pub copilot_plan: String,
    pub quota_reset_date: String,
    pub quota_snapshots: CopilotQuotaSnapshots,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotModel {
    pub id: String,
    pub name: String,
    pub vendor: String,
}

#[derive(Debug, Deserialize)]
struct CopilotModelsResponse {
    data: Vec<CopilotModelEntry>,
}

#[derive(Debug, Deserialize)]
struct CopilotModelEntry {
    id: String,
    name: String,
    vendor: String,
    #[serde(default)]
    model_picker_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CopilotToken {
    token: String,
    expires_at: i64,
}

impl CopilotToken {
    fn is_expiring_soon(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.expires_at - now < TOKEN_REFRESH_BUFFER_SECONDS
    }
}

#[derive(Debug, Deserialize)]
struct CopilotTokenResponse {
    token: String,
    expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubAccountData {
    github_token: String,
    user: GitHubUser,
    authenticated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CopilotAuthStore {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    accounts: HashMap<String, GitHubAccountData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_account_id: Option<String>,
}

pub struct CopilotAuthManager {
    accounts: RwLock<HashMap<String, GitHubAccountData>>,
    default_account_id: RwLock<Option<String>>,
    copilot_tokens: RwLock<HashMap<String, CopilotToken>>,
    refresh_locks: RwLock<HashMap<String, Arc<Mutex<()>>>>,
    http_client: reqwest::Client,
    storage_path: PathBuf,
}

pub struct CopilotAuthState(pub Arc<CopilotAuthManager>);

impl From<&GitHubAccountData> for GitHubAccount {
    fn from(value: &GitHubAccountData) -> Self {
        Self {
            id: value.user.id.to_string(),
            login: value.user.login.clone(),
            avatar_url: value.user.avatar_url.clone(),
            authenticated_at: value.authenticated_at,
        }
    }
}

impl CopilotAuthManager {
    pub fn new(storage_path: PathBuf, proxy_url: Option<String>) -> Self {
        let manager = Self {
            accounts: RwLock::new(HashMap::new()),
            default_account_id: RwLock::new(None),
            copilot_tokens: RwLock::new(HashMap::new()),
            refresh_locks: RwLock::new(HashMap::new()),
            http_client: crate::shared::http_client::build_http_client(
                proxy_url.as_deref(),
                Some(COPILOT_USER_AGENT),
                std::time::Duration::from_secs(30),
            )
            .unwrap_or_else(|error| {
                crate::utils::append_runtime_log(
                    "warn",
                    "copilot_auth",
                    &format!("Failed to build configured HTTP client: {error}"),
                );
                crate::shared::http_client::default_http_client()
            }),
            storage_path,
        };
        if let Err(error) = manager.load_from_disk_sync() {
            crate::utils::append_runtime_log(
                "warn",
                "copilot_auth",
                &format!("Failed to load Copilot auth store: {error}"),
            );
        }
        manager
    }

    pub async fn list_accounts(&self) -> Vec<GitHubAccount> {
        let accounts = self.accounts.read().await.clone();
        let default_account_id = self.resolve_default_account_id().await;
        Self::sorted_accounts(&accounts, default_account_id.as_deref())
    }

    pub async fn start_device_flow(&self) -> Result<GitHubDeviceCodeResponse, CopilotAuthError> {
        let response = self
            .http_client
            .post(GITHUB_DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .header("User-Agent", COPILOT_USER_AGENT)
            .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", "read:user")])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(CopilotAuthError::Network(format!(
                "GitHub device flow request failed: {status} - {text}"
            )));
        }

        response
            .json()
            .await
            .map_err(|error| CopilotAuthError::Parse(error.to_string()))
    }

    pub async fn poll_for_token(
        &self,
        device_code: &str,
    ) -> Result<Option<GitHubAccount>, CopilotAuthError> {
        let response = self
            .http_client
            .post(GITHUB_OAUTH_TOKEN_URL)
            .header("Accept", "application/json")
            .header("User-Agent", COPILOT_USER_AGENT)
            .form(&[
                ("client_id", GITHUB_CLIENT_ID),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;

        let oauth_response: GitHubOAuthResponse = response
            .json()
            .await
            .map_err(|error| CopilotAuthError::Parse(error.to_string()))?;

        if let Some(error) = oauth_response.error {
            return match error.as_str() {
                "authorization_pending" | "slow_down" => Ok(None),
                "access_denied" => Err(CopilotAuthError::AccessDenied),
                "expired_token" => Err(CopilotAuthError::ExpiredToken),
                _ => Err(CopilotAuthError::Network(format!(
                    "{error}: {}",
                    oauth_response.error_description.unwrap_or_default()
                ))),
            };
        }

        let access_token = oauth_response
            .access_token
            .ok_or_else(|| CopilotAuthError::Parse("Missing GitHub access token".to_string()))?;
        let user = self.fetch_user_info_with_token(&access_token).await?;
        let account_id = user.id.to_string();
        self.fetch_copilot_token_with_github_token(&access_token, &account_id)
            .await?;
        self.add_account_internal(access_token, user)
            .await
            .map(Some)
    }

    pub async fn remove_account(&self, account_id: &str) -> Result<(), CopilotAuthError> {
        {
            let mut accounts = self.accounts.write().await;
            if accounts.remove(account_id).is_none() {
                return Err(CopilotAuthError::AccountNotFound(account_id.to_string()));
            }
        }
        {
            let mut tokens = self.copilot_tokens.write().await;
            tokens.remove(account_id);
        }
        {
            let mut locks = self.refresh_locks.write().await;
            locks.remove(account_id);
        }
        {
            let accounts = self.accounts.read().await;
            let mut default_account_id = self.default_account_id.write().await;
            if default_account_id.as_deref() == Some(account_id) {
                *default_account_id = Self::fallback_default_account_id(&accounts);
            }
        }
        self.save_to_disk().await
    }

    pub async fn clear_auth(&self) -> Result<(), CopilotAuthError> {
        self.accounts.write().await.clear();
        self.copilot_tokens.write().await.clear();
        self.refresh_locks.write().await.clear();
        *self.default_account_id.write().await = None;
        if self.storage_path.exists() {
            fs::remove_file(&self.storage_path)?;
        }
        Ok(())
    }

    pub async fn set_default_account(&self, account_id: &str) -> Result<(), CopilotAuthError> {
        {
            let accounts = self.accounts.read().await;
            if !accounts.contains_key(account_id) {
                return Err(CopilotAuthError::AccountNotFound(account_id.to_string()));
            }
        }
        let mut default_account_id = self.default_account_id.write().await;
        *default_account_id = Some(account_id.to_string());
        drop(default_account_id);
        self.save_to_disk().await
    }

    pub async fn get_status(&self) -> CopilotAuthStatus {
        let accounts = self.accounts.read().await.clone();
        let default_account_id = self.resolve_default_account_id().await;
        let tokens = self.copilot_tokens.read().await.clone();
        let account_list = Self::sorted_accounts(&accounts, default_account_id.as_deref());
        let username = default_account_id
            .as_ref()
            .and_then(|id| accounts.get(id))
            .map(|account| account.user.login.clone())
            .or_else(|| account_list.first().map(|account| account.login.clone()));
        let expires_at = default_account_id
            .as_ref()
            .and_then(|id| tokens.get(id))
            .map(|token| token.expires_at);

        CopilotAuthStatus {
            authenticated: !account_list.is_empty(),
            accounts: account_list,
            default_account_id,
            username,
            expires_at,
        }
    }

    pub async fn fetch_usage(
        &self,
        account_id: Option<&str>,
    ) -> Result<CopilotUsage, CopilotAuthError> {
        let resolved = self
            .resolve_account_id(account_id)
            .await
            .ok_or(CopilotAuthError::GitHubTokenInvalid)?;
        let github_token = {
            let accounts = self.accounts.read().await;
            accounts
                .get(&resolved)
                .map(|account| account.github_token.clone())
                .ok_or_else(|| CopilotAuthError::AccountNotFound(resolved.clone()))?
        };
        let response = self
            .http_client
            .get(COPILOT_USAGE_URL)
            .header("Authorization", format!("token {github_token}"))
            .header("Content-Type", "application/json")
            .header("editor-version", COPILOT_EDITOR_VERSION)
            .header("editor-plugin-version", COPILOT_PLUGIN_VERSION)
            .header("user-agent", COPILOT_USER_AGENT)
            .header("x-github-api-version", COPILOT_API_VERSION)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CopilotAuthError::GitHubTokenInvalid);
        }
        let response = response
            .error_for_status()
            .map_err(|error| CopilotAuthError::Token(error.to_string()))?;
        response
            .json::<CopilotUsage>()
            .await
            .map_err(|error| CopilotAuthError::Parse(error.to_string()))
    }

    pub async fn fetch_models(
        &self,
        account_id: Option<&str>,
    ) -> Result<Vec<CopilotModel>, CopilotAuthError> {
        let token = self.get_valid_token_for_account(account_id).await?;
        let response = self
            .http_client
            .get(COPILOT_MODELS_URL)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("copilot-integration-id", COPILOT_INTEGRATION_ID)
            .header("editor-version", COPILOT_EDITOR_VERSION)
            .header("editor-plugin-version", COPILOT_PLUGIN_VERSION)
            .header("user-agent", COPILOT_USER_AGENT)
            .header("x-github-api-version", COPILOT_API_VERSION)
            .send()
            .await?;
        let response = response
            .error_for_status()
            .map_err(|error| CopilotAuthError::Token(error.to_string()))?;
        let payload = response
            .json::<CopilotModelsResponse>()
            .await
            .map_err(|error| CopilotAuthError::Parse(error.to_string()))?;
        Ok(payload
            .data
            .into_iter()
            .filter(|item| item.model_picker_enabled)
            .map(|item| CopilotModel {
                id: item.id,
                name: item.name,
                vendor: item.vendor,
            })
            .collect())
    }

    pub async fn get_valid_token_for_account(
        &self,
        account_id: Option<&str>,
    ) -> Result<String, CopilotAuthError> {
        let resolved_account_id = self
            .resolve_account_id(account_id)
            .await
            .ok_or(CopilotAuthError::GitHubTokenInvalid)?;

        {
            let tokens = self.copilot_tokens.read().await;
            if let Some(token) = tokens.get(&resolved_account_id) {
                if !token.is_expiring_soon() {
                    return Ok(token.token.clone());
                }
            }
        }

        let refresh_lock = self.get_refresh_lock(&resolved_account_id).await;
        let _guard = refresh_lock.lock().await;

        {
            let tokens = self.copilot_tokens.read().await;
            if let Some(token) = tokens.get(&resolved_account_id) {
                if !token.is_expiring_soon() {
                    return Ok(token.token.clone());
                }
            }
        }

        let github_token = {
            let accounts = self.accounts.read().await;
            accounts
                .get(&resolved_account_id)
                .map(|account| account.github_token.clone())
                .ok_or_else(|| CopilotAuthError::AccountNotFound(resolved_account_id.clone()))?
        };

        self.fetch_copilot_token_with_github_token(&github_token, &resolved_account_id)
            .await?;

        let tokens = self.copilot_tokens.read().await;
        tokens
            .get(&resolved_account_id)
            .map(|token| token.token.clone())
            .ok_or_else(|| CopilotAuthError::Token("Refreshed token is missing".to_string()))
    }

    async fn add_account_internal(
        &self,
        github_token: String,
        user: GitHubUser,
    ) -> Result<GitHubAccount, CopilotAuthError> {
        let now = chrono::Utc::now().timestamp();
        let account_id = user.id.to_string();
        let account_data = GitHubAccountData {
            github_token,
            user: user.clone(),
            authenticated_at: now,
        };

        {
            let mut accounts = self.accounts.write().await;
            accounts.insert(account_id.clone(), account_data);
        }
        {
            let mut default_account_id = self.default_account_id.write().await;
            if default_account_id.is_none() {
                *default_account_id = Some(account_id.clone());
            }
        }
        self.save_to_disk().await?;

        Ok(GitHubAccount {
            id: account_id,
            login: user.login,
            avatar_url: user.avatar_url,
            authenticated_at: now,
        })
    }

    async fn resolve_account_id(&self, requested: Option<&str>) -> Option<String> {
        if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
            let accounts = self.accounts.read().await;
            if accounts.contains_key(requested) {
                return Some(requested.to_string());
            }
            return None;
        }
        self.resolve_default_account_id().await
    }

    async fn resolve_default_account_id(&self) -> Option<String> {
        let stored_default = self.default_account_id.read().await.clone();
        let accounts = self.accounts.read().await;
        if let Some(default_account_id) = stored_default {
            if accounts.contains_key(&default_account_id) {
                return Some(default_account_id);
            }
        }
        Self::fallback_default_account_id(&accounts)
    }

    async fn get_refresh_lock(&self, account_id: &str) -> Arc<Mutex<()>> {
        {
            let locks = self.refresh_locks.read().await;
            if let Some(lock) = locks.get(account_id) {
                return Arc::clone(lock);
            }
        }

        let mut locks = self.refresh_locks.write().await;
        Arc::clone(
            locks
                .entry(account_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    async fn fetch_user_info_with_token(
        &self,
        github_token: &str,
    ) -> Result<GitHubUser, CopilotAuthError> {
        let response = self
            .http_client
            .get(GITHUB_USER_URL)
            .header("Authorization", format!("token {github_token}"))
            .header("User-Agent", COPILOT_USER_AGENT)
            .header("Editor-Version", COPILOT_EDITOR_VERSION)
            .header("Editor-Plugin-Version", COPILOT_PLUGIN_VERSION)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(CopilotAuthError::GitHubTokenInvalid);
        }

        response
            .json()
            .await
            .map_err(|error| CopilotAuthError::Parse(error.to_string()))
    }

    async fn fetch_copilot_token_with_github_token(
        &self,
        github_token: &str,
        account_id: &str,
    ) -> Result<(), CopilotAuthError> {
        let response = self
            .http_client
            .get(COPILOT_TOKEN_URL)
            .header("Authorization", format!("token {github_token}"))
            .header("User-Agent", COPILOT_USER_AGENT)
            .header("Editor-Version", COPILOT_EDITOR_VERSION)
            .header("Editor-Plugin-Version", COPILOT_PLUGIN_VERSION)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CopilotAuthError::GitHubTokenInvalid);
        }
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            return Err(CopilotAuthError::NoCopilotSubscription);
        }
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(CopilotAuthError::Token(format!("{status}: {text}")));
        }

        let token_response: CopilotTokenResponse = response
            .json()
            .await
            .map_err(|error| CopilotAuthError::Parse(error.to_string()))?;
        let mut tokens = self.copilot_tokens.write().await;
        tokens.insert(
            account_id.to_string(),
            CopilotToken {
                token: token_response.token,
                expires_at: token_response.expires_at,
            },
        );
        Ok(())
    }

    fn sorted_accounts(
        accounts: &HashMap<String, GitHubAccountData>,
        default_account_id: Option<&str>,
    ) -> Vec<GitHubAccount> {
        let mut account_list: Vec<GitHubAccount> =
            accounts.values().map(GitHubAccount::from).collect();
        account_list.sort_by(|left, right| {
            let left_default = default_account_id == Some(left.id.as_str());
            let right_default = default_account_id == Some(right.id.as_str());
            right_default
                .cmp(&left_default)
                .then_with(|| right.authenticated_at.cmp(&left.authenticated_at))
                .then_with(|| left.login.cmp(&right.login))
        });
        account_list
    }

    fn fallback_default_account_id(
        accounts: &HashMap<String, GitHubAccountData>,
    ) -> Option<String> {
        accounts
            .iter()
            .max_by(|(left_id, left), (right_id, right)| {
                left.authenticated_at
                    .cmp(&right.authenticated_at)
                    .then_with(|| right_id.cmp(left_id))
            })
            .map(|(id, _)| id.clone())
    }

    fn load_from_disk_sync(&self) -> Result<(), CopilotAuthError> {
        if !self.storage_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.storage_path)?;
        let store: CopilotAuthStore = serde_json::from_str(&content)
            .map_err(|error| CopilotAuthError::Parse(error.to_string()))?;

        if let Ok(mut accounts) = self.accounts.try_write() {
            *accounts = store.accounts;
        }
        if let Ok(mut default_account_id) = self.default_account_id.try_write() {
            *default_account_id = store.default_account_id;
        }
        Ok(())
    }

    async fn save_to_disk(&self) -> Result<(), CopilotAuthError> {
        let accounts = self.accounts.read().await.clone();
        let default_account_id = self.resolve_default_account_id().await;
        let store = CopilotAuthStore {
            version: 1,
            accounts,
            default_account_id,
        };
        let content = serde_json::to_string_pretty(&store)
            .map_err(|error| CopilotAuthError::Parse(error.to_string()))?;
        self.write_store_atomic(&content)
    }

    fn write_store_atomic(&self, content: &str) -> Result<(), CopilotAuthError> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let parent = self
            .storage_path
            .parent()
            .ok_or_else(|| CopilotAuthError::Io("Invalid Copilot auth storage path".to_string()))?;
        let file_name = self
            .storage_path
            .file_name()
            .ok_or_else(|| CopilotAuthError::Io("Invalid Copilot auth file name".to_string()))?
            .to_string_lossy()
            .to_string();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_path = parent.join(format!("{file_name}.tmp.{ts}"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&tmp_path)?;
            file.write_all(content.as_bytes())?;
            file.flush()?;
            fs::rename(&tmp_path, &self.storage_path)?;
            fs::set_permissions(&self.storage_path, fs::Permissions::from_mode(0o600))?;
        }

        #[cfg(windows)]
        {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)?;
            file.write_all(content.as_bytes())?;
            file.flush()?;
            if self.storage_path.exists() {
                let _ = fs::remove_file(&self.storage_path);
            }
            fs::rename(&tmp_path, &self.storage_path)?;
        }

        Ok(())
    }
}

pub fn copilot_request_headers(token: &str) -> Vec<(String, String)> {
    vec![
        ("authorization".to_string(), format!("Bearer {token}")),
        (
            "copilot-integration-id".to_string(),
            COPILOT_INTEGRATION_ID.to_string(),
        ),
        (
            "editor-version".to_string(),
            COPILOT_EDITOR_VERSION.to_string(),
        ),
        (
            "editor-plugin-version".to_string(),
            COPILOT_PLUGIN_VERSION.to_string(),
        ),
        ("user-agent".to_string(), COPILOT_USER_AGENT.to_string()),
        (
            "x-github-api-version".to_string(),
            COPILOT_API_VERSION.to_string(),
        ),
    ]
}

pub(crate) fn init_copilot_auth_state(app_handle: &AppHandle) {
    let storage_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cchub")
        .join("copilot_auth.json");
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
    app_handle.manage(CopilotAuthState(Arc::new(CopilotAuthManager::new(
        storage_path,
        proxy_url,
    ))));
}
