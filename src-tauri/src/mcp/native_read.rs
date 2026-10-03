//! Strict, tool-scoped native MCP reads. Never infer an entry from another tool.
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

mod definition;
mod document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Format {
    Standard,
    Gemini,
    OpenCode,
    Codex,
    Grok,
    Hermes,
}

impl Format {
    pub(super) fn for_tool(tool: &str) -> Result<Self, String> {
        match tool {
            "claude" | "claude-desktop" | "mcode" => Ok(Self::Standard),
            "gemini" => Ok(Self::Gemini),
            "opencode" => Ok(Self::OpenCode),
            "codex" => Ok(Self::Codex),
            "grokbuild" => Ok(Self::Grok),
            "hermes" => Ok(Self::Hermes),
            _ => Err("MCP configuration is not supported for this tool".into()),
        }
    }

    pub(super) fn container(self) -> &'static str {
        match self {
            Self::OpenCode => "mcp",
            Self::Codex | Self::Grok | Self::Hermes => "mcp_servers",
            _ => "mcpServers",
        }
    }
}

pub(super) use document::Entry;

pub(super) fn validate_entry(name: &str, entry: &Entry, tool: &str) -> Result<(), String> {
    definition::validate(name, entry, Format::for_tool(tool)?)
}

pub(super) fn parse_entries(
    text: &str,
    tool: &str,
    plugin: bool,
) -> Result<(String, std::collections::BTreeMap<String, Entry>), String> {
    document::parse(text, Format::for_tool(tool)?, plugin)
}

#[derive(Debug)]
pub(crate) struct ConfigView {
    pub config_path: String,
    pub servers: HashMap<String, Value>,
}

pub(crate) fn read_config(conn: &Connection, tool: &str) -> Result<ConfigView, String> {
    let tool = tool.to_ascii_lowercase();
    let format = Format::for_tool(&tool)?;
    let path = crate::commands::extra_commands::resolve_tool_mcp_path(conn, &tool)?;
    read_config_at(&path, format)
}

fn read_config_at(path: &Path, format: Format) -> Result<ConfigView, String> {
    let entries = document::read(path, format)?;
    let servers = entries
        .into_iter()
        .map(|(name, entry)| Ok((name, entry.to_json()?)))
        .collect::<Result<HashMap<_, _>, String>>()?;
    Ok(ConfigView {
        config_path: path.to_string_lossy().into_owned(),
        servers,
    })
}

#[cfg(test)]
mod tests;

pub(super) fn validate_json_entry(name: &str, value: &Value, tool: &str) -> Result<(), String> {
    let fields = value.as_object().ok_or("MCP entry must be a JSON object")?;
    definition::validate(
        name,
        &document::Entry::Json(fields.clone()),
        Format::for_tool(tool)?,
    )
}

pub(super) fn validate_yaml_entry(name: &str, fields: &serde_yaml::Mapping) -> Result<(), String> {
    definition::validate(name, &document::Entry::Yaml(fields.clone()), Format::Hermes)
}
