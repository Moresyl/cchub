use super::{ModelMatch, RoutingRule};
use serde_json::Value;

pub(super) struct Facts {
    model: String,
    images: bool,
    thinking: bool,
    size: u64,
}

impl Facts {
    pub(super) fn with_size(mut self, size: Option<u64>) -> Self {
        if let Some(size) = size {
            self.size = size;
        }
        self
    }

    pub(super) fn from_body(body: &[u8], path: &str) -> Self {
        let value = serde_json::from_slice::<Value>(body).unwrap_or(Value::Null);
        let model = value
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                path.split("models/")
                    .nth(1)
                    .map(|value| value.split(':').next().unwrap_or(value).to_string())
            })
            .unwrap_or_default();
        let effort = value
            .get("reasoning_effort")
            .and_then(Value::as_str)
            .or_else(|| value.pointer("/reasoning/effort").and_then(Value::as_str))
            .or_else(|| {
                value
                    .pointer("/output_config/effort")
                    .and_then(Value::as_str)
            });
        let thinking = effort.is_some_and(|effort| !matches!(effort, "" | "none"))
            || matches!(
                value.pointer("/thinking/type").and_then(Value::as_str),
                Some("enabled" | "adaptive")
            )
            || value
                .pointer("/generationConfig/thinkingConfig/thinkingLevel")
                .and_then(Value::as_str)
                .is_some_and(|level| {
                    matches!(
                        level.trim().to_ascii_lowercase().as_str(),
                        "minimal" | "low" | "medium" | "high"
                    )
                })
            || value
                .pointer("/generationConfig/thinkingConfig/thinkingBudget")
                .and_then(Value::as_i64)
                .is_some_and(|budget| budget != 0);
        let images = ["messages", "input", "contents"]
            .iter()
            .any(|key| value.get(*key).is_some_and(|content| has_image(content, 0)));
        Self {
            model,
            images,
            thinking,
            size: body.len() as u64,
        }
    }

    pub(super) fn matches(&self, rule: &RoutingRule) -> bool {
        (rule.model.is_empty()
            || match rule.match_mode {
                ModelMatch::Exact => self.model == rule.model,
                ModelMatch::Prefix => self.model.starts_with(&rule.model),
                ModelMatch::Contains => self.model.contains(&rule.model),
            })
            && (!rule.images || self.images)
            && (!rule.thinking || self.thinking)
            && self.size >= rule.min_request_bytes
    }
}

fn has_image(value: &Value, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    match value {
        Value::Array(items) => items.iter().any(|item| has_image(item, depth + 1)),
        Value::Object(fields) => {
            matches!(
                fields.get("type").and_then(Value::as_str),
                Some("image" | "image_url" | "input_image")
            ) || ["inlineData", "fileData", "inline_data", "file_data"]
                .iter()
                .any(|key| {
                    fields
                        .get(*key)
                        .and_then(|value| value.get("mimeType").or_else(|| value.get("mime_type")))
                        .and_then(Value::as_str)
                        .is_some_and(|mime| mime.starts_with("image/"))
                })
                || ["content", "parts"].iter().any(|key| {
                    fields
                        .get(*key)
                        .is_some_and(|value| has_image(value, depth + 1))
                })
        }
        _ => false,
    }
}
