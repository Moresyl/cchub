use serde_json::{Map, Value};
use std::path::Path;

pub(crate) mod connection;
mod native;

pub(crate) fn is_native_profile(profile: &Value) -> bool {
    native::is_native(profile)
}

fn provider_id(profile: &Value) -> Result<String, String> {
    let id = profile
        .pointer("/metadata/nativeProviderId")
        .and_then(Value::as_str)
        .unwrap_or("custom");
    if id.trim().is_empty()
        || id.len() > 128
        || id.contains(['/', '#'])
        || id.chars().any(char::is_control)
    {
        return Err(
            "OpenCode provider ID must be non-empty and contain no slashes, hashes or control characters"
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

fn merge_native_fields(target: &mut Value, source: &Value) {
    let before = target.clone();
    merge_fields(target, source);
    // Native overlays are edited as complete maps, so removed credentials and
    // header entries must not reappear through a recursive merge.
    for key in ["settings", "headers", "body", "variants"] {
        if let Some(value) = source.get(key) {
            target[key] = value.clone();
        }
    }
    if let Some(models) = source.get("models").and_then(Value::as_object) {
        for (id, source) in models {
            let mut model = before
                .get("models")
                .and_then(|models| models.get(id))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            merge_native_fields(&mut model, source);
            target["models"][id] = model;
        }
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

fn valid_legacy(value: &Value) -> bool {
    let Some(mut provider) = value.as_object().cloned() else {
        return false;
    };
    ["options", "models"]
        .iter()
        .all(|key| provider.get(*key).is_none_or(Value::is_object))
        && normalize_models(&mut provider).is_ok()
}

/// Convert the selected native provider to the profile format consumed by the
/// connection form and proxy. Reading never writes or creates a configuration.
pub(crate) fn read_profile(path: &Path) -> Result<String, String> {
    let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let document = crate::json_config::parse_json_object(&source)?;
    serde_json::to_string_pretty(&extract_profile(&document)?).map_err(|error| error.to_string())
}

fn extract_profile(document: &Value) -> Result<Value, String> {
    if document.get("provider").is_none()
        && document.get("providers").is_none()
        && (document.get("options").is_some()
            || document.get("npm").is_some()
            || native::is_native(document))
    {
        return Ok(document.clone());
    }
    let empty = Map::new();
    let legacy = match document.get("provider") {
        Some(value) => value
            .as_object()
            .ok_or("OpenCode provider container must be a JSON object")?,
        None => &empty,
    };
    let current = match document.get("providers") {
        Some(value) => value
            .as_object()
            .ok_or("OpenCode providers container must be a JSON object")?,
        None => &empty,
    };
    let selected = native::selection(document)?;
    let mut providers: std::collections::BTreeMap<&str, (&Value, bool)> = legacy
        .iter()
        .filter(|(_, value)| valid_legacy(value))
        .map(|(id, value)| (id.as_str(), (value, false)))
        .collect();
    for (id, value) in current {
        if native::validate(value).is_ok() {
            providers.insert(id, (value, true));
        }
    }
    let default_provider = Value::Object(Map::new());
    let (id, provider, is_native) = match selected.as_ref() {
        Some((id, _, _)) => {
            if !providers.contains_key(id.as_str()) {
                if let Some(value) = current.get(id) {
                    native::validate(value)?;
                }
                if legacy.get(id).is_some_and(|value| !valid_legacy(value)) {
                    return Err("OpenCode provider must be a JSON object".into());
                }
            }
            let (provider, is_native) = providers
                .get(id.as_str())
                .copied()
                .unwrap_or((&default_provider, document.get("providers").is_some()));
            (id.as_str(), provider, is_native)
        }
        None => providers
            .iter()
            .next()
            .map(|(id, (provider, is_native))| (*id, *provider, *is_native))
            .ok_or("No OpenCode provider configuration found")?,
    };
    let mut profile = provider
        .as_object()
        .cloned()
        .ok_or("OpenCode provider must be a JSON object")?;
    if let Some(models) = profile
        .get_mut("models")
        .and_then(Value::as_object_mut)
        .filter(|_| !is_native)
    {
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
    if is_native {
        metadata.insert("nativeFormat".into(), Value::String("providers".into()));
    }
    if let Some((_, model, selection)) = selected
        .as_ref()
        .filter(|(selected_id, _, _)| selected_id == id)
    {
        metadata.insert("nativeModelId".into(), Value::String(model.clone()));
        if is_native {
            metadata.insert("nativeModelSelection".into(), selection.clone());
        }
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
        || document.get("providers").is_some()
        || document.get("model").is_some()
    {
        extract_profile(&document)?
    } else {
        document
    };
    provider_id(&profile)?;
    if let Some(metadata) = profile.get("metadata") {
        let metadata = metadata
            .as_object()
            .ok_or("OpenCode metadata must be a JSON object")?;
        for key in ["nativeProviderId", "nativeModelId", "nativeFormat"] {
            if metadata.get(key).is_some_and(|value| !value.is_string()) {
                return Err(format!("Invalid OpenCode metadata.{key}"));
            }
        }
        if metadata
            .get("nativeFormat")
            .and_then(Value::as_str)
            .is_some_and(|value| value != "providers" && value != "provider")
        {
            return Err("Invalid OpenCode metadata.nativeFormat".into());
        }
        if let Some(model) = metadata
            .get("nativeModelId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            native::selection(
                &serde_json::json!({"model":format!("{}/{model}", provider_id(&profile)?)}),
            )?;
        }
        if let Some(selection) = metadata.get("nativeModelSelection") {
            let (id, _, _) = native::selection(&serde_json::json!({"model":selection}))?.unwrap();
            if id != provider_id(&profile)? {
                return Err("OpenCode model selection must use the profile provider".into());
            }
        }
    }
    if native::is_native(&profile) {
        native::validate(&profile)?;
        let mut profile = profile;
        profile
            .as_object_mut()
            .unwrap()
            .entry("metadata")
            .or_insert_with(|| serde_json::json!({}))["nativeFormat"] =
            Value::String("providers".into());
        return serde_json::to_string_pretty(&profile).map_err(|error| error.to_string());
    }
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
    let is_native = native::is_native(&profile);
    let mut provider = profile.as_object().unwrap().clone();
    let cleared_limits: Vec<(String, &'static str)> = provider
        .get("models")
        .and_then(Value::as_object)
        .filter(|_| !is_native)
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
    if !is_native {
        normalize_models(&mut provider)?;
    }
    let models = provider.get("models").and_then(Value::as_object);
    let explicit_selection = profile
        .pointer("/metadata/nativeModelId")
        .and_then(Value::as_str);
    let clear_selection = explicit_selection.is_some_and(|model| model.trim().is_empty());
    let selected = match explicit_selection {
        Some(model) => (!model.trim().is_empty()).then(|| model.to_string()),
        None => models.and_then(|models| models.keys().next().cloned()),
    };
    let selection = selected.as_ref().map(|model| {
        let retained = profile.pointer("/metadata/nativeModelSelection");
        let matches = retained
            .and_then(|value| native::selection(&serde_json::json!({"model":value})).ok())
            .flatten()
            .is_some_and(|(provider, selected, _)| provider == id && selected == *model);
        if is_native && matches {
            retained.unwrap().clone()
        } else {
            Value::String(format!("{id}/{model}"))
        }
    });
    if let Some(selection) = &selection {
        native::selection(&serde_json::json!({"model":selection}))?;
    }
    let desired = crate::json_config::edit_json_text(source, |document| {
        let root = document.as_object_mut().unwrap();
        if !is_native
            && root
                .get("providers")
                .and_then(|value| value.get(&id))
                .is_some_and(|value| native::validate(value).is_ok())
        {
            return Err("A native OpenCode provider shadows this legacy configuration; edit the native configuration instead".into());
        }
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
            .entry(if is_native { "providers" } else { "provider" })
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
        if is_native {
            merge_native_fields(target, &Value::Object(provider));
        } else {
            merge_fields(target, &Value::Object(provider));
        }
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
        if let Some(selection) = selection {
            root.insert("model".into(), selection);
        } else if clear_selection
            && native::selection(&Value::Object(root.clone()))?
                .is_some_and(|(provider, _, _)| provider == id)
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

#[cfg(test)]
#[path = "opencode_profiles/native_tests.rs"]
mod native_tests;

#[cfg(test)]
#[path = "opencode_profiles/connection_tests.rs"]
mod connection_tests;
