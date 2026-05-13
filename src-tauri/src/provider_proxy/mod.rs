use axum::{
    body::{to_bytes, Body},
    extract::{Path, State as AxumState},
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    routing::any,
    Router,
};
use bytes::Bytes;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State as TauriState};
use tokio::sync::oneshot;
use uuid::Uuid;

mod cost;
mod optimizer;
mod rewriters;
mod usage;
use cost::{
    calculate_proxy_total_cost, extract_error_message_from_response, log_proxy_request,
    parse_cost_text, transform_claude_response_body,
};
use optimizer::{apply_proxy_optimizers, read_optimizer_config, read_rectifier_config};
pub(crate) use optimizer::{update_optimizer_config_cache, update_rectifier_config_cache};
use rewriters::{
    rewrite_claude_snapshot, rewrite_codex_snapshot, rewrite_gemini_snapshot,
    rewrite_hermes_snapshot, rewrite_openclaw_snapshot, rewrite_opencode_snapshot,
};
use usage::{create_usage_tracking_stream, parse_usage_metrics_from_response};

use crate::copilot_auth::{self, CopilotAuthState};
use crate::db::DbState;
use crate::provider_proxy_transform::{
    anthropic_to_openai, anthropic_to_responses, create_anthropic_sse_stream,
    create_anthropic_sse_stream_from_gemini, create_anthropic_sse_stream_from_responses,
    openai_error_to_anthropic, rectify_anthropic_request_bytes,
};

const LOCAL_PROVIDER_PROXY_SETTINGS_KEY: &str = "local_provider_proxy_settings";
const LOCAL_PROVIDER_PROXY_HOST: &str = "127.0.0.1";
const LOCAL_PROVIDER_PROXY_TOKEN: &str = "cchub-local-proxy";
const DEFAULT_LOCAL_PROVIDER_PROXY_PORT: u16 = 34567;
const MAX_PROXY_BODY_BYTES: usize = 64 * 1024 * 1024;
const MANAGED_PROXY_TOOLS: [&str; 6] = [
    "claude", "codex", "gemini", "opencode", "openclaw", "hermes",
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
enum CircuitState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Default)]
struct EndpointCircuitState {
    state: CircuitState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    open_until: Option<Instant>,
    half_open_permit_taken: bool,
}

