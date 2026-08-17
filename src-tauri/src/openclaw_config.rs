use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn get_openclaw_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openclaw")
}

fn get_openclaw_config_path() -> PathBuf {
    get_openclaw_dir().join("openclaw.json")
}

fn read_config() -> Result<Value, String> {
    let path = get_openclaw_config_path();
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("读取 openclaw.json 失败: {e}"))?;
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(trimmed).map_err(|e| format!("解析 openclaw.json 失败: {e}"))
}

fn write_config(value: &Value) -> Result<(), String> {
    let path = get_openclaw_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())
}

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawEnvConfig {
    #[serde(flatten)]
    pub vars: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawToolsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawDefaultModel {
    pub primary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawModelCatalogEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawAgentsDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<OpenClawDefaultModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<HashMap<String, OpenClawModelCatalogEntry>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawHealthWarning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

// ── Public API ──

pub fn get_env_config() -> Result<OpenClawEnvConfig, String> {
    let config = read_config()?;
    match config.get("env") {
        Some(env_val) => {
            serde_json::from_value(env_val.clone()).map_err(|e| format!("解析 env 配置失败: {e}"))
        }
        None => Ok(OpenClawEnvConfig {
            vars: HashMap::new(),
        }),
    }
}

pub fn set_env_config(env: &OpenClawEnvConfig) -> Result<(), String> {
    let mut config = read_config()?;
    let obj = config.as_object_mut().ok_or("配置不是有效的 JSON 对象")?;
    let env_value = serde_json::to_value(env).map_err(|e| e.to_string())?;
    obj.insert("env".to_string(), env_value);
    write_config(&config)
}

pub fn get_tools_config() -> Result<OpenClawToolsConfig, String> {
    let config = read_config()?;
    match config.get("tools") {
        Some(tools_val) => serde_json::from_value(tools_val.clone())
            .map_err(|e| format!("解析 tools 配置失败: {e}")),
        None => Ok(OpenClawToolsConfig {
            profile: None,
            allow: Vec::new(),
            deny: Vec::new(),
            extra: HashMap::new(),
        }),
    }
}

pub fn set_tools_config(tools: &OpenClawToolsConfig) -> Result<(), String> {
    let mut config = read_config()?;
    let obj = config.as_object_mut().ok_or("配置不是有效的 JSON 对象")?;
    let tools_value = serde_json::to_value(tools).map_err(|e| e.to_string())?;
    obj.insert("tools".to_string(), tools_value);
    write_config(&config)
}

pub fn get_agents_defaults() -> Result<OpenClawAgentsDefaults, String> {
    let config = read_config()?;
    let agents = config.get("agents").and_then(|a| a.get("defaults"));
    match agents {
        Some(defaults_val) => serde_json::from_value(defaults_val.clone())
            .map_err(|e| format!("解析 agents.defaults 配置失败: {e}")),
        None => Ok(OpenClawAgentsDefaults {
            model: None,
            models: None,
            extra: HashMap::new(),
        }),
    }
}

pub fn set_agents_defaults(defaults: &OpenClawAgentsDefaults) -> Result<(), String> {
    let mut config = read_config()?;
    let obj = config.as_object_mut().ok_or("配置不是有效的 JSON 对象")?;
    let defaults_value = serde_json::to_value(defaults).map_err(|e| e.to_string())?;

    if let Some(agents) = obj.get_mut("agents") {
        if let Some(agents_obj) = agents.as_object_mut() {
            agents_obj.insert("defaults".to_string(), defaults_value);
        } else {
            let mut agents_map = Map::new();
            agents_map.insert("defaults".to_string(), defaults_value);
            obj.insert("agents".to_string(), Value::Object(agents_map));
        }
    } else {
        let mut agents_map = Map::new();
        agents_map.insert("defaults".to_string(), defaults_value);
        obj.insert("agents".to_string(), Value::Object(agents_map));
    }
    write_config(&config)
}

pub fn get_model_catalog() -> Result<Option<HashMap<String, OpenClawModelCatalogEntry>>, String> {
    Ok(get_agents_defaults()?.models)
}

pub fn set_model_catalog(
    catalog: &HashMap<String, OpenClawModelCatalogEntry>,
) -> Result<(), String> {
    let mut defaults = get_agents_defaults()?;
    defaults.models = Some(catalog.clone());
    set_agents_defaults(&defaults)
}

pub fn get_default_model() -> Result<Option<OpenClawDefaultModel>, String> {
    Ok(get_agents_defaults()?.model)
}

pub fn set_default_model(model: OpenClawDefaultModel) -> Result<(), String> {
    let mut defaults = get_agents_defaults()?;
    defaults.model = Some(model);
    set_agents_defaults(&defaults)
}

pub fn get_live_provider_ids() -> Result<Vec<String>, String> {
    let config = read_config()?;
    Ok(config
        .pointer("/models/providers")
        .and_then(Value::as_object)
        .map(|providers| providers.keys().cloned().collect())
        .unwrap_or_default())
}

pub fn get_live_provider(id: &str) -> Result<Option<Value>, String> {
    let config = read_config()?;
    Ok(config
        .get("models")
        .and_then(Value::as_object)
        .and_then(|models| models.get("providers"))
        .and_then(Value::as_object)
        .and_then(|providers| providers.get(id))
        .cloned())
}

pub fn remove_live_provider(id: &str) -> Result<bool, String> {
    let mut config = read_config()?;
    let Some(models) = config.get_mut("models").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    let Some(providers) = models.get_mut("providers").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    let removed = providers.remove(id).is_some();
    if removed {
        write_config(&config)?;
    }
    Ok(removed)
}

pub fn scan_health() -> Result<Vec<OpenClawHealthWarning>, String> {
    let path = get_openclaw_config_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let config: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            return Ok(vec![OpenClawHealthWarning {
                code: "parse_failed".to_string(),
                message: format!("openclaw.json 解析失败: {e}"),
                path: Some(path.display().to_string()),
            }]);
        }
    };

    let mut warnings = Vec::new();

    if let Some(providers) = config
        .get("models")
        .and_then(|m| m.get("providers"))
        .and_then(|p| p.as_object())
    {
        for (name, provider) in providers {
            if provider.get("baseUrl").and_then(|v| v.as_str()).is_none()
                && provider.get("base_url").and_then(|v| v.as_str()).is_none()
            {
                warnings.push(OpenClawHealthWarning {
                    code: "missing_base_url".to_string(),
                    message: format!("Provider '{name}' 缺少 baseUrl"),
                    path: Some(format!("models.providers.{name}")),
                });
            }
        }
    }

    if let Some(env) = config.get("env").and_then(|e| e.as_object()) {
        for (key, val) in env {
            if val.as_str().map(|s| s.is_empty()).unwrap_or(false) {
                warnings.push(OpenClawHealthWarning {
                    code: "empty_env_var".to_string(),
                    message: format!("环境变量 '{key}' 值为空"),
                    path: Some(format!("env.{key}")),
                });
            }
        }
    }

    Ok(warnings)
}

pub fn get_config_exists() -> bool {
    get_openclaw_config_path().exists()
}

pub fn get_config_path() -> String {
    get_openclaw_config_path().display().to_string()
}
