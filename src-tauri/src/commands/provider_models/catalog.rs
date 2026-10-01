use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: Option<String>,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub input_price: Option<String>,
    pub output_price: Option<String>,
    #[serde(default)]
    pub native_endpoints: Option<Vec<String>>,
    #[serde(default)]
    pub supported_reasoning_levels: Option<Vec<String>>,
    #[serde(default)]
    pub input_modalities: Option<Vec<String>>,
    #[serde(default)]
    pub output_modalities: Option<Vec<String>>,
    #[serde(default)]
    pub premium_request_billing: Option<crate::shared::model_billing::ModelBilling>,
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn positive_integer(value: &Value) -> Option<u64> {
    let number = value
        .as_u64()
        .or_else(|| value.as_str()?.trim().parse::<u64>().ok())?;
    // JSON numbers must remain exact when used by the JavaScript editor.
    (number > 0 && number <= 9_007_199_254_740_991).then_some(number)
}

fn first_number(entry: &Value, paths: &[&str]) -> Option<u64> {
    paths
        .iter()
        .find_map(|path| entry.pointer(path).and_then(positive_integer))
}

fn list(value: &Value, object_key: Option<&str>) -> Option<Vec<String>> {
    let entries = value.as_array()?;
    let mut result = Vec::new();
    for entry in entries {
        let name = text(entry).or_else(|| object_key.and_then(|key| entry.get(key)).and_then(text));
        if let Some(name) = name {
            if !result.contains(&name) {
                result.push(name);
            }
        }
    }
    // Distinguish an explicit empty list from a malformed optional field.
    (entries.is_empty() || !result.is_empty()).then_some(result)
}

fn first_list(entry: &Value, paths: &[&str], object_key: Option<&str>) -> Option<Vec<String>> {
    paths.iter().find_map(|path| {
        entry
            .pointer(path)
            .and_then(|value| list(value, object_key))
    })
}

fn price(value: &Value) -> Option<String> {
    let text = if value.is_number() {
        value.to_string()
    } else {
        text(value)?
    };
    let number = text.parse::<f64>().ok()?;
    (number.is_finite() && number >= 0.0).then_some(text)
}

fn model(entry: &Value, gemini: bool) -> Option<ModelInfo> {
    let raw_id = if gemini {
        entry
            .get("name")
            .and_then(text)
            .or_else(|| entry.get("id").and_then(text))
    } else {
        entry.get("id").and_then(text).or_else(|| text(entry))
    }?;
    let id = if gemini {
        raw_id.strip_prefix("models/").unwrap_or(&raw_id)
    } else {
        &raw_id
    };
    if id.is_empty() {
        return None;
    }
    Some(ModelInfo {
        id: id.into(),
        display_name: ["display_name", "displayName", "name"]
            .iter()
            .find_map(|key| entry.get(key).and_then(text))
            .filter(|name| name != &raw_id),
        context_window: first_number(
            entry,
            &[
                "/context_window",
                "/context_length",
                "/max_input_tokens",
                "/inputTokenLimit",
                "/contextWindow",
                "/limit/context",
                "/limits/context",
            ],
        ),
        max_output_tokens: first_number(
            entry,
            &[
                "/max_output_tokens",
                "/max_completion_tokens",
                "/max_tokens",
                "/outputTokenLimit",
                "/maxOutputTokens",
                "/top_provider/max_completion_tokens",
                "/limit/output",
                "/limits/output",
            ],
        ),
        input_price: entry.pointer("/pricing/prompt").and_then(price),
        output_price: entry.pointer("/pricing/completion").and_then(price),
        premium_request_billing: entry
            .get("billing")
            .map(crate::shared::model_billing::ModelBilling::from_value),
        native_endpoints: first_list(
            entry,
            &[
                "/native_endpoints",
                "/nativeEndpoints",
                "/supported_endpoints",
            ],
            None,
        ),
        supported_reasoning_levels: first_list(
            entry,
            &["/supported_reasoning_levels", "/supportedReasoningLevels"],
            Some("effort"),
        ),
        input_modalities: first_list(
            entry,
            &[
                "/modalities/input",
                "/input_modalities",
                "/inputModalities",
                "/architecture/input_modalities",
            ],
            None,
        ),
        output_modalities: first_list(
            entry,
            &[
                "/modalities/output",
                "/output_modalities",
                "/outputModalities",
                "/architecture/output_modalities",
            ],
            None,
        ),
    })
}

/// Optional vendor fields never invalidate usable model IDs. A wrong response
/// container remains an error so a misconfigured endpoint cannot look healthy.
pub(super) fn parse_catalog(payload: &Value, gemini: bool) -> Result<Vec<ModelInfo>, String> {
    let key = if gemini { "models" } else { "data" };
    let entries = payload
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Model response must contain a {key} array"))?;
    Ok(entries
        .iter()
        .filter_map(|entry| model(entry, gemini))
        .collect())
}

pub(super) fn merge_catalog(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    let mut unique: BTreeMap<String, ModelInfo> = BTreeMap::new();
    for next in models {
        let previous = unique.entry(next.id.clone()).or_insert_with(|| ModelInfo {
            id: next.id.clone(),
            ..Default::default()
        });
        macro_rules! enrich {
            ($($field:ident),*) => { $(if next.$field.is_some() { previous.$field = next.$field; })* };
        }
        enrich!(
            display_name,
            context_window,
            max_output_tokens,
            input_price,
            output_price,
            native_endpoints,
            supported_reasoning_levels,
            input_modalities,
            output_modalities
        );
        if let Some(billing) = next.premium_request_billing {
            previous.premium_request_billing = Some(
                previous
                    .premium_request_billing
                    .map(|previous| previous.agree(billing))
                    .unwrap_or(billing),
            );
        }
    }
    unique.into_values().collect()
}

#[cfg(test)]
mod tests;
