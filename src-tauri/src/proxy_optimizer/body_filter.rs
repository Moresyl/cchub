use super::config::OptimizerConfig;
use serde_json::Value;
use std::collections::HashSet;

pub fn filter(body: Value, config: &OptimizerConfig) -> Value {
    if !config.enabled || !config.body_filter {
        return body;
    }
    filter_private_params_with_whitelist(body, &config.body_filter_whitelist)
}

fn filter_private_params_with_whitelist(body: Value, whitelist: &[String]) -> Value {
    let whitelist_set: HashSet<&str> = whitelist.iter().map(|s| s.as_str()).collect();
    filter_recursive(body, &whitelist_set)
}

fn filter_recursive(value: Value, whitelist: &HashSet<&str>) -> Value {
    match value {
        Value::Object(map) => {
            let filtered: serde_json::Map<String, Value> = map
                .into_iter()
                .filter_map(|(key, val)| {
                    if key.starts_with('_') && !whitelist.contains(key.as_str()) {

                        None
                    } else {
                        Some((key, filter_recursive(val, whitelist)))
                    }
                })
                .collect();
            Value::Object(filtered)
        }
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(|v| filter_recursive(v, whitelist)).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn enabled_config() -> OptimizerConfig {
        OptimizerConfig {
            enabled: true,
            body_filter: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_filter_top_level() {
        let input = json!({
            "model": "claude-3",
            "_internal_id": "abc123",
            "_debug": true,
            "max_tokens": 1024
        });
        let output = filter(input, &enabled_config());
        assert!(output.get("model").is_some());
        assert!(output.get("max_tokens").is_some());
        assert!(output.get("_internal_id").is_none());
        assert!(output.get("_debug").is_none());
    }

    #[test]
    fn test_filter_nested() {
        let input = json!({
            "model": "claude-3",
            "messages": [{"role": "user", "content": "hi", "_token": "secret"}],
            "metadata": {"user_id": "u1", "_tracking": "t1"}
        });
        let output = filter(input, &enabled_config());
        let messages = output["messages"].as_array().unwrap();
        assert!(messages[0].get("_token").is_none());
        assert!(output["metadata"].get("_tracking").is_none());
        assert!(output["metadata"].get("user_id").is_some());
    }

    #[test]
    fn test_whitelist() {
        let config = OptimizerConfig {
            body_filter_whitelist: vec!["_metadata".to_string()],
            ..enabled_config()
        };
        let input = json!({
            "model": "claude-3",
            "_metadata": {"key": "value"},
            "_internal_id": "abc"
        });
        let output = filter(input, &config);
        assert!(output.get("_metadata").is_some());
        assert!(output.get("_internal_id").is_none());
    }

    #[test]
    fn test_disabled() {
        let config = OptimizerConfig {
            body_filter: false,
            ..enabled_config()
        };
        let input = json!({"_secret": "keep", "model": "test"});
        let output = filter(input.clone(), &config);
        assert_eq!(input, output);
    }
}
