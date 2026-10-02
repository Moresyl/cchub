use rusqlite::Connection;
use serde_json::json;
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::path::PathBuf;

use super::{config, env, providers};

fn edit_snapshot_config(source: &str, incoming: &Value) -> Result<String, String> {
    let incoming_root = incoming
        .as_mapping()
        .ok_or("Hermes snapshot config must be an object")?;
    crate::yaml_config::edit_yaml_text(source, |root| {
        if let Some(incoming_model) = incoming_root.get(Value::String("model".into())) {
            let incoming_model = incoming_model
                .as_mapping()
                .ok_or("Hermes snapshot model must be an object")?;
            let mut changes = Mapping::new();
            for key in ["provider", "base_url", "default"] {
                if let Some(value) = incoming_model.get(Value::String(key.into())) {
                    if !value.is_string() {
                        return Err("Hermes model fields must contain strings".into());
                    }
                    changes.insert(Value::String(key.into()), value.clone());
                }
            }
            if changes.is_empty() {
                return Ok(());
            }
            let model_key = Value::String("model".into());
            let mut effective = Value::Mapping(root.clone());
            effective
                .apply_merge()
                .map_err(|_| "Invalid Hermes YAML merge configuration")?;
            if let Some(inherited) = effective.get("model") {
                let inherited = inherited
                    .as_mapping()
                    .ok_or("Existing Hermes model must be an object")?;
                if changes
                    .iter()
                    .all(|(key, value)| inherited.get(key) == Some(value))
                {
                    return Ok(());
                }
                // A root merge may provide the entire model mapping. Adding a
                // partial explicit model would hide all its inherited options.
                if !root.contains_key(&model_key) {
                    root.insert(model_key, Value::Mapping(inherited.clone()));
                }
            }
            let next_model = mapping_entry_mut(root, "model")
                .as_mapping_mut()
                .ok_or("Existing Hermes model must be an object")?;
            next_model.extend(changes);
        }
        Ok(())
    })
}

fn add_target(
    plan: &mut crate::config_write::FilePlan,
    path: PathBuf,
    original: Option<Vec<u8>>,
    desired: Vec<u8>,
    unchanged: bool,
) {
    if unchanged {
        plan.guards.push((path, original));
    } else {
        plan.updates.push(crate::config_write::FileUpdate {
            path,
            original,
            desired,
        });
    }
}

fn json_to_yaml_value(value: &serde_json::Value) -> Result<Value, String> {
    serde_yaml::to_value(value).map_err(|e| e.to_string())
}

fn yaml_to_json_value(value: &Value) -> Result<serde_json::Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

fn mapping_entry_mut<'a>(mapping: &'a mut Mapping, key: &str) -> &'a mut Value {
    mapping
        .entry(Value::String(key.to_string()))
        .or_insert_with(|| Value::Mapping(Mapping::new()))
}

fn mapping_string(mapping: &Mapping, key: &str) -> Option<String> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub fn read_snapshot(conn: &Connection) -> Result<String, String> {
    let config_value = config::read_value(conn)?;
    let env_map = env::read_env_map(conn)?;
    let provider = config_value
        .as_mapping()
        .and_then(|root| root.get(Value::String("model".to_string())))
        .and_then(Value::as_mapping)
        .and_then(|model| mapping_string(model, "provider"));
    let env_key = providers::infer_api_key_env_key(provider.as_deref(), &env_map);

    serde_json::to_string_pretty(&json!({
        "config": yaml_to_json_value(&config_value)?,
        "env": env_map,
        "metadata": {
            "hermesProvider": provider,
            "hermesApiKeyEnv": env_key,
        },
    }))
    .map_err(|e| e.to_string())
}

pub(crate) fn prepare_snapshot(
    conn: &Connection,
    snapshot: &str,
    create_backup: bool,
) -> Result<(crate::config_write::FilePlan, Option<PathBuf>), String> {
    let parsed = crate::json_config::parse_json_object(snapshot)?;
    let incoming_config_value = parsed
        .get("config")
        .ok_or_else(|| "Hermes snapshot is missing config".to_string())
        .and_then(json_to_yaml_value)?;
    let incoming_env_value = parsed
        .get("env")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let incoming_env: HashMap<String, String> = serde_json::from_value(incoming_env_value)
        .map_err(|_| "Hermes environment must contain string values")?;
    env::render_env_map(&incoming_env)?;
    let metadata = parsed
        .get("metadata")
        .map(|value| {
            value
                .as_object()
                .ok_or("Hermes snapshot metadata must be an object")
        })
        .transpose()?
        .cloned()
        .unwrap_or_default();

    let config_path = super::config_path(conn)?;
    let env_path = super::env_path(conn)?;
    let config_bytes = crate::config_write::read(&config_path)?;
    let env_bytes = crate::config_write::read(&env_path)?;
    let config_source = config_bytes
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| "Hermes configuration must be UTF-8")?
        .unwrap_or("{}\n");
    let config_text = edit_snapshot_config(config_source, &incoming_config_value)?;
    let next_config: Value =
        serde_yaml::from_str(config_text.strip_prefix('\u{feff}').unwrap_or(&config_text))
            .map_err(|_| "Invalid Hermes YAML configuration")?;

    // Derive both desired files from the bytes captured by this plan. A second
    // read could combine an external edit with an older revision guard.
    let original_env = env::parse_env_text(
        env_bytes
            .as_deref()
            .map(std::str::from_utf8)
            .transpose()
            .map_err(|_| "Hermes environment must be UTF-8")?
            .unwrap_or(""),
    );
    let mut next_env = original_env.clone();
    let provider = next_config
        .as_mapping()
        .and_then(|root| root.get(Value::String("model".to_string())))
        .and_then(Value::as_mapping)
        .and_then(|model| mapping_string(model, "provider"));
    let target_env_key = metadata
        .get("hermesApiKeyEnv")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| providers::infer_api_key_env_key(provider.as_deref(), &incoming_env))
        .or_else(|| {
            provider
                .as_deref()
                .and_then(providers::default_env_key_for_provider)
                .map(str::to_string)
        });

    if let Some(target_key) = target_env_key.as_deref() {
        for known_key in providers::known_api_key_env_keys() {
            if *known_key != target_key {
                next_env.remove(*known_key);
            }
        }
    }

    for (key, value) in incoming_env {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            next_env.remove(&key);
        } else {
            next_env.insert(key, trimmed.to_string());
        }
    }
    let mut plan = crate::config_write::FilePlan::default();
    let mut backup_path = None;
    let config_unchanged = config_text == config_source;
    if create_backup
        && !config_unchanged
        && config_bytes.is_some()
        && !config::has_existing_backup(&config_path)
    {
        let path = config::backup_path_for(&config_path);
        if crate::config_write::read(&path)?.is_some() {
            return Err("Hermes backup location already exists".into());
        }
        plan.updates.push(crate::config_write::FileUpdate {
            path: path.clone(),
            original: None,
            desired: config_bytes.as_ref().unwrap().clone(),
        });
        backup_path = Some(path);
    }
    let env_text = env::render_env_map(&next_env)?;
    add_target(
        &mut plan,
        config_path,
        config_bytes,
        config_text.into_bytes(),
        config_unchanged,
    );
    add_target(
        &mut plan,
        env_path,
        env_bytes,
        env_text.into_bytes(),
        next_env == original_env,
    );
    Ok((plan, backup_path))
}

#[cfg(test)]
mod tests;
