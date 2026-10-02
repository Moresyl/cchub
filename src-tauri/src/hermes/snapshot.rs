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

pub(crate) fn apply_snapshot_without_backup(
    conn: &Connection,
    snapshot: &str,
) -> Result<Option<PathBuf>, String> {
    apply_snapshot_impl(conn, snapshot, false)
}

fn apply_snapshot_impl(
    conn: &Connection,
    snapshot: &str,
    create_backup: bool,
) -> Result<Option<PathBuf>, String> {
    let _guard = crate::json_config::write_lock()?;
    let (plan, backup) = prepare_snapshot(conn, snapshot, create_backup)?;
    plan.commit()?;
    Ok(backup)
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
    let mut next_config = match config_bytes.as_deref() {
        Some(bytes) => serde_yaml::from_slice::<Value>(bytes)
            .map_err(|_| "Invalid Hermes YAML configuration")?,
        None => Value::Mapping(Mapping::new()),
    };
    if !next_config.is_mapping() {
        return Err("Hermes configuration must be an object".into());
    }
    {
        let next_root = config::top_level_mapping_mut(&mut next_config);
        let incoming_root = incoming_config_value
            .as_mapping()
            .ok_or_else(|| "Hermes snapshot config must be an object".to_string())?;

        if let Some(incoming_model) = incoming_root.get(Value::String("model".to_string())) {
            let incoming_model = incoming_model
                .as_mapping()
                .ok_or("Hermes snapshot model must be an object")?;
            let model_value = mapping_entry_mut(next_root, "model");
            let next_model = model_value
                .as_mapping_mut()
                .ok_or("Existing Hermes model must be an object")?;
            for key in ["provider", "base_url", "default"] {
                if let Some(value) = incoming_model.get(Value::String(key.to_string())) {
                    if !value.is_string() {
                        return Err("Hermes model fields must contain strings".into());
                    }
                    next_model.insert(Value::String(key.to_string()), value.clone());
                }
            }
        }
    }

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
    let mut plan = crate::config_write::FilePlan::default();
    let mut backup_path = None;
    if create_backup && config_bytes.is_some() && !config::has_existing_backup(&config_path) {
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
    let config_text =
        serde_yaml::to_string(&next_config).map_err(|_| "Cannot encode Hermes YAML")?;
    let env_text = env::render_env_map(&next_env)?;
    plan.updates.push(crate::config_write::FileUpdate {
        path: config_path,
        original: config_bytes,
        desired: config_text.into_bytes(),
    });
    plan.updates.push(crate::config_write::FileUpdate {
        path: env_path,
        original: env_bytes,
        desired: env_text.into_bytes(),
    });
    Ok((plan, backup_path))
}
