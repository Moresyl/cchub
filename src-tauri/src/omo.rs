use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

const STANDARD_PLUGIN_PREFIXES: [&str; 2] = ["oh-my-openagent", "oh-my-opencode"];
const SLIM_PLUGIN_PREFIXES: [&str; 1] = ["oh-my-opencode-slim"];

#[derive(Debug, Clone, Copy)]
pub struct OmoVariant {
    pub id: &'static str,
    pub preferred_filename: &'static str,
    pub config_candidates: &'static [&'static str],
    pub plugin_name: &'static str,
    pub plugin_prefixes: &'static [&'static str],
    pub has_categories: bool,
}

pub const STANDARD_VARIANT: OmoVariant = OmoVariant {
    id: "standard",
    preferred_filename: "oh-my-openagent.jsonc",
    config_candidates: &[
        "oh-my-openagent.jsonc",
        "oh-my-openagent.json",
        "oh-my-opencode.jsonc",
        "oh-my-opencode.json",
    ],
    plugin_name: "oh-my-openagent@latest",
    plugin_prefixes: &STANDARD_PLUGIN_PREFIXES,
    has_categories: true,
};

pub const SLIM_VARIANT: OmoVariant = OmoVariant {
    id: "slim",
    preferred_filename: "oh-my-opencode-slim.jsonc",
    config_candidates: &["oh-my-opencode-slim.jsonc", "oh-my-opencode-slim.json"],
    plugin_name: "oh-my-opencode-slim@latest",
    plugin_prefixes: &SLIM_PLUGIN_PREFIXES,
    has_categories: false,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmoLocalConfigData {
    pub variant: String,
    pub file_path: String,
    pub last_modified: Option<String>,
    pub agents: Value,
    pub categories: Option<Value>,
    pub other_fields: Value,
    pub plugin_enabled: bool,
    pub plugins: Vec<String>,
    pub opencode_config_path: String,
}

fn format_system_time(value: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = value.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn variant_from_id(value: &str) -> Result<&'static OmoVariant, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "standard" | "omo" => Ok(&STANDARD_VARIANT),
        "slim" | "omo-slim" => Ok(&SLIM_VARIANT),
        _ => Err(format!("Unsupported OMO variant: {value}")),
    }
}

fn resolve_opencode_config_dir(conn: &Connection) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let custom_dir: Option<String> = conn
        .query_row(
            "SELECT config_dir FROM custom_paths WHERE tool_id = 'opencode'",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(dir) = custom_dir.filter(|dir| !dir.trim().is_empty()) {
        return Ok(PathBuf::from(dir));
    }

    let custom_config_path: Option<String> = conn
        .query_row(
            "SELECT mcp_config_path FROM custom_paths WHERE tool_id = 'opencode'",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(path) = custom_config_path.filter(|path| !path.trim().is_empty()) {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            return Ok(parent.to_path_buf());
        }
    }

    Ok(home.join(".opencode"))
}

fn opencode_config_path(conn: &Connection) -> Result<PathBuf, String> {
    Ok(resolve_opencode_config_dir(conn)?.join("opencode.json"))
}

fn variant_candidates(base_dir: &Path, variant: &OmoVariant) -> Vec<PathBuf> {
    variant
        .config_candidates
        .iter()
        .map(|name| base_dir.join(name))
        .collect()
}

fn find_existing_variant_config(base_dir: &Path, variant: &OmoVariant) -> Option<PathBuf> {
    variant_candidates(base_dir, variant)
        .into_iter()
        .find(|path| path.exists())
}

fn target_variant_config_path(base_dir: &Path, variant: &OmoVariant) -> PathBuf {
    find_existing_variant_config(base_dir, variant)
        .unwrap_or_else(|| base_dir.join(variant.preferred_filename))
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut quote = '\0';
    let mut line_comment = false;
    let mut block_comment = false;

    while index < chars.len() {
        let current = chars[index];
        let next = chars.get(index + 1).copied().unwrap_or('\0');

        if line_comment {
            if current == '\n' {
                line_comment = false;
                output.push('\n');
            }
            index += 1;
            continue;
        }

        if block_comment {
            if current == '*' && next == '/' {
                block_comment = false;
                index += 2;
            } else {
                if current == '\n' {
                    output.push('\n');
                }
                index += 1;
            }
            continue;
        }

        if in_string {
            output.push(current);
            if escape {
                escape = false;
            } else if current == '\\' {
                escape = true;
            } else if current == quote {
                in_string = false;
            }
            index += 1;
            continue;
        }

        if (current == '"' || current == '\'') && !line_comment && !block_comment {
            in_string = true;
            quote = current;
            output.push(current);
            index += 1;
            continue;
        }

        if current == '/' && next == '/' {
            line_comment = true;
            index += 2;
            continue;
        }

        if current == '/' && next == '*' {
            block_comment = true;
            index += 2;
            continue;
        }

        output.push(current);
        index += 1;
    }

    output
}

fn read_jsonc_object(path: &Path) -> Result<Map<String, Value>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let cleaned = strip_jsonc_comments(&content);
    let parsed: Value = serde_json::from_str(&cleaned).map_err(|e| e.to_string())?;
    parsed
        .as_object()
        .cloned()
        .ok_or_else(|| format!("OMO config is not a JSON object: {}", path.display()))
}