impl EndpointCircuitState {
    fn is_available(&mut self) -> bool {
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

    fn record_success(&mut self, success_threshold: u32) {
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

    fn record_failure(&mut self, failure_threshold: u32, timeout_secs: u64) {
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
    fn from_str(value: &str) -> Self {
        match value {
            "openai_chat" => Self::OpenAiChat,
            "openai_responses" => Self::OpenAiResponses,
            "gemini_native" => Self::GeminiNative,
            _ => Self::Anthropic,
        }
    }

    fn needs_transform(self) -> bool {
        matches!(
            self,
            Self::OpenAiChat | Self::OpenAiResponses | Self::GeminiNative
        )
    }
}

#[derive(Debug, Clone)]
pub(super) struct UpstreamTarget {
    profile_id: String,
    profile_name: String,
    base_url: String,
    use_full_url: bool,
    candidate_base_urls: Vec<String>,
    headers: Vec<(String, String)>,
    claude_api_format: Option<ClaudeApiFormat>,
    is_github_copilot: bool,
    cost_multiplier: f64,
}

#[derive(Debug, Clone)]
struct ProfileCandidate {
    profile_id: String,
    profile_name: String,
    snapshot: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ProxyRequestInsights {
    request_model: Option<String>,
    is_streaming: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ProxyUsageMetrics {
    response_model: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
}

pub(crate) fn init_local_provider_proxy_runtime(app_handle: &AppHandle) {
    app_handle.manage(LocalProviderProxyRuntime(Mutex::new(
        LocalProviderProxyRuntimeInner::default(),
    )));
}

fn current_profile_setting_key(tool_id: &str) -> String {
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

fn active_profile_id_for_tool(conn: &Connection, tool_id: &str) -> Result<String, String> {
    let setting_key = current_profile_setting_key(tool_id);
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        rusqlite::params![setting_key],
        |row| row.get(0),
    )
    .map_err(|_| format!("No active provider profile selected for {tool_id}"))
}

fn read_profile_candidates_for_tool(
    conn: &Connection,
    tool_id: &str,
) -> Result<Vec<ProfileCandidate>, String> {
    let active_profile_id = active_profile_id_for_tool(conn, tool_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, config_snapshot
             FROM config_profiles
             WHERE tool_id = ?1
             ORDER BY COALESCE(sort_order, 0) ASC, updated_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![tool_id], |row| {
            Ok(ProfileCandidate {
                profile_id: row.get(0)?,
                profile_name: row.get(1)?,
                snapshot: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut profiles = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if profiles.is_empty() {
        return Err(format!("No provider profiles available for {tool_id}"));
    }
    if let Some(index) = profiles
        .iter()
        .position(|profile| profile.profile_id == active_profile_id)
    {
        if index > 0 {
            let active = profiles.remove(index);
            profiles.insert(0, active);
        }
    } else {
        return Err(format!(
            "Active provider profile missing for {tool_id}: {}",
            active_profile_id
        ));
    }

    Ok(profiles)
}

fn default_base_url_for_claude(api_format: &str) -> Option<String> {
    if api_format == "anthropic" {
        Some("https://api.anthropic.com".to_string())
    } else {
        None
    }
}

fn default_base_url_for_gemini() -> String {
    "https://generativelanguage.googleapis.com/v1beta".to_string()
}

fn default_base_url_for_codex() -> String {
    "https://api.openai.com/v1".to_string()
}

fn is_claude_messages_path(relative_path: &str) -> bool {
    matches!(
        relative_path.trim().trim_matches('/'),
        "v1/messages" | "claude/v1/messages"
    )
}

fn strip_beta_query(query: Option<&str>) -> Option<String> {
    let filtered = query
        .map(|raw| {
            raw.split('&')
                .filter(|pair| {
                    !pair.is_empty()
                        && pair
                            .split_once('=')
                            .map(|(key, _)| !key.eq_ignore_ascii_case("beta"))
                            .unwrap_or(true)
                })
                .collect::<Vec<_>>()
                .join("&")
        })
        .filter(|value| !value.is_empty());

    filtered
}

fn rewrite_claude_request_target(
    relative_path: &str,
    query: Option<&str>,
    api_format: ClaudeApiFormat,
    is_github_copilot: bool,
    body_bytes: Option<&[u8]>,
) -> (String, Option<String>) {
    if !api_format.needs_transform() || !is_claude_messages_path(relative_path) {
        return (relative_path.to_string(), query.map(str::to_string));
    }

    let target_path = match api_format {
        ClaudeApiFormat::OpenAiChat if is_github_copilot => "chat/completions".to_string(),
        ClaudeApiFormat::OpenAiChat => "v1/chat/completions".to_string(),
        ClaudeApiFormat::OpenAiResponses => "v1/responses".to_string(),
        ClaudeApiFormat::GeminiNative => {
            let model = body_bytes
                .and_then(|b| serde_json::from_slice::<Value>(b).ok())
                .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(String::from))
                .unwrap_or_else(|| "gemini-2.5-flash".to_string());
            let model_id = model
                .strip_prefix('/')
                .unwrap_or(&model)
                .strip_prefix("models/")
                .unwrap_or(&model);
            let stream = body_bytes
                .and_then(|b| serde_json::from_slice::<Value>(b).ok())
                .and_then(|v| v.get("stream").and_then(|s| s.as_bool()))
                .unwrap_or(false);
            crate::gemini_transform::build_gemini_endpoint(model_id, stream)
        }
        ClaudeApiFormat::Anthropic => relative_path.to_string(),
    };

    (target_path, strip_beta_query(query))
}

fn should_strip_claude_transform_header(
    name: &str,
    api_format: Option<ClaudeApiFormat>,
    relative_path: &str,
) -> bool {
    matches!(api_format, Some(format) if format.needs_transform() && is_claude_messages_path(relative_path))
        && name.to_ascii_lowercase().starts_with("anthropic-")
}

fn canonicalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn extract_metadata_endpoint_candidates(parsed: &Value) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    let Some(items) = parsed
        .get("metadata")
        .and_then(|value| value.get("endpointCandidates"))
        .and_then(|value| value.as_array())
    else {
        return candidates;
    };

    for item in items {
        let Some(candidate) = item.as_str() else {
            continue;
        };
        let normalized = canonicalize_base_url(candidate);
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        candidates.push(normalized);
    }

    candidates
}

fn extract_metadata_object(parsed: &Value) -> serde_json::Map<String, Value> {
    parsed
        .get("metadata")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default()
}

fn extract_provider_type(parsed: &Value) -> Option<String> {
    extract_metadata_object(parsed)
        .get("providerType")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn extract_use_full_url(parsed: &Value) -> bool {
    extract_metadata_object(parsed)
        .get("useFullUrl")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn extract_cost_multiplier(parsed: &Value) -> f64 {
    let metadata = extract_metadata_object(parsed);
    let value = metadata.get("costMultiplier");
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(1.0),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or(1.0),
        _ => 1.0,
    }
    .clamp(0.0, 1_000_000.0)
}

fn extract_copilot_account_id(parsed: &Value) -> Option<String> {
    let metadata = extract_metadata_object(parsed);
    metadata
        .get("authBinding")
        .and_then(|value| {
            value
                .get("authProvider")
                .and_then(|item| item.as_str())
                .map(|provider| (value, provider))
        })
        .and_then(|(value, provider)| {
            if provider == "github_copilot" {
                value
                    .get("accountId")
                    .and_then(|item| item.as_str())
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
            } else {
                None
            }
        })
        .or_else(|| {
            metadata
                .get("githubAccountId")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn filter_endpoint_candidates(primary_base_url: &str, candidates: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    seen.insert(canonicalize_base_url(primary_base_url));

    let mut filtered = Vec::new();
    for candidate in candidates {
        let normalized = canonicalize_base_url(&candidate);
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        filtered.push(normalized);
    }
    filtered
}

fn profile_circuit_key(tool_id: &str, profile_id: &str) -> String {
    format!("{tool_id}::{profile_id}")
}

fn ordered_profile_candidates(
    app_handle: &AppHandle,
    conn: &Connection,
    tool_id: &str,
) -> Result<Vec<ProfileCandidate>, String> {
    let ordered = read_profile_candidates_for_tool(conn, tool_id)?;
    let available = app_handle
        .state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .ok()
        .map(|mut runtime| {
            ordered
                .iter()
                .filter(|profile| {
                    let key = profile_circuit_key(tool_id, &profile.profile_id);
                    match runtime.profile_circuits.get_mut(&key) {
                        Some(state) => state.is_available(),
                        None => true,
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
        });

    if let Some(available) = available {
        if !available.is_empty() {
            return Ok(available);
        }
    }

    Ok(ordered)
}

fn ordered_upstream_base_urls(app_handle: &AppHandle, upstream: &UpstreamTarget) -> Vec<String> {
    let primary = canonicalize_base_url(&upstream.base_url);
    let mut ordered = Vec::with_capacity(1 + upstream.candidate_base_urls.len());
    ordered.push(primary.clone());
    ordered.extend(
        upstream
            .candidate_base_urls
            .iter()
            .map(|candidate| canonicalize_base_url(candidate))
            .filter(|candidate| !candidate.is_empty() && candidate != &primary),
    );

    let preferred = app_handle
        .state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .ok()
        .and_then(|runtime| {
            runtime
                .preferred_base_urls
                .get(&upstream.profile_id)
                .cloned()
        })
        .map(|value| canonicalize_base_url(&value));

    if let Some(preferred) = preferred {
        if let Some(index) = ordered.iter().position(|value| value == &preferred) {
            if index > 0 {
                let preferred_value = ordered.remove(index);
                ordered.insert(0, preferred_value);
            }
        }
    }

    let available = app_handle
        .state::<LocalProviderProxyRuntime>()
        .0
        .lock()
        .ok()
        .map(|mut runtime| {
            ordered
                .iter()
                .filter(|base_url| {
                    let key = endpoint_circuit_key(&upstream.profile_id, base_url);
                    match runtime.endpoint_circuits.get_mut(&key) {
                        Some(state) => state.is_available(),
                        None => true,
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
        });

    if let Some(available) = available {
        if !available.is_empty() {
            return available;
        }
    }

    ordered
}

fn endpoint_circuit_key(profile_id: &str, base_url: &str) -> String {
    format!("{profile_id}::{}", canonicalize_base_url(base_url))
}

fn remember_preferred_upstream_base_url(
    app_handle: &AppHandle,
    profile_id: &str,
    primary_base_url: &str,
    selected_base_url: &str,
) {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let Ok(mut runtime) = runtime_state.0.lock() else {
        return;
    };

    let primary = canonicalize_base_url(primary_base_url);
    let selected = canonicalize_base_url(selected_base_url);
    if selected.is_empty() || selected == primary {
        runtime.preferred_base_urls.remove(profile_id);
    } else {
        runtime
            .preferred_base_urls
            .insert(profile_id.to_string(), selected);
    }
}

fn record_profile_success(
    app_handle: &AppHandle,
    tool_id: &str,
    profile_id: &str,
    config: &crate::proxy_optimizer::OptimizerConfig,
) {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let Ok(mut runtime) = runtime_state.0.lock() else {
        return;
    };
    let key = profile_circuit_key(tool_id, profile_id);
    let state = runtime.profile_circuits.entry(key).or_default();
    state.record_success(config.circuit_success_threshold);
}

fn record_profile_failure(
    app_handle: &AppHandle,
    tool_id: &str,
    profile_id: &str,
    profile_name: &str,
    config: &crate::proxy_optimizer::OptimizerConfig,
) {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let Ok(mut runtime) = runtime_state.0.lock() else {
        return;
    };

    let key = profile_circuit_key(tool_id, profile_id);
    let state = runtime.profile_circuits.entry(key).or_default();
    let was_open = state.state == CircuitState::Open;
    state.record_failure(
        config.circuit_failure_threshold,
        config.circuit_timeout_secs,
    );
    if !was_open && state.state == CircuitState::Open {
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            &format!(
                "Proxy profile circuit opened [{tool_id}] {} ({}) for {}s",
                profile_name, profile_id, config.circuit_timeout_secs
            ),
        );
    }
}

fn record_endpoint_success(
    app_handle: &AppHandle,
    upstream: &UpstreamTarget,
    base_url: &str,
    config: &crate::proxy_optimizer::OptimizerConfig,
) {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let Ok(mut runtime) = runtime_state.0.lock() else {
        return;
    };
    let key = endpoint_circuit_key(&upstream.profile_id, base_url);
    let state = runtime.endpoint_circuits.entry(key).or_default();
    state.record_success(config.circuit_success_threshold);
}

fn record_endpoint_failure(
    app_handle: &AppHandle,
    tool_id: &str,
    upstream: &UpstreamTarget,
    base_url: &str,
    config: &crate::proxy_optimizer::OptimizerConfig,
) {
    let runtime_state = app_handle.state::<LocalProviderProxyRuntime>();
    let Ok(mut runtime) = runtime_state.0.lock() else {
        return;
    };

    let key = endpoint_circuit_key(&upstream.profile_id, base_url);
    let state = runtime.endpoint_circuits.entry(key).or_default();
    let was_open = state.state == CircuitState::Open;
    state.record_failure(
        config.circuit_failure_threshold,
        config.circuit_timeout_secs,
    );
    if !was_open && state.state == CircuitState::Open {
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            &format!(
                "Proxy circuit opened [{tool_id}] {} (profile {}) @ {} for {}s",
                upstream.profile_name, upstream.profile_id, base_url, config.circuit_timeout_secs
            ),
        );
    }
}

fn is_retryable_upstream_status(status: StatusCode) -> bool {
    matches!(
        status.as_u16(),
        408 | 409 | 425 | 429 | 500 | 502 | 503 | 504
    )
}

async fn extract_upstream_target(
    app_handle: &AppHandle,
    tool_id: &str,
    profile_id: String,
    profile_name: String,
    snapshot: &str,
) -> Result<UpstreamTarget, String> {
    let parsed: Value = serde_json::from_str(snapshot).map_err(|e| e.to_string())?;
    let metadata_candidates = extract_metadata_endpoint_candidates(&parsed);
    let provider_type = extract_provider_type(&parsed);
    let is_github_copilot = provider_type.as_deref() == Some("github_copilot");
    let cost_multiplier = extract_cost_multiplier(&parsed);
    let use_full_url = extract_use_full_url(&parsed);

    match tool_id {
        "claude" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let api_format = env
                .get("ANTHROPIC_API_FORMAT")
                .and_then(|value| value.as_str())
                .unwrap_or("anthropic");
            let base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| default_base_url_for_claude(api_format))
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define a Claude upstream base URL")
                })?;
            let headers = if is_github_copilot {
                let account_id = extract_copilot_account_id(&parsed);
                let manager = app_handle.state::<CopilotAuthState>().0.clone();
                let token = manager
                    .get_valid_token_for_account(account_id.as_deref())
                    .await
                    .map_err(|error| {
                        format!(
                            "GitHub Copilot auth is not ready for provider {profile_name}: {error}"
                        )
                    })?;
                copilot_auth::copilot_request_headers(&token)
            } else {
                let token = env
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .or_else(|| env.get("ANTHROPIC_API_KEY"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        format!("Provider {profile_name} does not define a Claude API token")
                    })?;

                if api_format == "anthropic" {
                    vec![
                        ("x-api-key".to_string(), token.to_string()),
                        ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ]
                } else {
                    vec![("authorization".to_string(), format!("Bearer {token}"))]
                }
            };
            let claude_api_format = ClaudeApiFormat::from_str(api_format);

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                claude_api_format: Some(claude_api_format),
                is_github_copilot,
                cost_multiplier,
            })
        }
        "codex" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let base_url =
                extract_toml_string(config, "base_url").unwrap_or_else(default_base_url_for_codex);
            let token = parsed
                .get("auth")
                .and_then(|value| value.get("OPENAI_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define an OPENAI_API_KEY")
                })?;

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers: vec![("authorization".to_string(), format!("Bearer {token}"))],
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        "gemini" => {
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let base_url = env
                .get("GOOGLE_GEMINI_BASE_URL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(default_base_url_for_gemini);
            let token = env
                .get("GEMINI_API_KEY")
                .or_else(|| env.get("GOOGLE_API_KEY"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define a Gemini API key")
                })?;

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers: vec![("x-goog-api-key".to_string(), token.to_string())],
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        "openclaw" => {
            let protocol = parsed
                .get("api")
                .and_then(|value| value.as_str())
                .unwrap_or("openai-completions");
            let base_url = parsed
                .get("baseUrl")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define an OpenClaw baseUrl")
                })?
                .to_string();
            let token = parsed
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define an OpenClaw API key")
                })?;
            let headers = match protocol {
                "anthropic-messages" => vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ],
                "google-generative-ai" => vec![("x-goog-api-key".to_string(), token.to_string())],
                _ => vec![("authorization".to_string(), format!("Bearer {token}"))],
            };

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        "hermes" => {
            let config = parsed
                .get("config")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let model = config
                .get("model")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let env = parsed
                .get("env")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let provider = model
                .get("provider")
                .and_then(|value| value.as_str())
                .unwrap_or("custom");
            let base_url = model
                .get("base_url")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define a Hermes base_url")
                })?
                .to_string();
            let env_key = parsed
                .get("metadata")
                .and_then(|value| value.get("hermesApiKeyEnv"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    crate::hermes::providers::default_env_key_for_provider(provider)
                        .map(str::to_string)
                })
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define a Hermes API key env")
                })?;
            let token = env
                .get(&env_key)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define Hermes API key {env_key}")
                })?;
            let headers = if provider == "gemini" {
                vec![("x-goog-api-key".to_string(), token.to_string())]
            } else if provider == "anthropic" {
                vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ]
            } else {
                vec![("authorization".to_string(), format!("Bearer {token}"))]
            };

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        "opencode" => {
            let npm = parsed
                .get("npm")
                .and_then(|value| value.as_str())
                .unwrap_or("@ai-sdk/openai-compatible");
            let options = parsed
                .get("options")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default();
            let base_url = options
                .get("baseURL")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if npm.contains("anthropic") {
                        "https://api.anthropic.com".to_string()
                    } else if npm.contains("google") {
                        default_base_url_for_gemini()
                    } else {
                        default_base_url_for_codex()
                    }
                });
            let token = options
                .get("apiKey")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!("Provider {profile_name} does not define an OpenCode API key")
                })?;
            let headers = if npm.contains("anthropic") {
                vec![
                    ("x-api-key".to_string(), token.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ]
            } else if npm.contains("google") {
                vec![("x-goog-api-key".to_string(), token.to_string())]
            } else {
                vec![("authorization".to_string(), format!("Bearer {token}"))]
            };

            Ok(UpstreamTarget {
                profile_id,
                profile_name,
                candidate_base_urls: filter_endpoint_candidates(&base_url, metadata_candidates),
                base_url,
                use_full_url,
                headers,
                claude_api_format: None,
                is_github_copilot: false,
                cost_multiplier,
            })
        }
        _ => Err(format!("Unsupported proxy tool: {tool_id}")),
    }
}

