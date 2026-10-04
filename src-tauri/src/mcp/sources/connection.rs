use super::super::native_read::Entry;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConnectionFields {
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
}

impl ConnectionFields {
    pub(in crate::mcp) fn from_entry(entry: &Entry, tool: &str) -> Result<Self, String> {
        read(entry, tool)
    }
}

fn invalid() -> String {
    "Invalid MCP connection fields; check native syntax and types".into()
}

fn string(entry: &Entry, key: &str) -> Result<Option<String>, String> {
    entry
        .field(key)?
        .map(|value| value.as_str().map(str::to_owned).ok_or_else(invalid))
        .transpose()
}

fn strings(value: Option<Value>) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|value| value.as_str().map(str::to_owned).ok_or_else(invalid))
        .collect()
}

fn map(value: Option<Value>) -> Result<BTreeMap<String, String>, String> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    value
        .as_object()
        .ok_or_else(invalid)?
        .iter()
        .map(|(key, value)| Ok((key.clone(), value.as_str().ok_or_else(invalid)?.into())))
        .collect()
}

pub(super) fn read(entry: &Entry, tool: &str) -> Result<ConnectionFields, String> {
    let remote = string(entry, "url")?.or(string(entry, "httpUrl")?);
    let marker = string(entry, "type")?;
    let transport = if remote.is_none() {
        "stdio"
    } else if (tool == "hermes" && string(entry, "transport")?.as_deref() == Some("sse"))
        || marker.as_deref() == Some("sse")
        || (tool == "gemini" && string(entry, "url")?.is_some())
    {
        "sse"
    } else {
        "http"
    };
    if let Some(url) = remote {
        return Ok(ConnectionFields {
            transport: transport.into(),
            command: url,
            args: Vec::new(),
            env: BTreeMap::new(),
            headers: map(entry.field(if tool == "codex" {
                "http_headers"
            } else {
                "headers"
            })?)?,
        });
    }
    let (command, args) = if tool == "opencode" {
        let mut command = strings(entry.field("command")?)?.into_iter();
        (command.next().ok_or_else(invalid)?, command.collect())
    } else {
        (
            string(entry, "command")?.ok_or_else(invalid)?,
            strings(entry.field("args")?)?,
        )
    };
    Ok(ConnectionFields {
        transport: transport.into(),
        command,
        args,
        env: map(entry.field(if tool == "opencode" {
            "environment"
        } else {
            "env"
        })?)?,
        headers: BTreeMap::new(),
    })
}
