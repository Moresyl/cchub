use super::{definition, Format};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

// Keep native values in their own type. In particular TOML non-finite extension
// values must not turn into JSON null during validation or future editing.
pub(super) enum Entry {
    Json(serde_json::Map<String, Value>),
    Toml(toml::Table),
    Yaml(serde_yaml::Mapping),
}

impl Entry {
    pub(super) fn field(&self, key: &str) -> Result<Option<Value>, String> {
        match self {
            Self::Json(fields) => Ok(fields.get(key).cloned()),
            Self::Toml(fields) => fields
                .get(key)
                .map(|value| {
                    check_finite(value)?;
                    serde_json::to_value(value).map_err(|_| invalid())
                })
                .transpose(),
            Self::Yaml(fields) => fields
                .get(serde_yaml::Value::String(key.into()))
                .map(|value| {
                    check_yaml_json(value)?;
                    serde_json::to_value(value).map_err(|_| invalid())
                })
                .transpose(),
        }
    }

    pub(super) fn to_json(&self) -> Result<Value, String> {
        match self {
            Self::Json(fields) => Ok(Value::Object(fields.clone())),
            Self::Toml(fields) => {
                for value in fields.values() {
                    check_finite(value)?;
                }
                serde_json::to_value(fields).map_err(|_| invalid())
            }
            Self::Yaml(fields) => {
                check_yaml_json(&serde_yaml::Value::Mapping(fields.clone()))?;
                serde_json::to_value(fields).map_err(|_| invalid())
            }
        }
    }
}

fn invalid() -> String {
    "Invalid native MCP configuration; check syntax and field types before continuing".into()
}

fn check_finite(value: &toml::Value) -> Result<(), String> {
    match value {
        toml::Value::Float(value) if !value.is_finite() => {
            return Err("Native MCP values cannot be represented as JSON without loss".into());
        }
        toml::Value::Array(values) => {
            for value in values {
                check_finite(value)?;
            }
        }
        toml::Value::Table(values) => {
            for value in values.values() {
                check_finite(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_yaml_json(value: &serde_yaml::Value) -> Result<(), String> {
    match value {
        serde_yaml::Value::Number(number) if number.as_f64().is_some_and(|n| !n.is_finite()) => {
            return Err("Native MCP values cannot be represented as JSON without loss".into());
        }
        serde_yaml::Value::Sequence(values) => {
            for value in values {
                check_yaml_json(value)?;
            }
        }
        serde_yaml::Value::Mapping(values) => {
            for (key, value) in values {
                if !key.is_string() {
                    return Err("Native MCP mapping keys must be strings for JSON export".into());
                }
                check_yaml_json(value)?;
            }
        }
        serde_yaml::Value::Tagged(_) => {
            return Err("Native MCP YAML tags cannot be represented as JSON without loss".into());
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn read(path: &Path, format: Format) -> Result<BTreeMap<String, Entry>, String> {
    let Some(bytes) = crate::config_write::read(path)? else {
        return Ok(BTreeMap::new());
    };
    let text = std::str::from_utf8(&bytes).map_err(|_| "MCP configuration must use UTF-8")?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let entries = match format {
        Format::Codex | Format::Grok => read_toml(text, format)?,
        Format::Hermes => read_yaml(text, format)?,
        _ => read_json(text, format)?,
    };
    // Validate the entire requested scope before returning any entries. A broken
    // sibling cannot be silently omitted from a supposedly complete scan.
    for (name, entry) in &entries {
        definition::validate(name, entry, format)?;
    }
    Ok(entries)
}

fn read_json(text: &str, format: Format) -> Result<BTreeMap<String, Entry>, String> {
    let document = crate::json_config::parse_json_object(text)?;
    let Some(container) = document.get(format.container()) else {
        return Ok(BTreeMap::new());
    };
    let fields = container.as_object().ok_or_else(invalid)?;
    fields
        .iter()
        .map(|(name, value)| {
            let fields = value.as_object().ok_or_else(invalid)?;
            Ok((name.clone(), Entry::Json(fields.clone())))
        })
        .collect()
}

fn read_toml(text: &str, format: Format) -> Result<BTreeMap<String, Entry>, String> {
    let document: toml::Table = toml::from_str(text).map_err(|_| invalid())?;
    let Some(container) = document.get(format.container()) else {
        return Ok(BTreeMap::new());
    };
    let fields = container.as_table().ok_or_else(invalid)?;
    fields
        .iter()
        .map(|(name, value)| {
            let fields = value.as_table().ok_or_else(invalid)?;
            Ok((name.clone(), Entry::Toml(fields.clone())))
        })
        .collect()
}

fn read_yaml(text: &str, format: Format) -> Result<BTreeMap<String, Entry>, String> {
    let document: serde_yaml::Value = serde_yaml::from_str(text).map_err(|_| invalid())?;
    let root = document.as_mapping().ok_or_else(invalid)?;
    let Some(container) = root.get(serde_yaml::Value::String(format.container().into())) else {
        return Ok(BTreeMap::new());
    };
    let fields = container.as_mapping().ok_or_else(invalid)?;
    fields
        .iter()
        .map(|(name, value)| {
            let name = name.as_str().ok_or_else(invalid)?;
            let fields = value.as_mapping().ok_or_else(invalid)?;
            Ok((name.into(), Entry::Yaml(fields.clone())))
        })
        .collect()
}
