use axum::{
    body::Body,
    extract::{Path, State as AxumState},
    http::{Request, Response},
    routing::any,
    Router,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State as TauriState};
use tokio::sync::oneshot;

mod cost;
mod forward;
mod optimizer;
mod profiles;
mod rewriters;
mod upstream;
mod usage;
use cost::{calculate_proxy_total_cost, parse_cost_text};
use forward::forward_proxy_request;
pub(crate) use optimizer::{update_optimizer_config_cache, update_rectifier_config_cache};
use profiles::profile_circuit_key;
use rewriters::{
    rewrite_claude_snapshot, rewrite_codex_snapshot, rewrite_gemini_snapshot,
    rewrite_grok_snapshot, rewrite_hermes_snapshot, rewrite_openclaw_snapshot,
    rewrite_opencode_snapshot,
};
use upstream::{
    build_forward_response, build_forward_response_from_parts, build_json_response_from_value,
    build_proxy_error, build_upstream_request_url, extract_request_insights,
    extract_upstream_target, is_hop_by_hop_header, is_retryable_upstream_status,
    next_proxy_request_id, parse_json_bytes, read_response_body_limited, reqwest_client,
    transform_claude_request_body,
};

use crate::db::DbState;

const LOCAL_PROVIDER_PROXY_SETTINGS_KEY: &str = "local_provider_proxy_settings";
const LOCAL_PROVIDER_PROXY_HOST: &str = "127.0.0.1";
const LOCAL_PROVIDER_PROXY_TOKEN: &str = "cchub-local-proxy";
const DEFAULT_LOCAL_PROVIDER_PROXY_PORT: u16 = 34567;
const MAX_PROXY_BODY_BYTES: usize = 64 * 1024 * 1024;
const MAX_PROXY_RESPONSE_BODY_BYTES: usize = 128 * 1024 * 1024;
const MANAGED_PROXY_TOOLS: [&str; 7] = [
    "claude",
    "codex",
    "gemini",
    "grokbuild",
    "opencode",
    "openclaw",
    "hermes",
];
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalProviderProxySettings {
    pub port: u16,
    pub enabled_apps: Vec<String>,
}

impl Default for LocalProviderProxySettings {
    fn default() -> Self {
        Self {
            port: DEFAULT_LOCAL_PROVIDER_PROXY_PORT,
            enabled_apps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalProviderProxyStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub enabled_apps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum CircuitState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Default)]
pub(super) struct EndpointCircuitState {
    pub(super) state: CircuitState,
    pub(super) consecutive_failures: u32,
    pub(super) consecutive_successes: u32,
    pub(super) open_until: Option<Instant>,
    pub(super) half_open_permit_taken: bool,
}

impl EndpointCircuitState {
    pub(super) fn is_available(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if self.open_until.is_some_and(|until| Instant::now() >= until) {
                    self.state = CircuitState::HalfOpen;
                    self.half_open_permit_taken = true;
                    self.consecutive_successes = 0;
                    return true;
                }
                false
            }
            CircuitState::HalfOpen => false,
        }
    }

    pub(super) fn record_success(&mut self, success_threshold: u32) {
        self.consecutive_failures = 0;
        match self.state {
            CircuitState::HalfOpen => {
                self.consecutive_successes += 1;
                if self.consecutive_successes >= success_threshold {
                    self.state = CircuitState::Closed;
                    self.open_until = None;
                    self.half_open_permit_taken = false;
                } else {
                    self.half_open_permit_taken = false;
                }
            }
            _ => {
                self.state = CircuitState::Closed;
                self.open_until = None;
            }
        }
    }

    pub(super) fn record_failure(&mut self, failure_threshold: u32, timeout_secs: u64) {
        self.consecutive_successes = 0;
        match self.state {
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.open_until = Some(Instant::now() + Duration::from_secs(timeout_secs));
                self.half_open_permit_taken = false;
                self.consecutive_failures = 0;
            }
            _ => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                if self.consecutive_failures >= failure_threshold {
                    self.state = CircuitState::Open;
                    self.open_until = Some(Instant::now() + Duration::from_secs(timeout_secs));
                    self.consecutive_failures = 0;
                }
            }
        }
    }
}

