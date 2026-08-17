// proxy_optimizer 配置读取与 body 改写：Codex/Claude 各有不同的预处理。
use bytes::Bytes;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::db::DbState;

use super::LocalProviderProxyRuntime;

pub(super) fn read_optimizer_config(
    app_handle: &AppHandle,
) -> crate::proxy_optimizer::OptimizerConfig {
    if let Some(config) = app_handle
        .try_state::<LocalProviderProxyRuntime>()
        .and_then(|runtime_state| {
            runtime_state
                .0
                .lock()
                .ok()
                .and_then(|runtime| runtime.optimizer_config.clone())
        })
    {
        return apply_auto_failover_override(app_handle, config);
    }

    let db = app_handle.state::<DbState>();
    let conn = match db.0.lock() {
        Ok(conn) => conn,
        Err(_) => return crate::proxy_optimizer::OptimizerConfig::default(),
    };

    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![crate::proxy_optimizer::config::OPTIMIZER_CONFIG_SETTINGS_KEY],
            |row| row.get(0),
        )
        .ok();

    let config: crate::proxy_optimizer::OptimizerConfig = raw
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    update_optimizer_config_cache(app_handle, config.clone());
    apply_auto_failover_override_with_conn(&conn, config)
}

fn apply_auto_failover_override(
    app_handle: &AppHandle,
    config: crate::proxy_optimizer::OptimizerConfig,
) -> crate::proxy_optimizer::OptimizerConfig {
    let db = app_handle.state::<DbState>();
    let Ok(conn) = db.0.lock() else {
        return config;
    };
    apply_auto_failover_override_with_conn(&conn, config)
}

fn apply_auto_failover_override_with_conn(
    conn: &rusqlite::Connection,
    mut config: crate::proxy_optimizer::OptimizerConfig,
) -> crate::proxy_optimizer::OptimizerConfig {
    let enabled = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'proxy_auto_failover_enabled'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| serde_json::from_str::<bool>(&value).ok())
        .unwrap_or(true);
    config.failover_enabled &= enabled;
    config
}

pub(super) fn read_rectifier_config(
    app_handle: &AppHandle,
) -> crate::proxy_optimizer::config::RectifierConfig {
    if let Some(config) = app_handle
        .try_state::<LocalProviderProxyRuntime>()
        .and_then(|runtime_state| {
            runtime_state
                .0
                .lock()
                .ok()
                .and_then(|runtime| runtime.rectifier_config.clone())
        })
    {
        return config;
    }

    let db = app_handle.state::<DbState>();
    let conn = match db.0.lock() {
        Ok(conn) => conn,
        Err(_) => return crate::proxy_optimizer::config::RectifierConfig::default(),
    };

    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![crate::proxy_optimizer::config::RECTIFIER_CONFIG_SETTINGS_KEY],
            |row| row.get(0),
        )
        .ok();

    let config: crate::proxy_optimizer::config::RectifierConfig = raw
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();

    update_rectifier_config_cache(app_handle, config.clone());
    config
}

pub(crate) fn update_optimizer_config_cache(
    app_handle: &AppHandle,
    config: crate::proxy_optimizer::OptimizerConfig,
) {
    if let Some(runtime_state) = app_handle.try_state::<LocalProviderProxyRuntime>() {
        if let Ok(mut runtime) = runtime_state.0.lock() {
            runtime.optimizer_config = Some(config);
        }
    }
}

pub(crate) fn update_rectifier_config_cache(
    app_handle: &AppHandle,
    config: crate::proxy_optimizer::config::RectifierConfig,
) {
    if let Some(runtime_state) = app_handle.try_state::<LocalProviderProxyRuntime>() {
        if let Ok(mut runtime) = runtime_state.0.lock() {
            runtime.rectifier_config = Some(config);
        }
    }
}

pub(super) struct OptimizerResult {
    pub(super) body: Bytes,
    pub(super) extra_headers: Vec<(String, String)>,
}