fn extract_other_fields(obj: &Map<String, Value>, variant: &OmoVariant) -> Map<String, Value> {
    let mut other = Map::new();
    for (key, value) in obj {
        if key == "agents" {
            continue;
        }
        if variant.has_categories && key == "categories" {
            continue;
        }
        other.insert(key.clone(), value.clone());
    }
    other
}

fn matches_plugin_prefix(plugin_name: &str, prefix: &str) -> bool {
    plugin_name == prefix
        || plugin_name
            .strip_prefix(prefix)
            .map(|suffix| suffix.starts_with('@'))
            .unwrap_or(false)
}

fn matches_any_plugin_prefix(plugin_name: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|prefix| matches_plugin_prefix(plugin_name, prefix))
}

fn canonicalize_plugin_name(plugin_name: &str) -> String {
    if let Some(suffix) = plugin_name.strip_prefix("oh-my-opencode") {
        if suffix.is_empty() || suffix.starts_with('@') {
            return format!("oh-my-openagent{suffix}");
        }
    }
    plugin_name.to_string()
}

fn read_opencode_plugins(path: &Path) -> Result<(Map<String, Value>, Vec<String>), String> {
    if !path.exists() {
        return Ok((Map::new(), Vec::new()));
    }

    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let parsed: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let obj = parsed
        .as_object()
        .cloned()
        .ok_or_else(|| format!("Invalid OpenCode config object: {}", path.display()))?;
    let plugins = obj
        .get("plugin")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|value| value.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok((obj, plugins))
}

fn sync_omo_plugin(opencode_path: &Path, variant: &OmoVariant) -> Result<Vec<String>, String> {
    let (mut config, mut plugins) = read_opencode_plugins(opencode_path)?;
    let normalized_plugin = canonicalize_plugin_name(variant.plugin_name);

    plugins.retain(|plugin| {
        !matches_any_plugin_prefix(plugin, &STANDARD_PLUGIN_PREFIXES)
            && !matches_any_plugin_prefix(plugin, &SLIM_PLUGIN_PREFIXES)
    });
    if !plugins.iter().any(|plugin| plugin == &normalized_plugin) {
        plugins.push(normalized_plugin);
    }

    config.insert(
        "plugin".to_string(),
        Value::Array(plugins.iter().cloned().map(Value::String).collect()),
    );

    if let Some(parent) = opencode_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content =
        serde_json::to_string_pretty(&Value::Object(config)).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(opencode_path, &content).map_err(|e| e.to_string())?;
    Ok(plugins)
}

pub fn read_local_config(
    conn: &Connection,
    variant: &OmoVariant,
) -> Result<OmoLocalConfigData, String> {
    let config_dir = resolve_opencode_config_dir(conn)?;
    let config_path = target_variant_config_path(&config_dir, variant);
    let opencode_path = opencode_config_path(conn)?;
    let (_, plugins) = read_opencode_plugins(&opencode_path)?;
    let plugin_enabled = plugins
        .iter()
        .any(|plugin| matches_any_plugin_prefix(plugin, variant.plugin_prefixes));

    let (agents, categories, other_fields, last_modified) = if config_path.exists() {
        let obj = read_jsonc_object(&config_path)?;
        let last_modified = std::fs::metadata(&config_path)
            .ok()
            .and_then(|value| value.modified().ok())
            .map(format_system_time);
        let agents = obj
            .get("agents")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        let categories = if variant.has_categories {
            Some(
                obj.get("categories")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Map::new())),
            )
        } else {
            None
        };
        let other_fields = Value::Object(extract_other_fields(&obj, variant));
        (agents, categories, other_fields, last_modified)
    } else {
        (
            Value::Object(Map::new()),
            if variant.has_categories {
                Some(Value::Object(Map::new()))
            } else {
                None
            },
            Value::Object(Map::new()),
            None,
        )
    };

    Ok(OmoLocalConfigData {
        variant: variant.id.to_string(),
        file_path: config_path.to_string_lossy().to_string(),
        last_modified,
        agents,
        categories,
        other_fields,
        plugin_enabled,
        plugins,
        opencode_config_path: opencode_path.to_string_lossy().to_string(),
    })
}

pub fn write_local_config(
    conn: &Connection,
    variant: &OmoVariant,
    agents: Value,
    categories: Option<Value>,
    other_fields: Option<Value>,
) -> Result<OmoLocalConfigData, String> {
    if !agents.is_object() {
        return Err("OMO agents must be a JSON object".to_string());
    }
    if variant.has_categories && categories.as_ref().is_some_and(|value| !value.is_object()) {
        return Err("OMO categories must be a JSON object".to_string());
    }
    if other_fields
        .as_ref()
        .is_some_and(|value| !value.is_object())
    {
        return Err("OMO other fields must be a JSON object".to_string());
    }

    let config_dir = resolve_opencode_config_dir(conn)?;
    let config_path = target_variant_config_path(&config_dir, variant);
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut root = Map::new();
    if let Some(Value::Object(obj)) = other_fields {
        for (key, value) in obj {
            root.insert(key, value);
        }
    }
    root.insert("agents".to_string(), agents);
    if variant.has_categories {
        root.insert(
            "categories".to_string(),
            categories.unwrap_or_else(|| Value::Object(Map::new())),
        );
    }

    let content = serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&config_path, &content).map_err(|e| e.to_string())?;

    let opencode_path = opencode_config_path(conn)?;
    let _ = sync_omo_plugin(&opencode_path, variant)?;

    read_local_config(conn, variant)
}