#[derive(Default)]
pub(super) struct LocalProviderProxyRuntimeInner {
    pub(super) port: Option<u16>,
    pub(super) shutdown: Option<oneshot::Sender<()>>,
    pub(super) preferred_base_urls: HashMap<String, String>,
    pub(super) endpoint_circuits: HashMap<String, EndpointCircuitState>,
    pub(super) profile_circuits: HashMap<String, EndpointCircuitState>,
    pub(super) optimizer_config: Option<crate::proxy_optimizer::OptimizerConfig>,
    pub(super) rectifier_config: Option<crate::proxy_optimizer::config::RectifierConfig>,
}

pub(crate) struct LocalProviderProxyRuntime(pub(super) Mutex<LocalProviderProxyRuntimeInner>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitBreakerEntry {
    pub scope: String,
    pub key: String,
    pub state: String,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitBreakerStats {
    pub entries: Vec<CircuitBreakerEntry>,
    pub open_count: usize,
    pub half_open_count: usize,
}

fn circuit_state_label(state: CircuitState) -> &'static str {
    match state {
        CircuitState::Closed => "closed",
        CircuitState::Open => "open",
        CircuitState::HalfOpen => "half_open",
    }
}

fn circuit_entry(scope: &str, key: &str, state: &EndpointCircuitState) -> CircuitBreakerEntry {
    CircuitBreakerEntry {
        scope: scope.to_string(),
        key: key.to_string(),
        state: circuit_state_label(state.state).to_string(),
        consecutive_failures: state.consecutive_failures,
        consecutive_successes: state.consecutive_successes,
        retry_after_ms: state.open_until.map(|until| {
            until
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(u128::from(u64::MAX)) as u64
        }),
    }
}

pub fn get_circuit_breaker_stats(app_handle: &AppHandle) -> Result<CircuitBreakerStats, String> {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let runtime = runtime_state
        .0
        .lock()
        .map_err(|_| "Local provider proxy runtime lock is poisoned".to_string())?;
    let mut entries = runtime
        .profile_circuits
        .iter()
        .map(|(key, state)| circuit_entry("profile", key, state))
        .chain(
            runtime
                .endpoint_circuits
                .iter()
                .map(|(key, state)| circuit_entry("endpoint", key, state)),
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.scope.cmp(&right.scope).then(left.key.cmp(&right.key)));
    let open_count = entries.iter().filter(|entry| entry.state == "open").count();
    let half_open_count = entries
        .iter()
        .filter(|entry| entry.state == "half_open")
        .count();
    Ok(CircuitBreakerStats {
        entries,
        open_count,
        half_open_count,
    })
}

pub fn reset_circuit_breakers(app_handle: &AppHandle) -> Result<usize, String> {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let mut runtime = runtime_state
        .0
        .lock()
        .map_err(|_| "Local provider proxy runtime lock is poisoned".to_string())?;
    let reset_count = runtime.profile_circuits.len() + runtime.endpoint_circuits.len();
    runtime.profile_circuits.clear();
    runtime.endpoint_circuits.clear();
    Ok(reset_count)
}

pub fn reset_circuit_breaker_for_profile(
    app_handle: &AppHandle,
    tool_id: &str,
    profile_id: &str,
) -> Result<usize, String> {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let mut runtime = runtime_state
        .0
        .lock()
        .map_err(|_| "Local provider proxy runtime lock is poisoned".to_string())?;
    let profile_key = profile_circuit_key(tool_id, profile_id);
    let mut reset_count = usize::from(runtime.profile_circuits.remove(&profile_key).is_some());
    let endpoint_prefix = format!("{profile_id}::");
    let endpoint_keys = runtime
        .endpoint_circuits
        .keys()
        .filter(|key| key.starts_with(&endpoint_prefix))
        .cloned()
        .collect::<Vec<_>>();
    reset_count += endpoint_keys
        .into_iter()
        .filter(|key| runtime.endpoint_circuits.remove(key).is_some())
        .count();
    Ok(reset_count)
}

#[derive(Clone)]
struct ProxyRouterState {
    app_handle: AppHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClaudeApiFormat {
    Anthropic,
    OpenAiChat,
    OpenAiResponses,
    GeminiNative,
}

impl ClaudeApiFormat {
    pub(super) fn from_str(value: &str) -> Self {
        match value {
            "openai_chat" => Self::OpenAiChat,
            "openai_responses" => Self::OpenAiResponses,
            "gemini_native" => Self::GeminiNative,
            _ => Self::Anthropic,
        }
    }

    pub(super) fn needs_transform(self) -> bool {
        matches!(
            self,
            Self::OpenAiChat | Self::OpenAiResponses | Self::GeminiNative
        )
    }
}

#[derive(Debug, Clone)]
pub(super) struct UpstreamTarget {
    pub(super) profile_id: String,
    pub(super) profile_name: String,
    pub(super) base_url: String,
    pub(super) use_full_url: bool,
    pub(super) candidate_base_urls: Vec<String>,
    pub(super) headers: Vec<(String, String)>,
    pub(super) request_header_overrides: Vec<(String, String)>,
    pub(super) request_body_override: Option<serde_json::Value>,
    pub(super) claude_api_format: Option<ClaudeApiFormat>,
    pub(super) is_github_copilot: bool,
    pub(super) is_codex_oauth: bool,
    pub(super) cost_multiplier: f64,
}

#[derive(Debug, Clone)]
pub(super) struct ProfileCandidate {
    pub(super) profile_id: String,
    pub(super) profile_name: String,
    pub(super) snapshot: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ProxyRequestInsights {
    pub(super) request_model: Option<String>,
    pub(super) is_streaming: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ProxyUsageMetrics {
    pub(super) response_model: Option<String>,
    pub(super) input_tokens: u64,
    pub(super) output_tokens: u64,
    pub(super) cache_read_tokens: u64,
    pub(super) cache_creation_tokens: u64,
}

pub(crate) fn init_local_provider_proxy_runtime(app_handle: &AppHandle) {
    app_handle.manage(LocalProviderProxyRuntime(Mutex::new(
        LocalProviderProxyRuntimeInner::default(),
    )));
}

pub(super) fn current_profile_setting_key(tool_id: &str) -> String {
    format!("current_profile_{tool_id}")
}

fn normalize_local_provider_proxy_settings(
    settings: LocalProviderProxySettings,
) -> LocalProviderProxySettings {
    let mut seen = HashSet::new();
    let enabled_apps = settings
        .enabled_apps
        .into_iter()
        .filter_map(|tool_id| {
            let trimmed = tool_id.trim().to_ascii_lowercase();
            if MANAGED_PROXY_TOOLS.contains(&trimmed.as_str()) && seen.insert(trimmed.clone()) {
                Some(trimmed)
            } else {
                None
            }
        })
        .collect();

    let port = if settings.port < 1024 {
        DEFAULT_LOCAL_PROVIDER_PROXY_PORT
    } else {
        settings.port
    };

    LocalProviderProxySettings { port, enabled_apps }
}

pub(crate) fn read_local_provider_proxy_settings_from_conn(
    conn: &Connection,
) -> LocalProviderProxySettings {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![LOCAL_PROVIDER_PROXY_SETTINGS_KEY],
            |row| row.get(0),
        )
        .ok();

    raw.and_then(|value| serde_json::from_str::<LocalProviderProxySettings>(&value).ok())
        .map(normalize_local_provider_proxy_settings)
        .unwrap_or_default()
}

fn write_local_provider_proxy_settings_to_conn(
    conn: &Connection,
    settings: &LocalProviderProxySettings,
) -> Result<(), String> {
    let payload = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![LOCAL_PROVIDER_PROXY_SETTINGS_KEY, payload],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn local_provider_proxy_base_url(port: u16) -> String {
    format!("http://{LOCAL_PROVIDER_PROXY_HOST}:{port}/proxy")
}

fn local_provider_proxy_tool_base_url(port: u16, tool_id: &str) -> String {
    format!("{}/{}", local_provider_proxy_base_url(port), tool_id)
}

pub(crate) fn materialize_tool_snapshot_for_runtime(
    conn: &Connection,
    tool_id: &str,
    snapshot: &str,
) -> Result<String, String> {
    let settings = read_local_provider_proxy_settings_from_conn(conn);
    if !settings.enabled_apps.iter().any(|item| item == tool_id) {
        return Ok(snapshot.to_string());
    }

    match tool_id {
        "claude" => rewrite_claude_snapshot(snapshot, settings.port),
        "codex" => rewrite_codex_snapshot(snapshot, settings.port),
        "gemini" => rewrite_gemini_snapshot(snapshot, settings.port),
        "grokbuild" => rewrite_grok_snapshot(snapshot, settings.port),
        "openclaw" => rewrite_openclaw_snapshot(snapshot, settings.port),
        "hermes" => rewrite_hermes_snapshot(snapshot, settings.port),
        "opencode" => rewrite_opencode_snapshot(snapshot, settings.port),
        _ => Ok(snapshot.to_string()),
    }
}

fn stop_local_provider_proxy_locked(runtime: &mut LocalProviderProxyRuntimeInner) {
    if let Some(shutdown) = runtime.shutdown.take() {
        let _ = shutdown.send(());
    }
    runtime.port = None;
}

fn spawn_local_provider_proxy_server(
    app_handle: AppHandle,
    port: u16,
) -> Result<oneshot::Sender<()>, String> {
    // Bind synchronously so "port in use" errors surface immediately from the
    // caller. The std listener is handed off to the async task below, where
    // we convert it into a tokio listener inside a runtime context (required
    // by tokio::net::TcpListener::from_std, which panics with "there is no
    // reactor running" if called from the synchronous Tauri setup hook).
    let std_listener = std::net::TcpListener::bind((LOCAL_PROVIDER_PROXY_HOST, port))
        .map_err(|e| format!("Failed to bind local provider proxy on port {port}: {e}"))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| e.to_string())?;

    let router = Router::new()
        .route("/proxy/:tool_id", any(handle_proxy_root))
        .route("/proxy/:tool_id/*path", any(handle_proxy_path))
        .with_state(ProxyRouterState {
            app_handle: app_handle.clone(),
        });

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    crate::utils::append_runtime_log(
        "info",
        "provider_proxy",
        &format!(
            "Starting local provider proxy on {}:{}",
            LOCAL_PROVIDER_PROXY_HOST, port
        ),
    );

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(listener) => listener,
            Err(error) => {
                crate::utils::append_runtime_log(
                    "error",
                    "provider_proxy",
                    &format!("Failed to convert listener to tokio: {error}"),
                );
                return;
            }
        };
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(error) = server.await {
            crate::utils::append_runtime_log(
                "error",
                "provider_proxy",
                &format!("Local provider proxy stopped unexpectedly: {error}"),
            );
        }
    });

    Ok(shutdown_tx)
}

