// 解析 active profile snapshot、构造候选 upstream base URL 列表、熔断记账。
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashSet;
use tauri::{AppHandle, Manager};

use super::{
    current_profile_setting_key, CircuitState, ClaudeApiFormat, LocalProviderProxyRuntime,
    ProfileCandidate, UpstreamTarget,
};

pub(super) fn active_profile_id_for_tool(
    conn: &Connection,
    tool_id: &str,
) -> Result<String, String> {
    let setting_key = current_profile_setting_key(tool_id);
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        rusqlite::params![setting_key],
        |row| row.get(0),
    )
    .map_err(|_| format!("No active provider profile selected for {tool_id}"))
}

pub(super) fn read_profile_candidates_for_tool(
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

    apply_failover_queue(conn, tool_id, &mut profiles);
    Ok(profiles)
}

fn apply_failover_queue(conn: &Connection, tool_id: &str, profiles: &mut Vec<ProfileCandidate>) {
    if profiles.len() < 2 {
        return;
    }
    let queue_key = format!("proxy_failover_queue:{tool_id}");
    let queue = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![queue_key],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
        .unwrap_or_default();
    if queue.is_empty() {
        return;
    }
    let primary = profiles.remove(0);
    let mut ordered = vec![primary];
    for profile_id in queue {
        if let Some(index) = profiles
            .iter()
            .position(|profile| profile.profile_id == profile_id)
        {
            ordered.push(profiles.remove(index));
        }
    }
    ordered.append(profiles);
    *profiles = ordered;
}

pub(super) fn default_base_url_for_claude(api_format: &str) -> Option<String> {
    if api_format == "anthropic" {
        Some("https://api.anthropic.com".to_string())
    } else {
        None
    }
}

pub(super) fn default_base_url_for_gemini() -> String {
    "https://generativelanguage.googleapis.com/v1beta".to_string()
}

pub(super) fn default_base_url_for_codex() -> String {
    "https://api.openai.com/v1".to_string()
}

pub(super) fn is_claude_messages_path(relative_path: &str) -> bool {
    matches!(
        relative_path.trim().trim_matches('/'),
        "v1/messages" | "claude/v1/messages"
    )
}

pub(super) fn strip_beta_query(query: Option<&str>) -> Option<String> {
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

pub(super) fn rewrite_claude_request_target(
    relative_path: &str,
    query: Option<&str>,
    api_format: ClaudeApiFormat,
    is_github_copilot: bool,
    is_codex_oauth: bool,
    body_bytes: Option<&[u8]>,
) -> (String, Option<String>) {
    if !api_format.needs_transform() || !is_claude_messages_path(relative_path) {
        return (relative_path.to_string(), query.map(str::to_string));
    }

    let target_path = match api_format {
        ClaudeApiFormat::OpenAiChat if is_github_copilot => "chat/completions".to_string(),
        ClaudeApiFormat::OpenAiChat => "v1/chat/completions".to_string(),
        ClaudeApiFormat::OpenAiResponses if is_codex_oauth => "responses".to_string(),
        ClaudeApiFormat::OpenAiResponses => "v1/responses".to_string(),
        ClaudeApiFormat::GeminiNative => {
            let model = body_bytes
                .and_then(|b| serde_json::from_slice::<Value>(b).ok())
                .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(String::from))
                .unwrap_or_else(|| "gemini-3.6-flash".to_string());
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

pub(super) fn should_strip_claude_transform_header(
    name: &str,
    api_format: Option<ClaudeApiFormat>,
    relative_path: &str,
) -> bool {
    matches!(api_format, Some(format) if format.needs_transform() && is_claude_messages_path(relative_path))
        && name.to_ascii_lowercase().starts_with("anthropic-")
}

pub(super) fn canonicalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

pub(super) fn extract_metadata_endpoint_candidates(parsed: &Value) -> Vec<String> {
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

pub(super) fn extract_metadata_object(parsed: &Value) -> serde_json::Map<String, Value> {
    parsed
        .get("metadata")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default()
}

pub(super) fn extract_provider_type(parsed: &Value) -> Option<String> {
    extract_metadata_object(parsed)
        .get("providerType")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn extract_use_full_url(parsed: &Value) -> bool {
    extract_metadata_object(parsed)
        .get("useFullUrl")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

pub(super) fn extract_cost_multiplier(parsed: &Value) -> f64 {
    let metadata = extract_metadata_object(parsed);
    let value = metadata.get("costMultiplier");
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(1.0),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or(1.0),
        _ => 1.0,
    }
    .clamp(0.0, 1_000_000.0)
}

pub(super) fn extract_copilot_account_id(parsed: &Value) -> Option<String> {
    extract_bound_account_id(parsed, "github_copilot").or_else(|| {
        extract_metadata_object(parsed)
            .get("githubAccountId")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn extract_bound_account_id(parsed: &Value, provider_name: &str) -> Option<String> {
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
            if provider == provider_name {
                value
                    .get("accountId")
                    .and_then(|item| item.as_str())
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
            } else {
                None
            }
        })
}

pub(super) fn filter_endpoint_candidates(
    primary_base_url: &str,
    candidates: Vec<String>,
) -> Vec<String> {
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

pub(super) fn profile_circuit_key(tool_id: &str, profile_id: &str) -> String {
    format!("{tool_id}::{profile_id}")
}

pub(super) fn ordered_profile_candidates(
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

pub(super) fn ordered_upstream_base_urls(
    app_handle: &AppHandle,
    upstream: &UpstreamTarget,
) -> Vec<String> {
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

pub(super) fn endpoint_circuit_key(profile_id: &str, base_url: &str) -> String {
    format!("{profile_id}::{}", canonicalize_base_url(base_url))
}

pub(super) fn remember_preferred_upstream_base_url(
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

pub(super) fn record_profile_success(
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

pub(super) fn record_profile_failure(
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

pub(super) fn record_endpoint_success(
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

pub(super) fn record_endpoint_failure(
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
