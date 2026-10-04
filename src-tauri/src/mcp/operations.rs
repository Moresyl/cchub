//! Callers hold the database mutex first; all native operations then take the
//! shared configuration lock. Never acquire these locks in the opposite order.
use super::{config::McpServerConfig, service};
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};

pub(crate) fn refresh(conn: &Connection) -> Result<Vec<service::CatalogServer>, String> {
    let _guard = crate::json_config::write_lock()?;
    service::prepare_refresh(conn)?.commit(conn)?;
    service::list(conn)
}

pub(crate) fn install(
    conn: &Connection,
    name: String,
    config: McpServerConfig,
    targets: Vec<String>,
) -> Result<service::CatalogServer, String> {
    let _guard = crate::json_config::write_lock()?;
    service::install(conn, name, config, targets)
}

pub(crate) fn update(
    conn: &Connection,
    reference: &str,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    revision: Option<&str>,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    let id = service::resolve_id(conn, reference)?;
    service::update(conn, &id, command, args, env, revision)
}

pub(crate) fn remove(
    conn: &Connection,
    reference: &str,
    revision: Option<&str>,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    let id = service::resolve_id(conn, reference)?;
    service::uninstall(conn, &id, revision)
}

pub(crate) fn toggle(
    conn: &Connection,
    reference: &str,
    tool: &str,
    enabled: bool,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    let id = service::resolve_id(conn, reference)?;
    if enabled {
        service::sync(conn, &id, tool)
    } else {
        service::unsync(conn, &id, tool)
    }
}

pub(crate) fn statuses(
    conn: &Connection,
    ids: &[String],
) -> Result<BTreeMap<String, BTreeMap<String, service::ToolStatus>>, String> {
    let _guard = crate::json_config::write_lock()?;
    service::statuses(conn, ids)
}

pub(crate) fn status(
    conn: &Connection,
    reference: &str,
) -> Result<BTreeMap<String, service::ToolStatus>, String> {
    let _guard = crate::json_config::write_lock()?;
    let id = service::resolve_id(conn, reference)?;
    service::status(conn, &id)
}

pub(crate) fn export(conn: &Connection, reference: &str) -> Result<String, String> {
    let _guard = crate::json_config::write_lock()?;
    let id = service::resolve_id(conn, reference)?;
    service::export(conn, &id)
}

pub(crate) fn import_document(
    conn: &Connection,
    tool: &str,
    text: &str,
    targets: Vec<String>,
) -> Result<Vec<service::CatalogServer>, String> {
    let _guard = crate::json_config::write_lock()?;
    service::import_document(conn, tool, text, targets)
}

pub(crate) fn install_for_tool(
    conn: &Connection,
    tool: &str,
    name: String,
    config: McpServerConfig,
) -> Result<service::CatalogServer, String> {
    let _guard = crate::json_config::write_lock()?;
    service::install_for_tool(conn, tool, name, config, vec![])
}

pub(crate) fn import_targets(
    conn: &Connection,
    format_tool: &str,
    text: &str,
    targets: Vec<String>,
) -> Result<Vec<service::CatalogServer>, String> {
    let _guard = crate::json_config::write_lock()?;
    service::import_targets(conn, format_tool, text, targets)
}

fn scoped_id(
    conn: &Connection,
    tool: &str,
    reference: &str,
    include_copies: bool,
) -> Result<Option<String>, String> {
    use super::sources::SourceRole;
    let path = crate::commands::extra_commands::resolve_tool_mcp_path(conn, tool)?;
    let canonical = crate::config_write::target_key(&path)?;
    let list = service::list(conn)?;
    let state = service::CatalogState::load(conn)?;
    let exact = list.iter().any(|row| row.server.id == reference);
    let mut matches = Vec::new();
    for row in &list {
        if if exact {
            row.server.id != reference
        } else {
            row.server.name != reference
        } {
            continue;
        }
        if let Some(origin) = &row.origin {
            for binding in &origin.bindings {
                if binding.tool == tool
                    && binding.role == SourceRole::Primary
                    && crate::config_write::target_key(&binding.path)? == canonical
                {
                    matches.push(row.server.id.clone());
                    break;
                }
            }
        }
        if include_copies
            && !matches.contains(&row.server.id)
            && state.projections.iter().any(|copy| {
                copy.source_id == row.server.id
                    && copy.binding.tool == tool
                    && copy.canonical_path == canonical
            })
        {
            matches.push(row.server.id.clone());
        }
    }
    if matches.len() > 1 {
        return Err("Multiple MCP entries match this tool; select an explicit source ID".into());
    }
    if exact && matches.is_empty() {
        return Err("MCP source is not bound to the selected tool".into());
    }
    Ok(matches.pop())
}

pub(crate) fn remove_from_tool(
    conn: &Connection,
    tool: &str,
    reference: &str,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    let id = scoped_id(conn, tool, reference, true)?
        .ok_or("MCP source is not bound to the selected tool")?;
    service::unsync(conn, &id, tool)
}

pub(crate) fn upsert_native(
    conn: &Connection,
    tool: &str,
    reference: &str,
    value: serde_json::Value,
) -> Result<crate::db::models::McpServer, String> {
    use super::native_read::{Entry, Format};
    use super::sources::NativeSpec;
    let _guard = crate::json_config::write_lock()?;
    let fields = value
        .as_object()
        .ok_or("MCP specification must be an object")?;
    let entry = match Format::for_tool(tool)? {
        Format::Codex | Format::Grok => Entry::Toml(
            serde_json::from_value(value.clone()).map_err(|_| "Invalid native TOML fields")?,
        ),
        Format::Hermes => Entry::Yaml(
            serde_yaml::from_value(
                serde_yaml::to_value(&value).map_err(|_| "Invalid YAML fields")?,
            )
            .map_err(|_| "Invalid YAML fields")?,
        ),
        _ => Entry::Json(fields.clone()),
    };
    let spec = NativeSpec::from_entry(&entry)?;
    if let Some(id) = scoped_id(conn, tool, reference, false)? {
        service::replace(conn, &id, spec, None)?;
        return service::list(conn)?
            .into_iter()
            .find(|row| row.server.id == id)
            .map(|row| row.server)
            .ok_or("MCP source was not found after update".into());
    }
    service::install_batch(conn, tool, vec![(reference.into(), spec)], vec![])?
        .pop()
        .map(|row| row.server)
        .ok_or("MCP import was empty".into())
}