fn extract_toml_string(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(key) {
            return None;
        }
        let (_, raw) = trimmed.split_once('=')?;
        let value = raw.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn build_upstream_request_url(
    base_url: &str,
    relative_path: &str,
    query: Option<&str>,
    use_full_url: bool,
) -> String {
    if use_full_url {
        let trimmed = base_url.trim();
        if let Some(query) = query.filter(|value| !value.is_empty()) {
            if trimmed.contains('?') {
                return trimmed.to_string();
            }
            return format!("{trimmed}?{query}");
        }
        return trimmed.to_string();
    }
    let base = base_url.trim().trim_end_matches('/');
    let relative = relative_path.trim_start_matches('/');
    let adjusted = if relative.is_empty() || base.ends_with(&format!("/{relative}")) {
        String::new()
    } else if let Some(stripped) = relative.strip_prefix("v1/") {
        if base.ends_with("/v1") {
            stripped.to_string()
        } else {
            relative.to_string()
        }
    } else if relative == "v1" && base.ends_with("/v1") {
        String::new()
    } else if let Some(stripped) = relative.strip_prefix("v1beta/") {
        if base.ends_with("/v1beta") {
            stripped.to_string()
        } else {
            relative.to_string()
        }
    } else if relative == "v1beta" && base.ends_with("/v1beta") {
        String::new()
    } else {
        relative.to_string()
    };

    let mut url = if adjusted.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{adjusted}")
    };

    if let Some(query) = query.filter(|value| !value.is_empty()) {
        url.push('?');
        url.push_str(query);
    }

    url
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name,
        "host"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "authorization"
            | "x-api-key"
            | "x-goog-api-key"
    )
}

