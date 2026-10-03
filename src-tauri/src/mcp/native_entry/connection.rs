use crate::mcp::config::McpServerConfig;
use crate::mcp::native_read::{Entry, Format};
use crate::mcp::sources::{NativeOrigin, NativeSpec};

fn document(spec: Option<&NativeSpec>, name: &str, format: Format) -> Result<String, String> {
    let invalid = || "Cannot prepare the native MCP definition".to_owned();
    let Some(spec) = spec else {
        return Ok(if matches!(format, Format::Codex | Format::Grok) {
            ""
        } else {
            "{}\n"
        }
        .into());
    };
    match spec.entry()? {
        Entry::Json(fields) => {
            Ok(serde_json::json!({format.container(): {name: fields}}).to_string())
        }
        Entry::Toml(fields) => {
            let root = ::toml::Table::from_iter([(
                format.container().into(),
                ::toml::Value::Table(::toml::Table::from_iter([(
                    name.into(),
                    ::toml::Value::Table(fields),
                )])),
            )]);
            ::toml::to_string(&root).map_err(|_| invalid())
        }
        Entry::Yaml(fields) => {
            let root = serde_yaml::Mapping::from_iter([(
                serde_yaml::Value::String(format.container().into()),
                serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([(
                    serde_yaml::Value::String(name.into()),
                    serde_yaml::Value::Mapping(fields),
                )])),
            )]);
            serde_yaml::to_string(&root).map_err(|_| invalid())
        }
    }
}

pub(crate) fn patch_connection(
    spec: Option<&NativeSpec>,
    name: &str,
    tool: &str,
    config: &McpServerConfig,
) -> Result<NativeSpec, String> {
    let format = Format::for_tool(tool)?;
    if config.transport_type.as_deref() == Some("sse") && tool == "codex" {
        return Err("This tool does not support the source MCP SSE transport".into());
    }
    let source = document(spec, name, format)?;
    let desired = match format {
        Format::Codex | Format::Grok => crate::mcp::native_toml::edit(
            &source,
            name,
            Some(config),
            if format == Format::Codex {
                crate::mcp::native_toml::Format::Codex
            } else {
                crate::mcp::native_toml::Format::Grok
            },
        )?,
        Format::Hermes => crate::mcp::native_yaml::edit_text(
            &source,
            &[crate::mcp::native_yaml::Edit {
                name,
                config: Some(config),
            }],
        )?,
        _ => crate::mcp::native_json::edit_text(
            &source,
            &[crate::mcp::native_json::Edit {
                name,
                config: Some(config),
                format: match tool {
                    "gemini" => crate::mcp::formats::JsonMcpFormat::Gemini,
                    "opencode" => crate::mcp::formats::JsonMcpFormat::OpenCode,
                    "mcode" => crate::mcp::formats::JsonMcpFormat::MiniMax,
                    _ => crate::mcp::formats::JsonMcpFormat::Standard,
                },
            }],
        )?,
    };
    let (_, entries) = crate::mcp::native_read::parse_entries(&desired, tool, false)?;
    NativeSpec::from_entry(entries.get(name).ok_or("Prepared MCP entry is missing")?)
}

fn remove_connection_fields(entry: &mut Entry) {
    for key in [
        "command",
        "args",
        "env",
        "environment",
        "url",
        "httpUrl",
        "headers",
        "http_headers",
        "type",
        "transport",
    ] {
        match entry {
            Entry::Json(fields) => {
                fields.remove(key);
            }
            Entry::Toml(fields) => {
                fields.remove(key);
            }
            Entry::Yaml(fields) => {
                fields.remove(serde_yaml::Value::String(key.into()));
            }
        }
    }
}

fn contains_datetime(value: &::toml::Value) -> bool {
    match value {
        ::toml::Value::Datetime(_) => true,
        ::toml::Value::Array(values) => values.iter().any(contains_datetime),
        ::toml::Value::Table(values) => values.values().any(contains_datetime),
        _ => false,
    }
}

pub(crate) fn project_connection(origin: &NativeOrigin, tool: &str) -> Result<NativeSpec, String> {
    let format = Format::for_tool(tool)?;
    let mut entry = origin.spec.entry()?;
    if origin.bindings.iter().all(|binding| binding.tool != tool) {
        for key in [
            "env_http_headers",
            "http_headers_helper",
            "bearer_token_env_var",
            "oauth_resource",
            "auth",
            "oauth",
        ] {
            if entry.field(key)?.is_some() {
                return Err("Native MCP authentication requires an explicit target mapping; the target configuration was preserved".into());
            }
        }
    }
    remove_connection_fields(&mut entry);
    let same_format = matches!(
        (&entry, format),
        (
            Entry::Json(_),
            Format::Standard | Format::Gemini | Format::OpenCode
        ) | (Entry::Toml(_), Format::Codex | Format::Grok)
            | (Entry::Yaml(_), Format::Hermes)
    );
    let base = if same_format {
        NativeSpec::from_entry(&entry)?
    } else {
        if let Entry::Toml(fields) = &entry {
            if fields.values().any(contains_datetime) {
                return Err("The native MCP definition contains a date that this target format cannot retain".into());
            }
        }
        // Strict conversion refuses tags, non-finite values and non-string keys.
        let value = entry.to_json()?;
        match format {
            Format::Codex | Format::Grok => NativeSpec::Toml(
                ::toml::to_string(&value)
                    .map_err(|_| "The MCP definition cannot be retained in TOML")?,
            ),
            Format::Hermes => NativeSpec::Yaml(
                serde_yaml::to_string(&value)
                    .map_err(|_| "The MCP definition cannot be retained in YAML")?,
            ),
            _ => NativeSpec::Json(value.to_string()),
        }
    };
    let fields = &origin.connection;
    let config = McpServerConfig {
        command: fields.command.clone(),
        args: fields.args.clone(),
        env: if fields.transport == "stdio" {
            fields.env.clone()
        } else {
            fields.headers.clone()
        }
        .into_iter()
        .collect(),
        transport_type: Some(fields.transport.clone()),
    };
    patch_connection(Some(&base), &origin.native_name, tool, &config)
}

#[cfg(test)]
mod tests;
