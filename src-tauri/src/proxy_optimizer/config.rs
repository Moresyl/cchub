use serde::{Deserialize, Serialize};

use super::model_mapper::ModelMappingRule;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizerConfig {
    pub enabled: bool,
    pub thinking_optimizer: bool,
    pub cache_injection: bool,
    pub cache_ttl: String,
    pub body_filter: bool,
    #[serde(default)]
    pub body_filter_whitelist: Vec<String>,
    #[serde(default)]
    pub model_mapper: bool,
    #[serde(default)]
    pub model_mapper_default: String,
    #[serde(default)]
    pub model_mapper_rules: Vec<ModelMappingRule>,
    #[serde(default)]
    pub copilot_optimizer: bool,
    #[serde(default)]
    pub copilot_merge_tool_results: bool,
    #[serde(default)]
    pub copilot_sanitize_orphans: bool,
    #[serde(default)]
    pub copilot_strip_thinking: bool,
    #[serde(default)]
    pub copilot_compact_detection: bool,
    #[serde(default)]
    pub copilot_subagent_detection: bool,
    #[serde(default = "default_true")]
    pub copilot_model_normalization: bool,
    #[serde(default)]
    pub codex_field_stripping: bool,
    #[serde(default = "default_circuit_failure_threshold")]
    pub circuit_failure_threshold: u32,
    #[serde(default = "default_circuit_success_threshold")]
    pub circuit_success_threshold: u32,
    #[serde(default = "default_circuit_timeout_secs")]
    pub circuit_timeout_secs: u64,
    #[serde(default = "default_true")]
    pub failover_enabled: bool,
    #[serde(default = "default_max_profile_retries")]
    pub max_profile_retries: u32,
    #[serde(default = "default_streaming_first_byte_timeout")]
    pub streaming_first_byte_timeout: u64,
    #[serde(default = "default_streaming_idle_timeout")]
    pub streaming_idle_timeout: u64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            thinking_optimizer: false,
            cache_injection: false,
            cache_ttl: "5m".to_string(),
            body_filter: false,
            body_filter_whitelist: Vec::new(),
            model_mapper: false,
            model_mapper_default: String::new(),
            model_mapper_rules: Vec::new(),
            copilot_optimizer: false,
            copilot_merge_tool_results: true,
            copilot_sanitize_orphans: true,
            copilot_strip_thinking: true,
            copilot_compact_detection: true,
            copilot_subagent_detection: true,
            copilot_model_normalization: true,
            codex_field_stripping: false,
            circuit_failure_threshold: 3,
            circuit_success_threshold: 2,
            circuit_timeout_secs: 60,
            failover_enabled: true,
            max_profile_retries: 3,
            streaming_first_byte_timeout: 60,
            streaming_idle_timeout: 120,
        }
    }
}

fn default_circuit_failure_threshold() -> u32 { 3 }
fn default_circuit_success_threshold() -> u32 { 2 }
fn default_circuit_timeout_secs() -> u64 { 60 }
fn default_max_profile_retries() -> u32 { 3 }
fn default_streaming_first_byte_timeout() -> u64 { 60 }
fn default_streaming_idle_timeout() -> u64 { 120 }

pub const OPTIMIZER_CONFIG_SETTINGS_KEY: &str = "proxy_optimizer_config";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectifierConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub thinking_signature: bool,
    #[serde(default = "default_true")]
    pub thinking_budget: bool,
}

impl Default for RectifierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thinking_signature: true,
            thinking_budget: true,
        }
    }
}

fn default_true() -> bool {
    true
}

pub const RECTIFIER_CONFIG_SETTINGS_KEY: &str = "rectifier_config";