fn build_proxy_error(status: StatusCode, message: String) -> Response<Body> {
    (status, message).into_response()
}

fn build_forward_response_from_parts(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: Body,
) -> Response<Body> {
    let mut response = Response::builder().status(status);

    for (name, value) in headers {
        if is_hop_by_hop_header(name.as_str()) {
            continue;
        }
        response = response.header(name, value);
    }

    match response.body(body) {
        Ok(response) => response,
        Err(error) => build_proxy_error(
            StatusCode::BAD_GATEWAY,
            format!("Failed to build proxy response: {error}"),
        ),
    }
}

fn build_forward_response(upstream_response: reqwest::Response) -> Response<Body> {
    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let body = Body::from_stream(upstream_response.bytes_stream());
    build_forward_response_from_parts(status, &headers, body)
}

fn build_json_response_from_value(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    value: &Value,
) -> Response<Body> {
    let payload = match serde_json::to_vec(value) {
        Ok(payload) => payload,
        Err(error) => {
            return build_proxy_error(
                StatusCode::BAD_GATEWAY,
                format!("Failed to serialize proxy response body: {error}"),
            );
        }
    };

    let mut response = Response::builder().status(status);
    let mut has_content_type = false;
    for (name, header_value) in headers {
        if is_hop_by_hop_header(name.as_str()) {
            continue;
        }
        if name == reqwest::header::CONTENT_TYPE {
            has_content_type = true;
        }
        response = response.header(name, header_value);
    }

    if !has_content_type {
        response = response.header(reqwest::header::CONTENT_TYPE, "application/json");
    }

    match response.body(Body::from(payload)) {
        Ok(response) => response,
        Err(error) => build_proxy_error(
            StatusCode::BAD_GATEWAY,
            format!("Failed to build JSON proxy response: {error}"),
        ),
    }
}

