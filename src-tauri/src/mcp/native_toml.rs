//! Prepared MCP edits in native TOML; retain unmanaged fields and document trivia.
use super::config::McpServerConfig;
use crate::config_write::{FilePlan, FileUpdate};
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, TableLike};

#[derive(Clone, Copy)]
pub(crate) enum Format {
    Codex,
    Grok,
}

impl Format {
    fn fields(self, config: &McpServerConfig) -> &'static [&'static str] {
        match self {
            // Retain compatible native options, removing the other transport's fields.
            Self::Codex if super::formats::is_remote(config) => &[
                "type",
                "command",
                "args",
                "env",
                "url",
                "http_headers",
                "cwd",
                "env_vars",
            ],
            Self::Codex => &[
                "type",
                "command",
                "args",
                "env",
                "url",
                "http_headers",
                "env_http_headers",
                "http_headers_helper",
                "bearer_token_env_var",
                "oauth_resource",
                "auth",
            ],
            Self::Grok => &["type", "command", "args", "env", "url", "headers"],
        }
    }

    fn server(self, config: &McpServerConfig) -> Table {
        let mut server = super::formats::codex_server_table(config);
        if matches!(self, Self::Grok) {
            server.remove("type");
            if let Some(headers) = server.remove("http_headers") {
                server.insert("headers", headers);
            }
        }
        server
    }
}

fn validate(name: &str, config: Option<&McpServerConfig>) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err("MCP server name must be nonempty and contain no control characters".into());
    }
    let Some(config) = config else { return Ok(()) };
    if config.command.trim().is_empty() || config.command.contains('\0') {
        return Err("MCP command or URL must be nonempty".into());
    }
    if config.transport_type.as_deref().is_some_and(|kind| {
        !matches!(
            kind,
            "stdio" | "local" | "http" | "sse" | "streamable-http" | "remote"
        )
    }) {
        return Err("Unsupported MCP transport".into());
    }
    if super::formats::is_remote(config) {
        let url = url::Url::parse(&config.command).map_err(|_| "Invalid remote MCP URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("Remote MCP URL must use HTTP or HTTPS".into());
        }
    }
    Ok(())
}

fn patch_fields(
    target: &mut dyn TableLike,
    before: &toml::Table,
    desired: &toml::Table,
    generated: &dyn TableLike,
    fields: &[&str],
) {
    for key in fields {
        if before.get(*key) == desired.get(*key) {
            continue;
        }
        let Some(next) = generated.get(key) else {
            target.remove(key);
            continue;
        };
        if let (Some(previous), Some(desired), Some(existing), Some(generated)) = (
            before.get(*key).and_then(toml::Value::as_table),
            desired.get(*key).and_then(toml::Value::as_table),
            target.get_mut(key).and_then(Item::as_table_like_mut),
            next.as_table_like(),
        ) {
            let keys: std::collections::BTreeSet<&str> = previous
                .keys()
                .chain(desired.keys())
                .map(String::as_str)
                .collect();
            patch_fields(
                existing,
                previous,
                desired,
                generated,
                &keys.into_iter().collect::<Vec<_>>(),
            );
            continue;
        }
        let mut next = next.clone();
        if let (Some(next), Some(previous)) = (
            next.as_value_mut(),
            target.get(key).and_then(Item::as_value),
        ) {
            *next.decor_mut() = previous.decor().clone();
        }
        target.insert(key, next);
    }
}

fn same_table(left: &toml::Table, right: &toml::Table) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .all(|(key, value)| right.get(key).is_some_and(|other| same_value(value, other)))
}

fn same_value(left: &toml::Value, right: &toml::Value) -> bool {
    // TOML permits NaN in unrelated settings. IEEE equality would reject an
    // otherwise unchanged document containing it, including exact no-op edits.
    match (left, right) {
        (toml::Value::Float(left), toml::Value::Float(right)) => {
            left == right || (left.is_nan() && right.is_nan())
        }
        (toml::Value::Table(left), toml::Value::Table(right)) => same_table(left, right),
        (toml::Value::Array(left), toml::Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same_value(left, right))
        }
        _ => left == right,
    }
}

