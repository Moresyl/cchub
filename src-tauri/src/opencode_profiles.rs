use serde_json::{Map, Value};
use std::path::Path;

fn provider_id(profile: &Value) -> Result<String, String> {
    let id = profile
        .pointer("/metadata/nativeProviderId")
        .and_then(Value::as_str)
        .unwrap_or("custom")
        .trim();
    if id.is_empty() || id.len() > 128 || id.contains('/') || id.chars().any(char::is_control) {
        return Err(
            "OpenCode provider ID must be non-empty and contain no slashes or control characters"
                .into(),
        );
    }
    Ok(id.to_string())
}

fn merge_fields(target: &mut Value, source: &Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (name, value) in source {
                if let Some(existing) = target.get_mut(name) {
                    merge_fields(existing, value);
                } else {
                    target.insert(name.clone(), value.clone());
                }
            }
        }
        (target, source) => *target = source.clone(),
    }
}

fn normalize_models(provider: &mut Map<String, Value>) -> Result<(), String> {
    let Some(models) = provider.get_mut("models") else {
        return Ok(());
    };
    let models = models
        .as_object_mut()
        .ok_or("OpenCode models must be a JSON object")?;
    for model in models.values_mut() {
        let model = model
            .as_object_mut()
            .ok_or("OpenCode model entries must be JSON objects")?;
        let context = model.remove("contextLimit");
        let output = model.remove("outputLimit");
        if context.is_some() || output.is_some() {
            let limit = model
                .entry("limit")
                .or_insert_with(|| serde_json::json!({}));
            let limit = limit
                .as_object_mut()
                .ok_or("OpenCode model limit must be a JSON object")?;
            for (key, value) in [("context", context), ("output", output)] {
                if let Some(value) = value {
                    if value.is_null() {
                        limit.remove(key);
                        continue;
                    }
                    if value.as_u64().filter(|number| *number > 0).is_none() {
                        return Err("OpenCode token limits must be positive integers".into());
                    }
                    limit.insert(key.into(), value);
                }
            }
            if limit.is_empty() {
                model.remove("limit");
            }
        }
    }
    Ok(())
}

/// Convert the selected native provider to the profile format consumed by the
/// connection form and proxy. Reading never writes or creates a configuration.
pub(crate) fn read_profile(path: &Path) -> Result<String, String> {
    let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let document = crate::json_config::parse_json_object(&source)?;
    serde_json::to_string_pretty(&extract_profile(&document)?).map_err(|error| error.to_string())
}

fn extract_profile(document: &Value) -> Result<Value, String> {
    if document.get("options").is_some() || document.get("npm").is_some() {
        return Ok(document.clone());
    }
    let empty = Map::new();
    let providers = match document.get("provider") {
        Some(value) => value
            .as_object()
            .ok_or("OpenCode provider container must be a JSON object")?,
        None => &empty,
    };
    let selected = document
        .get("model")
        .and_then(Value::as_str)
        .and_then(|model| model.split_once('/'));
    let default_provider = Value::Object(Map::new());
    let (id, provider) = match selected {
        Some((id, _)) => (id, providers.get(id).unwrap_or(&default_provider)),
        None => providers
            .iter()
            .next()
            .map(|(id, provider)| (id.as_str(), provider))
            .ok_or("No OpenCode provider configuration found")?,
    };
    let mut profile = provider
        .as_object()
        .cloned()
        .ok_or("OpenCode provider must be a JSON object")?;
    if let Some(models) = profile.get_mut("models").and_then(Value::as_object_mut) {
        for model in models.values_mut().filter_map(Value::as_object_mut) {
            for (native, form) in [("context", "contextLimit"), ("output", "outputLimit")] {
                if let Some(value) = model
                    .get("limit")
                    .and_then(|limit| limit.get(native))
                    .cloned()
                {
                    model.insert(form.into(), value);
                }
            }
        }
    }
    let metadata = profile
        .entry("metadata")
        .or_insert_with(|| serde_json::json!({}));
    let metadata = metadata
        .as_object_mut()
        .ok_or("OpenCode provider metadata must be a JSON object")?;
    metadata.insert("nativeProviderId".into(), Value::String(id.into()));
    if let Some((_, model)) = selected.filter(|(selected_id, _)| *selected_id == id) {
        metadata.insert("nativeModelId".into(), Value::String(model.into()));
    } else {
        metadata.insert("nativeModelId".into(), Value::String(String::new()));
    }
    Ok(Value::Object(profile))
}