fn reqwest_client() -> Result<reqwest::Client, String> {
    crate::shared::http_client::build_http_client_no_timeout(
        None,
        Some("CCHub Local Provider Proxy"),
    )
}

fn next_proxy_request_id() -> String {
    let uuid = Uuid::new_v4().simple().to_string();
    format!(
        "proxy-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        &uuid[..8]
    )
}

fn parse_json_bytes(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

fn transform_claude_request_body(
    api_format: ClaudeApiFormat,
    body_bytes: &[u8],
) -> Result<Bytes, String> {
    let parsed = parse_json_bytes(body_bytes)
        .ok_or_else(|| "Claude transformed proxy request must be valid JSON".to_string())?;
    let transformed = match api_format {
        ClaudeApiFormat::Anthropic => parsed,
        ClaudeApiFormat::OpenAiChat => anthropic_to_openai(parsed)?,
        ClaudeApiFormat::OpenAiResponses => anthropic_to_responses(parsed)?,
        ClaudeApiFormat::GeminiNative => {
            let (gemini_body, _model_id) = crate::gemini_transform::anthropic_to_gemini(parsed)?;
            gemini_body
        }
    };
    serde_json::to_vec(&transformed)
        .map(Bytes::from)
        .map_err(|error| format!("Failed to serialize transformed Claude request: {error}"))
}

fn extract_gemini_model_from_path(relative_path: &str) -> Option<String> {
    let (_, suffix) = relative_path.split_once("models/")?;
    let model = suffix
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches('/');
    if model.is_empty() {
        None
    } else {
        Some(model.to_string())
    }
}

fn extract_request_insights(
    tool_id: &str,
    relative_path: &str,
    body_bytes: &[u8],
) -> ProxyRequestInsights {
    let parsed = parse_json_bytes(body_bytes);
    let request_model = parsed
        .as_ref()
        .and_then(|value| value.get("model"))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if tool_id == "gemini" {
                extract_gemini_model_from_path(relative_path)
            } else {
                None
            }
        });

    let is_streaming = parsed
        .as_ref()
        .and_then(|value| value.get("stream"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || relative_path.contains("streamGenerateContent");

    ProxyRequestInsights {
        request_model,
        is_streaming,
    }
}

#[allow(clippy::never_loop)]
async fn forward_proxy_request(
    app_handle: AppHandle,
    tool_id: String,
    relative_path: String,
    request: Request<Body>,
) -> Response<Body> {
    let settings = {
        let db = app_handle.state::<DbState>();
        let conn = match db.0.lock() {
            Ok(conn) => conn,
            Err(error) => {
                return build_proxy_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Database lock failed: {error}"),
                );
            }
        };
        read_local_provider_proxy_settings_from_conn(&conn)
    };

    if !settings.enabled_apps.iter().any(|item| item == &tool_id) {
        return build_proxy_error(
            StatusCode::NOT_FOUND,
            format!("Local provider proxy is not enabled for {tool_id}"),
        );
    }

    let request_query = request.uri().query().map(str::to_string);
    let profile_candidates = {
        let db = app_handle.state::<DbState>();
        let conn = match db.0.lock() {
            Ok(conn) => conn,
            Err(error) => {
                return build_proxy_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Database lock failed: {error}"),
                );
            }
        };

        match ordered_profile_candidates(&app_handle, &conn, &tool_id) {
            Ok(value) => value,
            Err(error) => return build_proxy_error(StatusCode::BAD_GATEWAY, error),
        }
    };
    let profile_candidate_count = profile_candidates.len();

    let method = request.method().clone();
    let client = match reqwest_client() {
        Ok(client) => client,
        Err(error) => return build_proxy_error(StatusCode::BAD_GATEWAY, error),
    };

    let original_relative_path = relative_path;
    let original_headers: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> = request
        .headers()
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    let body_bytes = match to_bytes(request.into_body(), MAX_PROXY_BODY_BYTES).await {
        Ok(body) => body,
        Err(error) => {
            return build_proxy_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("Failed to read request body: {error}"),
            )
        }
    };

    let request_id = next_proxy_request_id();
    let started_at = Instant::now();
    let mut last_error: Option<String> = None;
    let rectifier_config = read_rectifier_config(&app_handle);
    let optimizer_config = read_optimizer_config(&app_handle);

    let mut total_profile_retries: u32 = 0;

    'profiles: for (profile_index, candidate) in profile_candidates.into_iter().enumerate() {
        if profile_index > 0 {
            if !optimizer_config.failover_enabled {
                break;
            }
            total_profile_retries += 1;
            if total_profile_retries > optimizer_config.max_profile_retries {
                break;
            }
        }

        let upstream = match extract_upstream_target(
            &app_handle,
            &tool_id,
            candidate.profile_id.clone(),
            candidate.profile_name.clone(),
            &candidate.snapshot,
        )
        .await
        {
            Ok(target) => target,
            Err(error) => {
                last_error = Some(error.clone());
                if profile_index + 1 < profile_candidate_count {
                    crate::utils::append_runtime_log(
                        "warn",
                        "provider_proxy",
                        &format!(
                            "Skipping unavailable provider [{tool_id}] {} ({}): {error}",
                            candidate.profile_name, candidate.profile_id
                        ),
                    );
                    continue;
                }
                return build_proxy_error(StatusCode::BAD_GATEWAY, error);
            }
        };

        let forwarded_headers: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> =
            original_headers
                // Header filtering depends on the selected upstream profile and transform mode.
                // Clone from the original request snapshot because the body has already been moved.
                .iter()
                .filter_map(|(name, value)| {
                    if is_hop_by_hop_header(name.as_str())
                        || should_strip_claude_transform_header(
                            name.as_str(),
                            upstream.claude_api_format,
                            &original_relative_path,
                        )
                    {
                        None
                    } else {
                        Some((name.clone(), value.clone()))
                    }
                })
                .collect();
        let has_accept_encoding_header = forwarded_headers
            .iter()
            .any(|(name, _)| name.as_str().eq_ignore_ascii_case("accept-encoding"));

        let (effective_relative_path, effective_request_query, effective_body_bytes) =
            match upstream.claude_api_format {
                Some(api_format)
                    if api_format.needs_transform()
                        && is_claude_messages_path(&original_relative_path) =>
                {
                    let (rewritten_path, rewritten_query) = rewrite_claude_request_target(
                        &original_relative_path,
                        request_query.as_deref(),
                        api_format,
                        upstream.is_github_copilot,
                        Some(body_bytes.as_ref()),
                    );
                    let transformed_body =
                        match transform_claude_request_body(api_format, body_bytes.as_ref()) {
                            Ok(body) => body,
                            Err(error) => return build_proxy_error(StatusCode::BAD_REQUEST, error),
                        };
                    (rewritten_path, rewritten_query, transformed_body)
                }
                _ => (
                    original_relative_path.clone(),
                    request_query.clone(),
                    body_bytes.clone(),
                ),
            };

        let optimizer_result = apply_proxy_optimizers(
            &tool_id,
            effective_body_bytes,
            &original_headers,
            &optimizer_config,
        );
        let effective_body_bytes = optimizer_result.body;
        let optimizer_extra_headers = optimizer_result.extra_headers;

        let request_insights = extract_request_insights(
            &tool_id,
            &effective_relative_path,
            effective_body_bytes.as_ref(),
        );
        let ordered_base_urls = ordered_upstream_base_urls(&app_handle, &upstream);
        let attempt_count = ordered_base_urls.len();

        for (index, base_url) in ordered_base_urls.iter().enumerate() {
            let mut request_body_bytes = effective_body_bytes.clone();
            let mut rectifier_attempts = 0usize;

            loop {
                let upstream_url = build_upstream_request_url(
                    base_url,
                    &effective_relative_path,
                    effective_request_query.as_deref(),
                    upstream.use_full_url,
                );
                let mut builder = client.request(method.clone(), upstream_url.clone());
                for (name, value) in &forwarded_headers {
                    builder = builder.header(name, value);
                }
                for (name, value) in &upstream.headers {
                    builder = builder.header(name, value);
                }
                for (name, value) in &optimizer_extra_headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                if !request_insights.is_streaming && !has_accept_encoding_header {
                    builder = builder.header(reqwest::header::ACCEPT_ENCODING, "gzip, deflate, br");
                }
                if !request_body_bytes.is_empty() {
                    builder = builder.body(request_body_bytes.clone());
                }

                match builder.send().await {
                    Ok(response) => {
                        let status = response.status();
                        let is_retryable_status = is_retryable_upstream_status(status);
                        if is_retryable_status {
                            record_endpoint_failure(
                                &app_handle,
                                &tool_id,
                                &upstream,
                                base_url,
                                &optimizer_config,
                            );
                            record_profile_failure(
                                &app_handle,
                                &tool_id,
                                &upstream.profile_id,
                                &upstream.profile_name,
                                &optimizer_config,
                            );
                            if index + 1 < attempt_count {
                                crate::utils::append_runtime_log(
                                    "warn",
                                    "provider_proxy",
                                    &format!(
                                        "Proxy failover retry [{tool_id}] {} (profile {}) {} returned {}. Trying next endpoint.",
                                        upstream.profile_name, upstream.profile_id, base_url, status
                                    ),
                                );
                                continue;
                            }
                            if profile_index + 1 < profile_candidate_count {
                                crate::utils::append_runtime_log(
                                    "warn",
                                    "provider_proxy",
                                    &format!(
                                        "Proxy failover switching provider [{tool_id}] {} ({}) after {} from {}.",
                                        upstream.profile_name, upstream.profile_id, status, base_url
                                    ),
                                );
                                last_error = Some(format!(
                                    "Upstream returned retryable status {} for {} ({})",
                                    status, upstream.profile_name, upstream.profile_id
                                ));
                                continue 'profiles;
                            }
                        }

                        let latency_ms =
                            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                        if status.is_success() {
                            record_endpoint_success(
                                &app_handle,
                                &upstream,
                                base_url,
                                &optimizer_config,
                            );
                            record_profile_success(
                                &app_handle,
                                &tool_id,
                                &upstream.profile_id,
                                &optimizer_config,
                            );
                            remember_preferred_upstream_base_url(
                                &app_handle,
                                &upstream.profile_id,
                                &upstream.base_url,
                                base_url,
                            );
                            if canonicalize_base_url(base_url)
                                != canonicalize_base_url(&upstream.base_url)
                            {
                                crate::utils::append_runtime_log(
                                    "info",
                                    "provider_proxy",
                                    &format!(
                                        "Proxy failover promoted alternate endpoint [{tool_id}] {} -> {}",
                                        upstream.base_url, base_url
                                    ),
                                );
                            }
                            if profile_index > 0 {
                                let _ = app_handle.emit(
                                    "provider-failover",
                                    serde_json::json!({
                                        "tool_id": &tool_id,
                                        "profile_name": &upstream.profile_name,
                                        "profile_id": &upstream.profile_id,
                                    }),
                                );
                            }
                        }

                        let headers = response.headers().clone();
                        let content_type = headers
                            .get(reqwest::header::CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                            .map(|value| value.to_ascii_lowercase())
                            .unwrap_or_default();
                        let is_json_response = content_type.contains("application/json")
                            || content_type.contains("+json");
                        let is_stream_response = request_insights.is_streaming
                            || content_type.contains("text/event-stream");
                        let claude_transform = upstream.claude_api_format.filter(|format| {
                            format.needs_transform()
                                && is_claude_messages_path(&original_relative_path)
                        });

                        if is_json_response && (!is_stream_response || !status.is_success()) {
                            match response.bytes().await {
                                Ok(bytes) => {
                                    let parsed = parse_json_bytes(&bytes);
                                    let upstream_error_message = parsed
                                        .as_ref()
                                        .and_then(extract_error_message_from_response);

                                    if status == StatusCode::BAD_REQUEST
                                        && rectifier_attempts < 2
                                        && matches!(
                                            upstream.claude_api_format,
                                            Some(ClaudeApiFormat::Anthropic)
                                        )
                                        && is_claude_messages_path(&original_relative_path)
                                    {
                                        match rectify_anthropic_request_bytes(
                                            request_body_bytes.as_ref(),
                                            upstream_error_message.as_deref(),
                                            &rectifier_config,
                                        ) {
                                            Ok(Some(rectified_body)) => {
                                                rectifier_attempts += 1;
                                                request_body_bytes = Bytes::from(rectified_body);
                                                crate::utils::append_runtime_log(
                                                    "info",
                                                    "provider_proxy",
                                                    &format!(
                                                        "Applied Claude request rectifier [{tool_id}] {} ({}) after upstream 400: {}",
                                                        upstream.profile_name,
                                                        upstream.profile_id,
                                                        upstream_error_message
                                                            .as_deref()
                                                            .unwrap_or("unknown error")
                                                    ),
                                                );
                                                continue;
                                            }
                                            Ok(None) => {}
                                            Err(error) => {
                                                crate::utils::append_runtime_log(
                                                    "warn",
                                                    "provider_proxy",
                                                    &format!(
                                                        "Failed to apply Claude request rectifier [{tool_id}] {} ({}): {error}",
                                                        upstream.profile_name, upstream.profile_id
                                                    ),
                                                );
                                            }
                                        }
                                    }

                                    let transformed_body = match (claude_transform, parsed) {
                                        (Some(api_format), Some(parsed)) => {
                                            match transform_claude_response_body(
                                                api_format,
                                                status,
                                                parsed,
                                                request_insights.request_model.as_deref(),
                                            ) {
                                                Ok(value) => Some(value),
                                                Err(error) => {
                                                    let message = format!(
                                                        "Failed to transform upstream response for {} ({}/{}): {error}",
                                                        upstream.profile_name, tool_id, upstream.profile_id
                                                    );
                                                    log_proxy_request(
                                                        &app_handle,
                                                        &request_id,
                                                        &tool_id,
                                                        &upstream,
                                                        &request_insights,
                                                        None,
                                                        latency_ms,
                                                        StatusCode::BAD_GATEWAY.as_u16(),
                                                        Some(&message),
                                                    );
                                                    return build_proxy_error(
                                                        StatusCode::BAD_GATEWAY,
                                                        message,
                                                    );
                                                }
                                            }
                                        }
                                        (Some(_), None) if status.is_success() => {
                                            let message = format!(
                                                "Upstream returned a non-JSON success body for transformed Claude request: {} ({}/{})",
                                                upstream.profile_name, tool_id, upstream.profile_id
                                            );
                                            log_proxy_request(
                                                &app_handle,
                                                &request_id,
                                                &tool_id,
                                                &upstream,
                                                &request_insights,
                                                None,
                                                latency_ms,
                                                StatusCode::BAD_GATEWAY.as_u16(),
                                                Some(&message),
                                            );
                                            return build_proxy_error(
                                                StatusCode::BAD_GATEWAY,
                                                message,
                                            );
                                        }
                                        (Some(_), None) => {
                                            Some(openai_error_to_anthropic(status.as_u16(), None))
                                        }
                                        (None, parsed) => parsed,
                                    };

                                    let usage = transformed_body
                                        .as_ref()
                                        .and_then(parse_usage_metrics_from_response);
                                    let error_message = if status.is_success() {
                                        None
                                    } else {
                                        transformed_body
                                            .as_ref()
                                            .and_then(extract_error_message_from_response)
                                    };
                                    log_proxy_request(
                                        &app_handle,
                                        &request_id,
                                        &tool_id,
                                        &upstream,
                                        &request_insights,
                                        usage.as_ref(),
                                        latency_ms,
                                        status.as_u16(),
                                        error_message.as_deref(),
                                    );
                                    if let Some(transformed_body) = transformed_body {
                                        return build_json_response_from_value(
                                            status,
                                            &headers,
                                            &transformed_body,
                                        );
                                    }
                                    return build_forward_response_from_parts(
                                        status,
                                        &headers,
                                        Body::from(bytes),
                                    );
                                }
                                Err(error) => {
                                    let message = format!(
                                        "Failed to read upstream response body for {} ({}/{}): {error}",
                                        upstream.profile_name, tool_id, upstream.profile_id
                                    );
                                    log_proxy_request(
                                        &app_handle,
                                        &request_id,
                                        &tool_id,
                                        &upstream,
                                        &request_insights,
                                        None,
                                        latency_ms,
                                        StatusCode::BAD_GATEWAY.as_u16(),
                                        Some(&message),
                                    );
                                    return build_proxy_error(StatusCode::BAD_GATEWAY, message);
                                }
                            }
                        }

                        if is_stream_response {
                            log_proxy_request(
                                &app_handle,
                                &request_id,
                                &tool_id,
                                &upstream,
                                &request_insights,
                                None,
                                latency_ms,
                                status.as_u16(),
                                None,
                            );
                            if let Some(api_format) = claude_transform {
                                let body = match api_format {
                                    ClaudeApiFormat::OpenAiChat => {
                                        Body::from_stream(create_usage_tracking_stream(
                                            create_anthropic_sse_stream(response.bytes_stream()),
                                            app_handle.clone(),
                                            request_id.clone(),
                                            tool_id.clone(),
                                            upstream.clone(),
                                            request_insights.clone(),
                                            optimizer_config.streaming_first_byte_timeout,
                                            optimizer_config.streaming_idle_timeout,
                                        ))
                                    }
                                    ClaudeApiFormat::OpenAiResponses => {
                                        Body::from_stream(create_usage_tracking_stream(
                                            create_anthropic_sse_stream_from_responses(
                                                response.bytes_stream(),
                                            ),
                                            app_handle.clone(),
                                            request_id.clone(),
                                            tool_id.clone(),
                                            upstream.clone(),
                                            request_insights.clone(),
                                            optimizer_config.streaming_first_byte_timeout,
                                            optimizer_config.streaming_idle_timeout,
                                        ))
                                    }
                                    ClaudeApiFormat::GeminiNative => {
                                        let gemini_model = request_insights
                                            .request_model
                                            .clone()
                                            .unwrap_or_else(|| "gemini-2.5-flash".to_string());
                                        Body::from_stream(create_usage_tracking_stream(
                                            create_anthropic_sse_stream_from_gemini(
                                                response.bytes_stream(),
                                                gemini_model,
                                            ),
                                            app_handle.clone(),
                                            request_id.clone(),
                                            tool_id.clone(),
                                            upstream.clone(),
                                            request_insights.clone(),
                                            optimizer_config.streaming_first_byte_timeout,
                                            optimizer_config.streaming_idle_timeout,
                                        ))
                                    }
                                    ClaudeApiFormat::Anthropic => {
                                        Body::from_stream(create_usage_tracking_stream(
                                            response.bytes_stream(),
                                            app_handle.clone(),
                                            request_id.clone(),
                                            tool_id.clone(),
                                            upstream.clone(),
                                            request_insights.clone(),
                                            optimizer_config.streaming_first_byte_timeout,
                                            optimizer_config.streaming_idle_timeout,
                                        ))
                                    }
                                };
                                return build_forward_response_from_parts(status, &headers, body);
                            }
                            if content_type.contains("text/event-stream") {
                                let body = Body::from_stream(create_usage_tracking_stream(
                                    response.bytes_stream(),
                                    app_handle.clone(),
                                    request_id.clone(),
                                    tool_id.clone(),
                                    upstream.clone(),
                                    request_insights.clone(),
                                    optimizer_config.streaming_first_byte_timeout,
                                    optimizer_config.streaming_idle_timeout,
                                ));
                                return build_forward_response_from_parts(status, &headers, body);
                            }
                            return build_forward_response(response);
                        }

                        let error_message = if status.is_success() {
                            None
                        } else {
                            Some(format!("Upstream returned HTTP {}", status.as_u16()))
                        };
                        log_proxy_request(
                            &app_handle,
                            &request_id,
                            &tool_id,
                            &upstream,
                            &request_insights,
                            None,
                            latency_ms,
                            status.as_u16(),
                            error_message.as_deref(),
                        );
                        return build_forward_response(response);
                    }
                    Err(error) => {
                        let message = format!(
                            "Upstream request failed for {} ({}/{} @ {}): {error}",
                            upstream.profile_name, tool_id, upstream.profile_id, base_url
                        );
                        last_error = Some(message.clone());
                        record_endpoint_failure(
                            &app_handle,
                            &tool_id,
                            &upstream,
                            base_url,
                            &optimizer_config,
                        );
                        if index + 1 < attempt_count {
                            crate::utils::append_runtime_log(
                                "warn",
                                "provider_proxy",
                                &format!(
                                    "Proxy request failed [{tool_id}] {} @ {}: {error}. Trying next endpoint.",
                                    upstream.profile_name, base_url
                                ),
                            );
                            continue;
                        }

                        record_profile_failure(
                            &app_handle,
                            &tool_id,
                            &upstream.profile_id,
                            &upstream.profile_name,
                            &optimizer_config,
                        );
                        if profile_index + 1 < profile_candidate_count {
                            crate::utils::append_runtime_log(
                                "warn",
                                "provider_proxy",
                                &format!(
                                    "Proxy request failed [{tool_id}] {} ({} @ {}). Trying next provider.",
                                    upstream.profile_name, upstream.profile_id, base_url
                                ),
                            );
                            continue 'profiles;
                        }

                        crate::utils::append_runtime_log("warn", "provider_proxy", &message);
                        let latency_ms =
                            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                        log_proxy_request(
                            &app_handle,
                            &request_id,
                            &tool_id,
                            &upstream,
                            &request_insights,
                            None,
                            latency_ms,
                            StatusCode::BAD_GATEWAY.as_u16(),
                            Some(&message),
                        );
                        return build_proxy_error(StatusCode::BAD_GATEWAY, message);
                    }
                }
            }
        }
    }

    build_proxy_error(
        StatusCode::BAD_GATEWAY,
        last_error.unwrap_or_else(|| format!("No upstream provider available for {tool_id}")),
    )
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
