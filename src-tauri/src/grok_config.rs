//! Minimal Grok Build config adapter for provider profiles.
//!
//! Grok Build stores an ordinary TOML document in `~/.grok/config.toml`.
//! Profiles use the same JSON snapshot envelope as the other tools so they
//! can be edited and backed up without exposing credentials in log output.

use serde_json::Value;
use std::path::PathBuf;

const DEFAULT_MODEL: &str = "grok-4.5";
const DEFAULT_BASE_URL: &str = "https://api.x.ai/v1";

pub fn get_grok_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".grok")
}

pub fn get_grok_config_path() -> PathBuf {
    get_grok_config_dir().join("config.toml")
}

pub fn read_snapshot() -> Result<String, String> {
    let path = get_grok_config_path();
    if !path.exists() {
        return Err(format!("Config file not found: {}", path.display()));
    }
    let config = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    config
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("Invalid Grok Build config: {error}"))?;
    serde_json::to_string_pretty(&serde_json::json!({ "config": config }))
        .map_err(|error| error.to_string())
}

pub fn apply_snapshot(snapshot: &str) -> Result<(), String> {
    let config = snapshot_to_toml(snapshot)?;
    let path = get_grok_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &config).map_err(|error| error.to_string())
}

fn snapshot_to_toml(snapshot: &str) -> Result<String, String> {
    let value = serde_json::from_str::<Value>(snapshot).map_err(|error| error.to_string())?;
    if let Some(config) = value.get("config").and_then(Value::as_str) {
        config
            .parse::<toml_edit::DocumentMut>()
            .map_err(|error| format!("Invalid Grok Build config: {error}"))?;
        return Ok(config.to_string());
    }

    let base_url = value
        .get("baseUrl")
        .or_else(|| value.get("base_url"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_BASE_URL);
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_MODEL);
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Grok");
    let api_key = value
        .get("apiKey")
        .or_else(|| value.get("api_key"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut document = toml_edit::DocumentMut::new();
    document["models"]["default"] = toml_edit::value(model);
    document["model"][model]["model"] = toml_edit::value(model);
    document["model"][model]["base_url"] = toml_edit::value(base_url);
    document["model"][model]["name"] = toml_edit::value(name);
    document["model"][model]["api_backend"] = toml_edit::value("responses");
    document["model"][model]["context_window"] = toml_edit::value(500_000_i64);
    if !api_key.trim().is_empty() {
        document["model"][model]["api_key"] = toml_edit::value(api_key);
    }
    Ok(document.to_string())
}

#[cfg(test)]
mod tests {
    use super::snapshot_to_toml;

    #[test]
    fn preserves_valid_toml_snapshots() {
        let config = "[models]\ndefault = \"grok-4.5\"\n";
        assert_eq!(
            snapshot_to_toml(&format!(r#"{{"config":{config:?}}}"#)).unwrap(),
            config
        );
    }

    #[test]
    fn builds_a_valid_toml_snapshot_from_provider_fields() {
        let config = snapshot_to_toml(
            r#"{"baseUrl":"https://example.com/v1","model":"demo","apiKey":"secret"}"#,
        )
        .unwrap();
        assert!(config.contains("default = \"demo\""));
        assert!(config.contains("base_url = \"https://example.com/v1\""));
        assert!(config.parse::<toml_edit::DocumentMut>().is_ok());
    }
}