fn edit(
    source: &str,
    name: &str,
    config: Option<&McpServerConfig>,
    format: Format,
) -> Result<String, String> {
    let input = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut document: DocumentMut = input
        .parse()
        .map_err(|_| "Invalid native MCP TOML; check syntax before editing")?;
    let mut expected: toml::Table = toml::from_str(input)
        .map_err(|_| "Invalid native MCP TOML; check syntax before editing")?;
    let original = expected.clone();
    if document
        .get("mcp_servers")
        .is_some_and(|value| value.as_table_like().is_none())
    {
        return Err("MCP mcp_servers must be a TOML table".into());
    }
    if document.get("mcp_servers").is_none() {
        if config.is_none() {
            return Ok(source.into());
        }
        document["mcp_servers"] = toml_edit::table();
        expected.insert("mcp_servers".into(), toml::Value::Table(toml::Table::new()));
    }
    let servers = document["mcp_servers"]
        .as_table_like_mut()
        .ok_or("Invalid MCP table")?;
    let expected_servers = expected
        .get_mut("mcp_servers")
        .and_then(toml::Value::as_table_mut)
        .ok_or("Invalid MCP table")?;
    if servers
        .get(name)
        .is_some_and(|entry| entry.as_table_like().is_none())
    {
        return Err("Existing MCP entry must be a TOML table".into());
    }
    if let Some(config) = config {
        let generated = format.server(config);
        // Table Display renders only its body; use a document so nested env and
        // header tables participate in both the patch and semantic verification.
        let desired: toml::Table =
            toml::from_str(&DocumentMut::from(generated.clone()).to_string())
                .map_err(|_| "Cannot prepare native MCP configuration")?;
        if let Some(existing) = servers.get_mut(name) {
            let before = expected_servers
                .get(name)
                .and_then(toml::Value::as_table)
                .ok_or("Invalid MCP entry")?
                .clone();
            let mut fields = format.fields(config).to_vec();
            // HTTP supports local placement, but not stdio's remote executor.
            if matches!(format, Format::Codex)
                && super::formats::is_remote(config)
                && before
                    .get("experimental_environment")
                    .and_then(toml::Value::as_str)
                    == Some("remote")
            {
                fields.push("experimental_environment");
            }
            patch_fields(
                existing.as_table_like_mut().ok_or("Invalid MCP entry")?,
                &before,
                &desired,
                &generated,
                &fields,
            );
            let mut updated = before;
            for field in &fields {
                if let Some(value) = desired.get(*field) {
                    updated.insert((*field).into(), value.clone());
                } else {
                    updated.remove(*field);
                }
            }
            expected_servers.insert(name.into(), toml::Value::Table(updated));
        } else {
            servers.insert(name, Item::Table(generated));
            expected_servers.insert(name.into(), toml::Value::Table(desired));
        }
    } else {
        servers.remove(name);
        expected_servers.remove(name);
    }
    if same_table(&expected, &original) {
        return Ok(source.into());
    }
    let mut output = document.to_string();
    if input.contains("\r\n") && !input.replace("\r\n", "").contains('\n') {
        output = output.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    let actual: toml::Table =
        toml::from_str(&output).map_err(|_| "Cannot serialize native MCP configuration")?;
    if !same_table(&actual, &expected) {
        return Err("Native MCP edit did not match the intended configuration".into());
    }
    Ok(if source.starts_with('\u{feff}') {
        format!("\u{feff}{output}")
    } else {
        output
    })
}

// Preparation reads only. Group callers hold the application write lock through
// FilePlan's byte checks, native replacements and any database finalizer.
pub(crate) fn prepare(
    path: &Path,
    name: &str,
    config: Option<&McpServerConfig>,
    format: Format,
) -> Result<FilePlan, String> {
    validate(name, config)?;
    let original = crate::config_write::read(path)?;
    let source = original
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| "Native MCP configuration must be UTF-8")?
        .unwrap_or("");
    let desired = edit(source, name, config, format)?.into_bytes();
    let mut plan = FilePlan::default();
    if original.as_deref() == Some(&desired) || (original.is_none() && config.is_none()) {
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
    format: Format,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    prepare(path, name, config, format)?.commit()
}

#[cfg(test)]
mod tests;