pub(super) fn apply_proxy_optimizers(
    tool_id: &str,
    is_codex_oauth: bool,
    body_bytes: Bytes,
    original_headers: &[(axum::http::HeaderName, axum::http::HeaderValue)],
    config: &crate::proxy_optimizer::OptimizerConfig,
) -> OptimizerResult {
    if tool_id == "claude" && is_codex_oauth {
        let result_body = match serde_json::from_slice::<Value>(&body_bytes) {
            Ok(mut body) => {
                crate::provider_proxy_transform::strip_codex_oauth_fields(&mut body);
                match serde_json::to_vec(&body) {
                    Ok(value) => Bytes::from(value),
                    Err(_) => body_bytes,
                }
            }
            Err(_) => body_bytes,
        };
        return OptimizerResult {
            body: result_body,
            extra_headers: Vec::new(),
        };
    }

    if tool_id == "codex" && config.codex_field_stripping {
        let result_body = match serde_json::from_slice::<Value>(&body_bytes) {
            Ok(mut body) => {
                crate::provider_proxy_transform::strip_codex_oauth_fields(&mut body);
                match serde_json::to_vec(&body) {
                    Ok(v) => Bytes::from(v),
                    Err(_) => body_bytes,
                }
            }
            Err(_) => body_bytes,
        };
        return OptimizerResult {
            body: result_body,
            extra_headers: Vec::new(),
        };
    }

    if tool_id != "claude" {
        return OptimizerResult {
            body: body_bytes,
            extra_headers: Vec::new(),
        };
    }

    if !config.enabled {
        return OptimizerResult {
            body: body_bytes,
            extra_headers: Vec::new(),
        };
    }

    let mut body: Value = match serde_json::from_slice(&body_bytes) {
        Ok(value) => value,
        Err(_) => {
            return OptimizerResult {
                body: body_bytes,
                extra_headers: Vec::new(),
            }
        }
    };

    let mut extra_headers: Vec<(String, String)> = Vec::new();

    // 0. Copilot optimizer (classify before body modifications, inject headers)
    if config.copilot_optimizer {
        if config.copilot_model_normalization {
            crate::proxy_optimizer::copilot_optimizer::apply_copilot_model_normalization(&mut body);
        }

        let has_anthropic_beta = original_headers
            .iter()
            .any(|(name, _)| name.as_str().eq_ignore_ascii_case("anthropic-beta"));

        let classification = crate::proxy_optimizer::copilot_optimizer::classify_request(
            &body,
            has_anthropic_beta,
            config.copilot_compact_detection,
            config.copilot_subagent_detection,
        );

        extra_headers.push((
            "x-initiator".to_string(),
            classification.initiator.to_string(),
        ));

        if classification.is_subagent {
            extra_headers.push((
                "x-interaction-type".to_string(),
                "conversation-subagent".to_string(),
            ));
        }

        let session_id = original_headers
            .iter()
            .find(|(name, _)| name.as_str().eq_ignore_ascii_case("x-session-id"))
            .and_then(|(_, v)| v.to_str().ok())
            .unwrap_or("");

        let request_id =
            crate::proxy_optimizer::copilot_optimizer::deterministic_request_id(&body, session_id);
        extra_headers.push(("x-request-id".to_string(), request_id));

        if let Some(interaction_id) =
            crate::proxy_optimizer::copilot_optimizer::deterministic_interaction_id(session_id)
        {
            extra_headers.push(("x-interaction-id".to_string(), interaction_id));
        }

        if config.copilot_sanitize_orphans {
            body = crate::proxy_optimizer::copilot_optimizer::sanitize_orphan_tool_results(body);
        }
        if config.copilot_merge_tool_results {
            body = crate::proxy_optimizer::copilot_optimizer::merge_tool_results(body);
        }
        if config.copilot_strip_thinking {
            body = crate::proxy_optimizer::copilot_optimizer::strip_thinking_blocks(body);
        }
    }

    // 1. Body filter
    body = crate::proxy_optimizer::body_filter::filter(body, config);

    // 2. Model mapper
    if config.model_mapper {
        let mapping = crate::proxy_optimizer::model_mapper::ModelMapping {
            default_model: if config.model_mapper_default.is_empty() {
                None
            } else {
                Some(config.model_mapper_default.clone())
            },
            custom_rules: config.model_mapper_rules.clone(),
            ..Default::default()
        };
        let (mapped_body, _, _) =
            crate::proxy_optimizer::model_mapper::apply_model_mapping(body, &mapping);
        body = mapped_body;
    }

    // 3. Thinking optimizer
    crate::proxy_optimizer::thinking_optimizer::optimize(&mut body, config);

    // 4. Cache injector
    crate::proxy_optimizer::cache_injector::inject(&mut body, config);

    let result_body = match serde_json::to_vec(&body) {
        Ok(bytes) => Bytes::from(bytes),
        Err(_) => body_bytes,
    };

    OptimizerResult {
        body: result_body,
        extra_headers,
    }
}