pub(crate) fn sync_local_provider_proxy_server(app_handle: &AppHandle) -> Result<(), String> {
    let settings = {
        let db = app_handle.state::<DbState>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_local_provider_proxy_settings_from_conn(&conn)
    };

    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let mut runtime = runtime_state.0.lock().map_err(|e| e.to_string())?;
    let should_run = !settings.enabled_apps.is_empty();

    if !should_run {
        if runtime.port.is_some() {
            crate::utils::append_runtime_log(
                "info",
                "provider_proxy",
                "Stopping local provider proxy",
            );
        }
        stop_local_provider_proxy_locked(&mut runtime);
        return Ok(());
    }

    if runtime.port == Some(settings.port) {
        return Ok(());
    }

    stop_local_provider_proxy_locked(&mut runtime);
    let shutdown = spawn_local_provider_proxy_server(app_handle.clone(), settings.port)?;
    runtime.port = Some(settings.port);
    runtime.shutdown = Some(shutdown);
    Ok(())
}

fn build_local_provider_proxy_status(
    app_handle: &AppHandle,
    settings: LocalProviderProxySettings,
) -> Result<LocalProviderProxyStatus, String> {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let runtime = runtime_state.0.lock().map_err(|e| e.to_string())?;
    Ok(LocalProviderProxyStatus {
        running: runtime.port == Some(settings.port) && !settings.enabled_apps.is_empty(),
        host: LOCAL_PROVIDER_PROXY_HOST.to_string(),
        port: settings.port,
        base_url: local_provider_proxy_base_url(settings.port),
        enabled_apps: settings.enabled_apps,
    })
}

