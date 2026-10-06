use serde_json::{Map, Value};

fn check(value: &Value, path: &str, valid: impl FnOnce(&Value) -> bool) -> Result<(), String> {
    if valid(value) {
        Ok(())
    } else {
        Err(format!("Invalid OpenCode field: {path}"))
    }
}

fn field(
    object: &Map<String, Value>,
    name: &str,
    path: &str,
    valid: impl FnOnce(&Value) -> bool,
) -> Result<(), String> {
    if let Some(value) = object.get(name) {
        check(value, &format!("{path}.{name}"), valid)?;
    }
    Ok(())
}

fn strings(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(Value::is_string))
}

fn integer(value: &Value) -> bool {
    value.as_i64().is_some() || value.as_u64().is_some()
}

fn compaction(value: &Value) -> bool {
    value.is_object()
        && matches!(
            value.get("type").and_then(Value::as_str),
            Some("summary" | "native")
        )
}

fn overlays(object: &Map<String, Value>, path: &str, provider: bool) -> Result<(), String> {
    field(object, "body", path, Value::is_object)?;
    field(object, "headers", path, |value| {
        value
            .as_object()
            .is_some_and(|items| items.values().all(Value::is_string))
    })?;
    if let Some(settings) = object.get("settings") {
        check(settings, &format!("{path}.settings"), Value::is_object)?;
        let settings = settings.as_object().unwrap();
        let path = format!("{path}.settings");
        field(settings, "compaction", &path, compaction)?;
        if provider {
            field(settings, "timeout", &path, |value| {
                value.is_number() || value == &Value::Bool(false)
            })?;
            field(settings, "chunkTimeout", &path, Value::is_number)?;
            field(settings, "transport", &path, |value| {
                matches!(value.as_str(), Some("http" | "websocket"))
            })?;
        }
    }
    Ok(())
}

fn cost(value: &Value, path: &str) -> Result<(), String> {
    check(value, path, Value::is_object)?;
    let object = value.as_object().unwrap();
    for name in ["input", "output"] {
        check(
            object.get(name).unwrap_or(&Value::Null),
            &format!("{path}.{name}"),
            Value::is_number,
        )?;
    }
    if let Some(cache) = object.get("cache") {
        check(cache, &format!("{path}.cache"), Value::is_object)?;
        for name in ["read", "write"] {
            field(
                cache.as_object().unwrap(),
                name,
                &format!("{path}.cache"),
                Value::is_number,
            )?;
        }
    }
    if let Some(tier) = object.get("tier") {
        check(tier, &format!("{path}.tier"), |value| {
            value.is_object()
                && value.get("type").and_then(Value::as_str) == Some("context")
                && value.get("size").is_some_and(integer)
        })?;
    }
    Ok(())
}

