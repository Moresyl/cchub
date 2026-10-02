use super::{document::Entry, Format};
use serde_json::Value;

fn invalid() -> String {
    "Invalid MCP connection fields; check command, transport, arguments and environment".into()
}

fn string(entry: &Entry, key: &str) -> Result<Option<String>, String> {
    entry
        .field(key)?
        .map(|value| value.as_str().map(str::to_owned).ok_or_else(invalid))
        .transpose()
}

fn string_array(value: &Value) -> Result<(), String> {
    let items = value.as_array().ok_or_else(invalid)?;
    if items.iter().any(|item| !item.is_string()) {
        return Err(invalid());
    }
    Ok(())
}

fn string_map(value: &Value) -> Result<(), String> {
    let fields = value.as_object().ok_or_else(invalid)?;
    if fields.values().any(|value| !value.is_string()) {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn validate(name: &str, entry: &Entry, format: Format) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err("MCP server names must be nonempty and contain no control characters".into());
    }
    for key in ["enabled", "disabled"] {
        if entry.field(key)?.is_some_and(|value| !value.is_boolean()) {
            return Err("MCP enabled and disabled flags must be booleans".into());
        }
    }
    if let Some(args) = entry.field("args")? {
        string_array(&args)?;
    }
    for key in [
        "env",
        "environment",
        "headers",
        "http_headers",
        "env_http_headers",
    ] {
        if let Some(value) = entry.field(key)? {
            string_map(&value)?;
        }
    }

    let command = entry.field("command")?;
    let command_present = match &command {
        Some(Value::String(value)) => {
            if value.trim().is_empty() || value.contains('\0') {
                return Err(invalid());
            }
            true
        }
        Some(value) if format == Format::OpenCode => {
            string_array(value)?;
            let items = value.as_array().ok_or_else(invalid)?;
            if items.is_empty()
                || items[0]
                    .as_str()
                    .is_none_or(|value| value.trim().is_empty() || value.contains('\0'))
            {
                return Err(invalid());
            }
            true
        }
        Some(_) => return Err(invalid()),
        None => false,
    };
    if format == Format::OpenCode && command_present && !command.as_ref().unwrap().is_array() {
        return Err("OpenCode local MCP command must be a nonempty string array".into());
    }
    let url = string(entry, "url")?;
    let http_url = string(entry, "httpUrl")?;
    if http_url.is_some() && format != Format::Gemini {
        return Err("httpUrl is only supported by the Gemini MCP format".into());
    }
    if url.is_some() && http_url.is_some() {
        return Err("MCP connection must select one URL transport".into());
    }
    let remote = url.as_deref().or(http_url.as_deref());
    if command_present == remote.is_some() {
        return Err("MCP connection must specify either a command or a remote URL".into());
    }
    if let Some(remote) = remote {
        let url = url::Url::parse(remote).map_err(|_| "Invalid remote MCP URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("Remote MCP URL must use HTTP or HTTPS and include a host".into());
        }
    }
    if let Some(transport) = string(entry, "type")? {
        let remote_type = match transport.as_str() {
            "stdio" | "local" => false,
            "http" | "sse" | "streamable-http" | "remote" => true,
            _ => return Err("Unsupported MCP transport".into()),
        };
        if remote_type != remote.is_some() {
            return Err("MCP transport does not match its command or URL".into());
        }
        if format == Format::Gemini
            && ((http_url.is_some() && transport == "sse")
                || (url.is_some() && transport == "http"))
        {
            return Err("Gemini MCP transport does not match url or httpUrl".into());
        }
    }
    Ok(())
}