async fn handle_proxy_root(
    Path(tool_id): Path<String>,
    AxumState(state): AxumState<ProxyRouterState>,
    request: Request<Body>,
) -> Response<Body> {
    forward_proxy_request(state.app_handle, tool_id, String::new(), request).await
}

async fn handle_proxy_path(
    Path((tool_id, path)): Path<(String, String)>,
    AxumState(state): AxumState<ProxyRouterState>,
    request: Request<Body>,
) -> Response<Body> {
    forward_proxy_request(state.app_handle, tool_id, path, request).await
}

fn reapply_active_profiles(conn: &Connection) -> Result<(), String> {
    for tool_id in MANAGED_PROXY_TOOLS {
        let setting_key = current_profile_setting_key(tool_id);
        let active_profile_id: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                rusqlite::params![setting_key],
                |row| row.get(0),
            )
            .ok();

        if let Some(profile_id) = active_profile_id {
            // Startup reapply: preserve user-managed keys (e.g., codex personality /
            // model_reasoning_effort) written through the Tools page. The only reason
            // we reapply at startup is to rewrite the proxy base_url — we must not
            // clobber unrelated per-tool settings the user edited after the profile
            // was captured.
            crate::commands::extra_commands::apply_config_profile_from_conn(
                conn,
                &profile_id,
                true,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn initialize_local_provider_proxy(app_handle: &AppHandle) -> Result<(), String> {
    sync_local_provider_proxy_server(app_handle)?;
    let settings = {
        let db = app_handle.state::<DbState>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_local_provider_proxy_settings_from_conn(&conn)
    };
    if settings.enabled_apps.is_empty() {
        return Ok(());
    }

    let db = app_handle.state::<DbState>();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    reapply_active_profiles(&conn)
}

#[tauri::command]
pub fn get_local_provider_proxy_settings(
    db: TauriState<'_, DbState>,
) -> Result<LocalProviderProxySettings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_local_provider_proxy_settings_from_conn(&conn))
}

#[tauri::command]
pub fn get_local_provider_proxy_status(
    app_handle: tauri::AppHandle,
    db: TauriState<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let settings = read_local_provider_proxy_settings_from_conn(&conn);
    drop(conn);
    build_local_provider_proxy_status(&app_handle, settings)
}

#[tauri::command]
pub fn set_local_provider_proxy_settings(
    settings: LocalProviderProxySettings,
    app_handle: tauri::AppHandle,
    db: TauriState<'_, DbState>,
) -> Result<LocalProviderProxyStatus, String> {
    let normalized = normalize_local_provider_proxy_settings(settings);

    let previous = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        read_local_provider_proxy_settings_from_conn(&conn)
    };

    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        write_local_provider_proxy_settings_to_conn(&conn, &normalized)?;
    }

    if let Err(error) = sync_local_provider_proxy_server(&app_handle) {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        write_local_provider_proxy_settings_to_conn(&conn, &previous)?;
        drop(conn);
        let _ = sync_local_provider_proxy_server(&app_handle);
        return Err(error);
    }

    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        reapply_active_profiles(&conn)?;
    }

    crate::utils::append_runtime_log(
        "info",
        "provider_proxy",
        &format!(
            "Updated local provider proxy settings: port={}, apps={}",
            normalized.port,
            normalized.enabled_apps.join(",")
        ),
    );

    build_local_provider_proxy_status(&app_handle, normalized)
}

