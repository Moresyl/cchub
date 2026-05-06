use rusqlite::Connection;
use serde_json::json;
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::path::PathBuf;

use super::{config, env, providers};

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

pub fn apply_snapshot(conn: &Connection, snapshot: &str) -> Result<Option<PathBuf>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| format!("Invalid Hermes snapshot JSON: {e}"))?;
    let incoming_config_value = parsed
        .get("config")
        .ok_or_else(|| "Hermes snapshot is missing config".to_string())
        .and_then(json_to_yaml_value)?;
    let incoming_env_value = parsed
        .get("env")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let incoming_env: HashMap<String, String> =
        serde_json::from_value(incoming_env_value).map_err(|e| e.to_string())?;
    let metadata = parsed
        .get("metadata")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut next_config = config::read_value(conn)?;
    {
        let next_root = config::top_level_mapping_mut(&mut next_config);
        let incoming_root = incoming_config_value
            .as_mapping()
            .ok_or_else(|| "Hermes snapshot config must be an object".to_string())?;

        if let Some(incoming_model) = incoming_root
            .get(Value::String("model".to_string()))
            .and_then(Value::as_mapping)
        {
            let model_value = mapping_entry_mut(next_root, "model");
            let next_model = config::top_level_mapping_mut(model_value);
            for key in ["provider", "base_url", "default"] {
                if let Some(value) = incoming_model.get(Value::String(key.to_string())) {
                    next_model.insert(Value::String(key.to_string()), value.clone());
                }
            }
        }
    }

    let backup_path = config::write_value(conn, &next_config)?;

    let mut next_env = env::read_env_map(conn)?;
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
    env::write_env_map(conn, &next_env)?;

    Ok(backup_path)
}