/// Upsert one provider and its selected model without replacing the user's
/// native settings, MCP servers, plugins or unrelated providers.
pub(crate) fn normalize_profile(snapshot: &str) -> Result<String, String> {
    let document = crate::json_config::parse_json_object(snapshot)?;
    let profile = if document.get("provider").is_some()
        || document.get("model").is_some_and(Value::is_string)
    {
        extract_profile(&document)?
    } else {
        document
    };
    provider_id(&profile)?;
    let mut provider = profile.as_object().unwrap().clone();
    for key in ["options", "models"] {
        if provider.get(key).is_some_and(|value| !value.is_object()) {
            return Err(format!("OpenCode {key} must be a JSON object"));
        }
    }
    normalize_models(&mut provider)?;
    serde_json::to_string_pretty(&profile).map_err(|error| error.to_string())
}

#[cfg(test)]
pub(crate) fn apply_profile(path: &Path, snapshot: &str) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    prepare_profile(path, snapshot)?.commit()
}

pub(crate) fn prepare_profile(
    path: &Path,
    snapshot: &str,
) -> Result<crate::config_write::FilePlan, String> {
    let original = crate::config_write::read(path)?;
    let source = original
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| "OpenCode configuration must be UTF-8")?
        .unwrap_or("{}\n");
    let profile = crate::json_config::parse_json_object(&normalize_profile(snapshot)?)?;
    let id = provider_id(&profile)?;
    let mut provider = profile.as_object().unwrap().clone();
    let cleared_limits: Vec<(String, &'static str)> = provider
        .get("models")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|models| models.iter())
        .flat_map(|(id, model)| {
            [("contextLimit", "context"), ("outputLimit", "output")]
                .into_iter()
                .filter(|(form, _)| model.get(*form).is_some_and(Value::is_null))
                .map(|(_, native)| (id.clone(), native))
                .collect::<Vec<_>>()
        })
        .collect();
    for key in ["metadata", "customEndpoints", "custom_endpoints"] {
        provider.remove(key);
    }
    for key in ["options", "models"] {
        if provider.get(key).is_some_and(|value| !value.is_object()) {
            return Err(format!("OpenCode {key} must be a JSON object"));
        }
    }
    normalize_models(&mut provider)?;
    let models = provider.get("models").and_then(Value::as_object);
    let explicit_selection = profile
        .pointer("/metadata/nativeModelId")
        .and_then(Value::as_str);
    let clear_selection = explicit_selection.is_some_and(|model| model.trim().is_empty());
    let selected = match explicit_selection {
        Some(model) => (!model.trim().is_empty()).then(|| model.to_string()),
        None => models.and_then(|models| models.keys().next().cloned()),
    };
    let desired = crate::json_config::edit_json_text(source, |document| {
        let root = document.as_object_mut().unwrap();
        // Repair files emitted by older versions that put the provider at root.
        if root.contains_key("options") || root.contains_key("npm") {
            for key in [
                "npm",
                "options",
                "models",
                "name",
                "metadata",
                "customEndpoints",
                "custom_endpoints",
            ] {
                root.remove(key);
            }
        }
        let providers = root
            .entry("provider")
            .or_insert_with(|| serde_json::json!({}));
        let providers = providers
            .as_object_mut()
            .ok_or("OpenCode provider container must be a JSON object")?;
        let target = providers
            .entry(id.clone())
            .or_insert_with(|| serde_json::json!({}));
        if !target.is_object() {
            return Err("Existing OpenCode provider must be a JSON object".into());
        }
        merge_fields(target, &Value::Object(provider));
        for (model_id, key) in &cleared_limits {
            if let Some(model) = target
                .get_mut("models")
                .and_then(|models| models.get_mut(model_id))
                .and_then(Value::as_object_mut)
            {
                if let Some(limit) = model.get_mut("limit").and_then(Value::as_object_mut) {
                    limit.remove(*key);
                    if limit.is_empty() {
                        model.remove("limit");
                    }
                }
            }
        }
        if let Some(model) = selected {
            root.insert("model".into(), Value::String(format!("{id}/{model}")));
        } else if clear_selection
            && root
                .get("model")
                .and_then(Value::as_str)
                .is_some_and(|model| {
                    model
                        .split_once('/')
                        .is_some_and(|(provider, _)| provider == id)
                })
        {
            root.remove("model");
        }
        Ok(())
    })?;
    Ok(crate::config_write::FilePlan {
        updates: vec![crate::config_write::FileUpdate {
            path: path.into(),
            original,
            desired: desired.into_bytes(),
        }],
        guards: Vec::new(),
    })
}

#[cfg(test)]
#[path = "opencode_profiles_tests.rs"]
mod tests;
