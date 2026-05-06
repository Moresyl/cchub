use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelMappingRule {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub match_mode: MatchMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    Contains,
    Exact,
}

#[derive(Debug, Clone, Default)]
pub struct ModelMapping {
    pub haiku_model: Option<String>,
    pub sonnet_model: Option<String>,
    pub opus_model: Option<String>,
    pub default_model: Option<String>,
    pub custom_rules: Vec<ModelMappingRule>,
}

impl ModelMapping {
    #[allow(dead_code)]
    pub fn from_env(env: Option<&Value>) -> Self {
        let get_str = |key: &str| -> Option<String> {
            env.and_then(|e| e.get(key))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        };

        Self {
            haiku_model: get_str("ANTHROPIC_DEFAULT_HAIKU_MODEL"),
            sonnet_model: get_str("ANTHROPIC_DEFAULT_SONNET_MODEL"),
            opus_model: get_str("ANTHROPIC_DEFAULT_OPUS_MODEL"),
            default_model: get_str("ANTHROPIC_MODEL"),
            custom_rules: Vec::new(),
        }
    }

    pub fn has_mapping(&self) -> bool {
        self.haiku_model.is_some()
            || self.sonnet_model.is_some()
            || self.opus_model.is_some()
            || self.default_model.is_some()
            || !self.custom_rules.is_empty()
    }

    pub fn map_model(&self, original: &str) -> String {
        // 1. Custom rules (highest priority)
        for rule in &self.custom_rules {
            let matched = match rule.match_mode {
                MatchMode::Exact => original == rule.from,
                MatchMode::Contains => original.to_lowercase().contains(&rule.from.to_lowercase()),
            };
            if matched {
                return rule.to.clone();
            }
        }

        let lower = original.to_lowercase();

        // 2. Type-based mapping
        if lower.contains("haiku") {
            if let Some(ref m) = self.haiku_model {
                return m.clone();
            }
        }
        if lower.contains("opus") {
            if let Some(ref m) = self.opus_model {
                return m.clone();
            }
        }
        if lower.contains("sonnet") {
            if let Some(ref m) = self.sonnet_model {
                return m.clone();
            }
        }

        // 3. Default model
        if let Some(ref m) = self.default_model {
            return m.clone();
        }

        original.to_string()
    }
}

pub fn apply_model_mapping(
    mut body: Value,
    mapping: &ModelMapping,
) -> (Value, Option<String>, Option<String>) {
    if !mapping.has_mapping() {
        let original = body.get("model").and_then(|m| m.as_str()).map(String::from);
        return (body, original, None);
    }

    let original_model = body.get("model").and_then(|m| m.as_str()).map(String::from);

    if let Some(ref original) = original_model {
        let mapped = mapping.map_model(original);
        if mapped != *original {
            body["model"] = serde_json::json!(mapped);
            return (body, Some(original.clone()), Some(mapped));
        }
    }

    (body, original_model, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_type_based_mapping() {
        let mapping = ModelMapping {
            haiku_model: Some("custom-haiku".to_string()),
            sonnet_model: Some("custom-sonnet".to_string()),
            opus_model: Some("custom-opus".to_string()),
            default_model: None,
            custom_rules: Vec::new(),
        };

        assert_eq!(mapping.map_model("claude-haiku-4-5"), "custom-haiku");
        assert_eq!(mapping.map_model("claude-sonnet-4-6"), "custom-sonnet");
        assert_eq!(mapping.map_model("claude-opus-4-7"), "custom-opus");
        assert_eq!(mapping.map_model("unknown-model"), "unknown-model");
    }

    #[test]
    fn test_custom_rules_priority() {
        let mapping = ModelMapping {
            sonnet_model: Some("fallback-sonnet".to_string()),
            custom_rules: vec![ModelMappingRule {
                from: "claude-sonnet-4-6".to_string(),
                to: "my-special-model".to_string(),
                match_mode: MatchMode::Exact,
            }],
            ..Default::default()
        };

        assert_eq!(mapping.map_model("claude-sonnet-4-6"), "my-special-model");
        assert_eq!(mapping.map_model("claude-sonnet-4-5"), "fallback-sonnet");
    }

    #[test]
    fn test_apply_to_body() {
        let body = json!({"model": "claude-haiku-4-5", "messages": []});
        let mapping = ModelMapping {
            haiku_model: Some("fast-model".to_string()),
            ..Default::default()
        };

        let (result, original, mapped) = apply_model_mapping(body, &mapping);
        assert_eq!(result["model"], "fast-model");
        assert_eq!(original.unwrap(), "claude-haiku-4-5");
        assert_eq!(mapped.unwrap(), "fast-model");
    }

    #[test]
    fn test_no_mapping_passthrough() {
        let body = json!({"model": "claude-sonnet-4-6", "messages": []});
        let mapping = ModelMapping::default();

        let (result, original, mapped) = apply_model_mapping(body.clone(), &mapping);
        assert_eq!(result, body);
        assert_eq!(original.unwrap(), "claude-sonnet-4-6");
        assert!(mapped.is_none());
    }
}
