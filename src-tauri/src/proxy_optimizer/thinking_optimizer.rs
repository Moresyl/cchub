use super::config::OptimizerConfig;
use serde_json::{json, Value};

pub fn optimize(body: &mut Value, config: &OptimizerConfig) {
    if !config.enabled || !config.thinking_optimizer {
        return;
    }

    let model = match body.get("model").and_then(|m| m.as_str()) {
        Some(m) => m.to_lowercase(),
        None => return,
    };

    if model.contains("haiku") {
        return;
    }

    if model.contains("opus-4-7") || model.contains("opus-4-6") || model.contains("sonnet-4-6") {
        body["thinking"] = json!({"type": "adaptive"});
        body["output_config"] = json!({"effort": "max"});
        append_beta(body, "context-1m-2025-08-07");
        return;
    }

    let max_tokens = body
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(16384);

    let budget_target = max_tokens.saturating_sub(1);

    let thinking_type = body
        .get("thinking")
        .and_then(|t| t.get("type"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    match thinking_type.as_deref() {
        None | Some("disabled") => {
            body["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": budget_target
            });
            append_beta(body, "interleaved-thinking-2025-05-14");
        }
        Some("enabled") => {
            let current_budget = body
                .get("thinking")
                .and_then(|t| t.get("budget_tokens"))
                .and_then(|b| b.as_u64())
                .unwrap_or(0);
            if current_budget < budget_target {
                body["thinking"]["budget_tokens"] = json!(budget_target);
            }
            append_beta(body, "interleaved-thinking-2025-05-14");
        }
        _ => {
            append_beta(body, "interleaved-thinking-2025-05-14");
        }
    }
}

fn append_beta(body: &mut Value, beta: &str) {
    match body.get("anthropic_beta") {
        Some(Value::Array(arr)) => {
            if arr.iter().any(|v| v.as_str() == Some(beta)) {
                return;
            }
            if let Some(arr) = body["anthropic_beta"].as_array_mut() {
                arr.push(json!(beta));
            }
        }
        Some(Value::Null) | None => {
            body["anthropic_beta"] = json!([beta]);
        }
        _ => {
            body["anthropic_beta"] = json!([beta]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn enabled_config() -> OptimizerConfig {
        OptimizerConfig {
            enabled: true,
            thinking_optimizer: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_haiku_skipped() {
        let mut body = json!({
            "model": "claude-haiku-4-5",
            "max_tokens": 4096,
            "messages": [{"role": "user", "content": "hello"}]
        });
        let original = body.clone();
        optimize(&mut body, &enabled_config());
        assert_eq!(body, original);
    }

    #[test]
    fn test_adaptive_opus_4_7() {
        let mut body = json!({
            "model": "claude-opus-4-7-20250514",
            "max_tokens": 16384,
            "messages": [{"role": "user", "content": "hello"}]
        });
        optimize(&mut body, &enabled_config());
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "max");
    }

    #[test]
    fn test_adaptive_sonnet_4_6() {
        let mut body = json!({
            "model": "claude-sonnet-4-6-20250514",
            "max_tokens": 16384,
            "messages": [{"role": "user", "content": "hello"}]
        });
        optimize(&mut body, &enabled_config());
        assert_eq!(body["thinking"]["type"], "adaptive");
    }

    #[test]
    fn test_legacy_sonnet_3_5() {
        let mut body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 8192,
            "messages": [{"role": "user", "content": "hello"}]
        });
        optimize(&mut body, &enabled_config());
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["budget_tokens"], 8191);
    }

    #[test]
    fn test_disabled_no_change() {
        let config = OptimizerConfig {
            enabled: true,
            thinking_optimizer: false,
            ..enabled_config()
        };
        let mut body = json!({
            "model": "claude-opus-4-7-20250514",
            "max_tokens": 16384,
            "messages": [{"role": "user", "content": "hello"}]
        });
        let original = body.clone();
        optimize(&mut body, &config);
        assert_eq!(body, original);
    }
}
