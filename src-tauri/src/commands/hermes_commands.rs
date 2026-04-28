use serde_yaml::{Mapping, Value};
use tauri::State;

use crate::db::DbState;
use crate::hermes;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesMemoryLimits {
    pub memory: u32,
    pub user: u32,
    pub memory_enabled: bool,
    pub user_enabled: bool,
}

#[tauri::command]
pub fn get_hermes_memory_limits(db: State<'_, DbState>) -> Result<HermesMemoryLimits, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let config = hermes::config::read_value(&conn)?;

    let memory_section = config.get("memory");
    let memory_enabled = memory_section
        .and_then(|m| m.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let memory_limit = memory_section
        .and_then(|m| m.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(2200) as u32;

    let user_section = config.get("user");
    let user_enabled = user_section
        .and_then(|u| u.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let user_limit = user_section
        .and_then(|u| u.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1375) as u32;

    Ok(HermesMemoryLimits {
        memory: memory_limit,
        user: user_limit,
        memory_enabled,
        user_enabled,
    })
}

#[tauri::command]
pub fn get_hermes_memory_content(
    db: State<'_, DbState>,
    kind: String,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let root = hermes::hermes_root(&conn)?;
    let filename = match kind.as_str() {
        "memory" => "MEMORY.md",
        "user" => "USER.md",
        _ => return Err(format!("Unknown memory kind: {kind}")),
    };
    let path = root.join(filename);
    if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| e.to_string())
    } else {
        Ok(String::new())
    }
}

#[tauri::command]
pub fn save_hermes_memory_content(
    db: State<'_, DbState>,
    kind: String,
    content: String,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let root = hermes::hermes_root(&conn)?;
    let filename = match kind.as_str() {
        "memory" => "MEMORY.md",
        "user" => "USER.md",
        _ => return Err(format!("Unknown memory kind: {kind}")),
    };
    let path = root.join(filename);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_hermes_memory_enabled(
    db: State<'_, DbState>,
    kind: String,
    enabled: bool,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut config = hermes::config::read_value(&conn)?;

    let section_key = match kind.as_str() {
        "memory" => "memory",
        "user" => "user",
        _ => return Err(format!("Unknown memory kind: {kind}")),
    };

    let top = hermes::config::top_level_mapping_mut(&mut config);
    let section = top
        .entry(serde_yaml::Value::String(section_key.to_string()))
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

    if let serde_yaml::Value::Mapping(map) = section {
        map.insert(
            serde_yaml::Value::String("enabled".to_string()),
            serde_yaml::Value::Bool(enabled),
        );
    }

    hermes::config::write_value(&conn, &config)?;
    Ok(())
}

// --- Hermes Provider CRUD ---

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HermesProvider {
    pub name: String,
    pub base_url: String,
    pub api_mode: String,
    pub model: String,
    pub api_key_env: String,
}

fn yaml_key(key: &str) -> Value {
    Value::String(key.to_string())
}

fn yaml_str(mapping: &Mapping, key: &str) -> String {
    mapping
        .get(yaml_key(key))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn parse_provider_from_mapping(entry: &Mapping) -> Option<HermesProvider> {
    let name = yaml_str(entry, "name");
    if name.is_empty() {
        return None;
    }
    Some(HermesProvider {
        name,
        base_url: yaml_str(entry, "base_url"),
        api_mode: {
            let mode = yaml_str(entry, "api_mode");
            if mode.is_empty() { "chat_completions".to_string() } else { mode }
        },
        model: yaml_str(entry, "model"),
        api_key_env: yaml_str(entry, "api_key_env"),
    })
}

fn get_custom_providers_seq(config: &Value) -> Vec<HermesProvider> {
    config
        .as_mapping()
        .and_then(|root| root.get(yaml_key("custom_providers")))
        .and_then(Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(|entry| entry.as_mapping().and_then(parse_provider_from_mapping))
                .collect()
        })
        .unwrap_or_default()
}

fn provider_to_yaml(provider: &HermesProvider) -> Value {
    let mut entry = Mapping::new();
    entry.insert(yaml_key("name"), Value::String(provider.name.clone()));
    entry.insert(yaml_key("base_url"), Value::String(provider.base_url.clone()));
    entry.insert(yaml_key("api_mode"), Value::String(provider.api_mode.clone()));
    entry.insert(yaml_key("model"), Value::String(provider.model.clone()));
    if !provider.api_key_env.is_empty() {
        entry.insert(yaml_key("api_key_env"), Value::String(provider.api_key_env.clone()));
    }
    Value::Mapping(entry)
}

#[tauri::command]
pub fn list_hermes_providers(db: State<'_, DbState>) -> Result<Vec<HermesProvider>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let config = hermes::config::read_value(&conn)?;
    Ok(get_custom_providers_seq(&config))
}

#[tauri::command]
pub fn get_hermes_provider(
    db: State<'_, DbState>,
    name: String,
) -> Result<HermesProvider, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let config = hermes::config::read_value(&conn)?;
    get_custom_providers_seq(&config)
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Provider '{}' not found", name))
}

#[tauri::command]
pub fn save_hermes_provider(
    db: State<'_, DbState>,
    provider: HermesProvider,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut config = hermes::config::read_value(&conn)?;
    let top = hermes::config::top_level_mapping_mut(&mut config);

    let seq_value = top
        .entry(yaml_key("custom_providers"))
        .or_insert_with(|| Value::Sequence(Vec::new()));

    let seq = match seq_value {
        Value::Sequence(s) => s,
        _ => {
            *seq_value = Value::Sequence(Vec::new());
            match seq_value {
                Value::Sequence(s) => s,
                _ => unreachable!(),
            }
        }
    };

    let existing_idx = seq.iter().position(|entry| {
        entry
            .as_mapping()
            .and_then(|m| m.get(yaml_key("name")))
            .and_then(Value::as_str)
            == Some(&provider.name)
    });

    let yaml_entry = provider_to_yaml(&provider);
    if let Some(idx) = existing_idx {
        seq[idx] = yaml_entry;
    } else {
        seq.push(yaml_entry);
    }

    // Also update the .env file if api_key_env is set
    if !provider.api_key_env.is_empty() {
        let env_map = hermes::env::read_env_map(&conn)?;
        if !env_map.contains_key(&provider.api_key_env) {
            // Don't overwrite existing keys, just ensure the key exists
        }
    }

    hermes::config::write_value(&conn, &config)?;
    Ok(())
}

#[tauri::command]
pub fn delete_hermes_provider(
    db: State<'_, DbState>,
    name: String,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut config = hermes::config::read_value(&conn)?;
    let top = hermes::config::top_level_mapping_mut(&mut config);

    if let Some(Value::Sequence(seq)) = top.get_mut(yaml_key("custom_providers")) {
        seq.retain(|entry| {
            entry
                .as_mapping()
                .and_then(|m| m.get(yaml_key("name")))
                .and_then(Value::as_str)
                != Some(&name)
        });
    }

    hermes::config::write_value(&conn, &config)?;
    Ok(())
}

#[tauri::command]
pub fn set_hermes_active_provider(
    db: State<'_, DbState>,
    name: String,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut config = hermes::config::read_value(&conn)?;
    let providers = get_custom_providers_seq(&config);
    let provider = providers
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Provider '{}' not found", name))?;

    let top = hermes::config::top_level_mapping_mut(&mut config);
    let model_value = top
        .entry(yaml_key("model"))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let model = match model_value {
        Value::Mapping(m) => m,
        _ => {
            *model_value = Value::Mapping(Mapping::new());
            match model_value {
                Value::Mapping(m) => m,
                _ => unreachable!(),
            }
        }
    };

    model.insert(yaml_key("provider"), Value::String(provider.name.clone()));
    if !provider.base_url.is_empty() {
        model.insert(yaml_key("base_url"), Value::String(provider.base_url.clone()));
    }
    if !provider.model.is_empty() {
        model.insert(yaml_key("default"), Value::String(provider.model.clone()));
    }

    hermes::config::write_value(&conn, &config)?;
    Ok(())
}

#[tauri::command]
pub fn get_hermes_active_provider(db: State<'_, DbState>) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let config = hermes::config::read_value(&conn)?;
    let name = config
        .as_mapping()
        .and_then(|root| root.get(yaml_key("model")))
        .and_then(Value::as_mapping)
        .and_then(|model| model.get(yaml_key("provider")))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok(name)
}
