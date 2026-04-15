use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct HermesProviderPreset {
    pub provider: &'static str,
    pub env_key: Option<&'static str>,
}

pub const PRESETS: &[HermesProviderPreset] = &[
    HermesProviderPreset {
        provider: "nous",
        env_key: None,
    },
    HermesProviderPreset {
        provider: "openrouter",
        env_key: Some("OPENROUTER_API_KEY"),
    },
    HermesProviderPreset {
        provider: "openrouter",
        env_key: Some("OPENROUTER_API_KEY"),
    },
    HermesProviderPreset {
        provider: "gemini",
        env_key: Some("GEMINI_API_KEY"),
    },
    HermesProviderPreset {
        provider: "zai",
        env_key: Some("GLM_API_KEY"),
    },
    HermesProviderPreset {
        provider: "kimi-coding",
        env_key: Some("KIMI_API_KEY"),
    },
    HermesProviderPreset {
        provider: "custom",
        env_key: None,
    },
];

const KNOWN_API_KEY_ENV_KEYS: &[&str] = &[
    "OPENROUTER_API_KEY",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GLM_API_KEY",
    "KIMI_API_KEY",
    "KIMI_CN_API_KEY",
    "MINIMAX_API_KEY",
    "XIAOMI_API_KEY",
    "HF_TOKEN",
    "VOICE_TOOLS_OPENAI_KEY",
];

pub fn preset_for_provider(provider: &str) -> Option<HermesProviderPreset> {
    PRESETS.iter().copied().find(|preset| preset.provider == provider)
}

pub fn default_env_key_for_provider(provider: &str) -> Option<&'static str> {
    preset_for_provider(provider).and_then(|preset| preset.env_key)
}

/// 返回已知 Hermes provider 的默认 base URL,用于端点探活等场景的 fallback。
/// 未收录的 provider(含 `custom`)返回 None,调用方应要求用户显式配置。
pub fn default_base_url_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "nous" | "nous-api" => Some("https://portal.nousresearch.com/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "anthropic" => Some("https://api.anthropic.com"),
        "openai-codex" => Some("https://api.openai.com/v1"),
        "gemini" => Some("https://generativelanguage.googleapis.com/v1beta"),
        "zai" => Some("https://api.z.ai/v1"),
        "kimi-coding" => Some("https://api.moonshot.ai/v1"),
        "minimax" => Some("https://api.minimax.chat/v1"),
        "minimax-cn" => Some("https://api.minimaxi.chat/v1"),
        "huggingface" => Some("https://api-inference.huggingface.co"),
        _ => None,
    }
}

pub fn known_api_key_env_keys() -> &'static [&'static str] {
    KNOWN_API_KEY_ENV_KEYS
}

pub fn infer_api_key_env_key(
    provider: Option<&str>,
    env_map: &HashMap<String, String>,
) -> Option<String> {
    if let Some(provider_key) = provider.and_then(default_env_key_for_provider) {
        if env_map
            .get(provider_key)
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Some(provider_key.to_string());
        }
    }

    KNOWN_API_KEY_ENV_KEYS.iter().find_map(|key| {
        env_map
            .get(*key)
            .filter(|value| !value.trim().is_empty())
            .map(|_| (*key).to_string())
    })
}