fn model(value: &Value, path: &str) -> Result<(), String> {
    check(value, path, Value::is_object)?;
    let object = value.as_object().unwrap();
    for name in ["modelID", "family", "name", "package"] {
        field(object, name, path, Value::is_string)?;
    }
    field(object, "disabled", path, Value::is_boolean)?;
    overlays(object, path, false)?;
    if let Some(limit) = object.get("limit") {
        check(limit, &format!("{path}.limit"), Value::is_object)?;
        for name in ["context", "input", "output"] {
            field(
                limit.as_object().unwrap(),
                name,
                &format!("{path}.limit"),
                integer,
            )?;
        }
    }
    if let Some(compatibility) = object.get("compatibility") {
        let path = format!("{path}.compatibility");
        check(compatibility, &path, Value::is_object)?;
        let compatibility = compatibility.as_object().unwrap();
        field(compatibility, "reasoningField", &path, Value::is_string)?;
        field(compatibility, "maxTokensField", &path, |value| {
            matches!(value.as_str(), Some("max_tokens" | "max_completion_tokens"))
        })?;
        for name in [
            "requireReasoning",
            "requireFinishReason",
            "requireAssistantAfterTool",
            "supportsPromptCacheKey",
        ] {
            field(compatibility, name, &path, Value::is_boolean)?;
        }
    }
    if let Some(capabilities) = object.get("capabilities") {
        let path = format!("{path}.capabilities");
        check(capabilities, &path, Value::is_object)?;
        let capabilities = capabilities.as_object().unwrap();
        check(
            capabilities.get("tools").unwrap_or(&Value::Null),
            &format!("{path}.tools"),
            Value::is_boolean,
        )?;
        for name in ["input", "output"] {
            check(
                capabilities.get(name).unwrap_or(&Value::Null),
                &format!("{path}.{name}"),
                strings,
            )?;
        }
    }
    if let Some(variants) = object.get("variants") {
        check(variants, &format!("{path}.variants"), Value::is_array)?;
        for (index, variant) in variants.as_array().unwrap().iter().enumerate() {
            let path = format!("{path}.variants[{index}]");
            check(variant, &path, Value::is_object)?;
            let variant = variant.as_object().unwrap();
            check(
                variant.get("id").unwrap_or(&Value::Null),
                &format!("{path}.id"),
                Value::is_string,
            )?;
            overlays(variant, &path, false)?;
        }
    }
    if let Some(value) = object.get("cost") {
        if let Some(items) = value.as_array() {
            for (index, item) in items.iter().enumerate() {
                cost(item, &format!("{path}.cost[{index}]"))?;
            }
        } else {
            cost(value, &format!("{path}.cost"))?;
        }
    }
    Ok(())
}

/// Validate known native fields while keeping extension fields byte-for-byte in the value.
pub(super) fn validate(value: &Value) -> Result<(), String> {
    check(value, "providers", Value::is_object)?;
    let object = value.as_object().unwrap();
    for name in ["canonical", "name", "package"] {
        field(object, name, "providers", Value::is_string)?;
    }
    field(object, "env", "providers", strings)?;
    overlays(object, "providers", true)?;
    if let Some(models) = object.get("models") {
        check(models, "providers.models", Value::is_object)?;
        for (id, value) in models.as_object().unwrap() {
            model(value, &format!("providers.models[{id:?}]"))?;
        }
    }
    Ok(())
}

pub(super) fn is_native(profile: &Value) -> bool {
    profile
        .pointer("/metadata/nativeFormat")
        .and_then(Value::as_str)
        == Some("providers")
        || ["package", "canonical", "settings"]
            .iter()
            .any(|key| profile.get(*key).is_some())
}

pub(super) fn selection(document: &Value) -> Result<Option<(String, String, Value)>, String> {
    let Some(value) = document.get("model") else {
        return Ok(None);
    };
    let (provider, model) = if let Some(value) = value.as_str() {
        let (provider, model) = value
            .split_once('/')
            .ok_or("Invalid OpenCode model selection")?;
        (provider.to_string(), model.to_string())
    } else if let Some(value) = value.as_object() {
        let provider = value
            .get("providerID")
            .and_then(Value::as_str)
            .ok_or("Invalid OpenCode model.providerID")?;
        let model = value
            .get("model")
            .and_then(Value::as_str)
            .ok_or("Invalid OpenCode model.model")?;
        let mut model = model.to_string();
        if let Some(variant) = value.get("variant") {
            let variant = variant
                .as_str()
                .filter(|value| !value.is_empty() && !value.contains('#'))
                .ok_or("Invalid OpenCode model.variant")?;
            model.push('#');
            model.push_str(variant);
        }
        (provider.to_string(), model)
    } else {
        return Err("Invalid OpenCode model selection".into());
    };
    let mut parts = model.split('#');
    let model_id = parts.next().unwrap_or_default();
    let variant = parts.next();
    if provider.is_empty()
        || provider.contains(['/', '#'])
        || model_id.is_empty()
        || variant.is_some_and(str::is_empty)
        || parts.next().is_some()
        || provider.chars().chain(model.chars()).any(char::is_control)
    {
        return Err("Invalid OpenCode model selection".into());
    }
    Ok(Some((provider, model, value.clone())))
}
