//! Prepared Hermes MCP edits. Keep the full YAML stream and unmanaged values.
use super::config::McpServerConfig;
use crate::config_write::{FilePlan, FileUpdate};
use serde_yaml::{Mapping, Value};
use std::path::Path;

const FIELDS: &[&str] = &[
    "command",
    "args",
    "env",
    "url",
    "headers",
    "type",
    "transport",
];

pub(crate) struct Edit<'a> {
    pub name: &'a str,
    pub config: Option<&'a McpServerConfig>,
}

fn key(name: &str) -> Value {
    Value::String(name.into())
}

fn invalid() -> String {
    "Invalid native MCP YAML; check syntax and mapping types before editing".into()
}

fn connection(name: &str, config: &McpServerConfig) -> Result<Mapping, String> {
    if config.command.trim().is_empty() || config.command.contains('\0') {
        return Err("MCP command or URL must be nonempty and contain no null characters".into());
    }
    if config.transport_type.as_deref().is_some_and(|transport| {
        !matches!(
            transport,
            "stdio" | "local" | "http" | "sse" | "remote" | "streamable-http"
        )
    }) {
        return Err("Unsupported MCP transport".into());
    }
    if config.args.iter().any(|arg| arg.contains('\0')) {
        return Err("MCP arguments must not contain null characters".into());
    }
    let mut fields = Mapping::new();
    let remote = super::formats::is_remote(config);
    fields.insert(
        key(if remote { "url" } else { "command" }),
        key(&config.command),
    );
    if remote && config.transport_type.as_deref() == Some("sse") {
        fields.insert(key("transport"), key("sse"));
    }
    if !remote && !config.args.is_empty() {
        fields.insert(
            key("args"),
            Value::Sequence(config.args.iter().map(|arg| key(arg)).collect()),
        );
    }
    if !config.env.is_empty() {
        // Deterministic new fields; existing native field order stays untouched.
        let values: std::collections::BTreeMap<_, _> = config.env.iter().collect();
        let values = values
            .into_iter()
            .map(|(name, value)| (key(name), key(value)))
            .collect();
        fields.insert(
            key(if remote { "headers" } else { "env" }),
            Value::Mapping(values),
        );
    }
    super::native_read::validate_yaml_entry(name, &fields)?;
    Ok(fields)
}

fn apply(root: &mut Mapping, edit: &Edit<'_>) -> Result<(), String> {
    if edit.name.trim().is_empty() || edit.name.chars().any(char::is_control) {
        return Err("MCP server name must be nonempty and contain no control characters".into());
    }
    let desired = edit
        .config
        .map(|config| connection(edit.name, config))
        .transpose()?;
    if root
        .get(key("mcp_servers"))
        .is_some_and(|value| !value.is_mapping())
    {
        return Err("MCP mcp_servers must be a YAML mapping".into());
    }
    if desired.is_none() && !root.contains_key(key("mcp_servers")) {
        return Ok(());
    }
    let servers = root
        .entry(key("mcp_servers"))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let servers = servers.as_mapping_mut().ok_or_else(invalid)?;
    if servers
        .get(key(edit.name))
        .is_some_and(|value| !value.is_mapping())
    {
        return Err("Existing MCP entry must be a YAML mapping".into());
    }
    if let Some(desired) = desired {
        let mut next = servers
            .get(key(edit.name))
            .and_then(Value::as_mapping)
            .cloned()
            .unwrap_or_default();
        for field in FIELDS {
            next.remove(key(field));
        }
        next.extend(desired);
        // Validate known fields without coercing unknown YAML tags, NaN or keys
        // to JSON. Timeout, enabled, auth and tool policies are never generated.
        super::native_read::validate_yaml_entry(edit.name, &next)?;
        servers.insert(key(edit.name), Value::Mapping(next));
    } else {
        servers.remove(key(edit.name));
    }
    Ok(())
}

pub(crate) fn edit_text(source: &str, edits: &[Edit<'_>]) -> Result<String, String> {
    crate::yaml_config::edit_yaml_text(source, |root| {
        for edit in edits {
            apply(root, edit)?;
        }
        Ok(())
    })
}

// Pure preparation joins the caller's application-lock-before-DB operation.
pub(crate) fn prepare(path: &Path, edits: &[Edit<'_>]) -> Result<FilePlan, String> {
    let original = crate::config_write::read(path)?;
    let source = original
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| "Native MCP configuration must be UTF-8")?
        .unwrap_or("{}\n");
    let desired = edit_text(source, edits)?.into_bytes();
    let mut plan = FilePlan::default();
    if original.as_deref() == Some(&desired) || desired == source.as_bytes() {
        plan.guards.push((path.to_owned(), original));
    } else {
        plan.updates.push(FileUpdate {
            path: path.to_owned(),
            original,
            desired,
        });
    }
    Ok(plan)
}

pub(crate) fn update_at(
    path: &Path,
    name: &str,
    config: Option<&McpServerConfig>,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    prepare(path, &[Edit { name, config }])?.commit()
}

#[cfg(test)]
mod tests;