#[cfg(test)]
mod tests {
    use super::{CircuitState, EndpointCircuitState};
    use std::time::Duration;

    #[test]
    fn circuit_opens_after_threshold_and_exposes_retry_window() {
        let mut state = EndpointCircuitState::default();
        state.record_failure(2, 30);
        assert_eq!(state.state, CircuitState::Closed);
        assert_eq!(state.consecutive_failures, 1);

        state.record_failure(2, 30);
        assert_eq!(state.state, CircuitState::Open);
        assert!(state.open_until.is_some());
        assert!(state.open_until.unwrap() > std::time::Instant::now());
    }

    #[test]
    fn half_open_requires_recovery_threshold() {
        let mut state = EndpointCircuitState {
            state: CircuitState::Open,
            open_until: Some(std::time::Instant::now() - Duration::from_secs(1)),
            ..Default::default()
        };
        assert!(state.is_available());
        assert_eq!(state.state, CircuitState::HalfOpen);
        assert!(!state.is_available(), "only one half-open probe may run");

        state.record_success(2);
        assert_eq!(state.state, CircuitState::HalfOpen);
        state.record_success(2);
        assert_eq!(state.state, CircuitState::Closed);
        assert!(state.open_until.is_none());
    }

    #[test]
    fn half_open_failure_reopens_circuit() {
        let mut state = EndpointCircuitState {
            state: CircuitState::HalfOpen,
            ..Default::default()
        };
        state.record_failure(3, 10);
        assert_eq!(state.state, CircuitState::Open);
        assert!(state.open_until.is_some());
        assert_eq!(state.consecutive_failures, 0);
    }
}
