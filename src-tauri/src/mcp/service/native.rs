use super::super::sources::{SourceRole, SourceSnapshot};
use super::*;

pub(super) fn configured_binding(conn: &Connection, tool: &str) -> Result<SourceBinding, String> {
    super::super::native_read::Format::for_tool(tool)?;
    Ok(SourceBinding {
        tool: tool.into(),
        path: crate::commands::extra_commands::resolve_tool_mcp_path(conn, tool)?,
        role: SourceRole::Primary,
    })
}

pub(super) fn read_at(
    conn: &Connection,
    binding: &SourceBinding,
    canonical: &std::path::Path,
) -> Result<SourceSnapshot, String> {
    let configured = match binding.role {
        SourceRole::Primary => {
            crate::commands::extra_commands::resolve_tool_mcp_path(conn, &binding.tool)?
        }
        SourceRole::Secondary if binding.tool == "claude" => {
            crate::commands::extra_commands::resolve_tool_config_path(conn, "claude")?
        }
        SourceRole::Plugin if binding.tool == "claude" => {
            let root = crate::commands::extra_commands::resolve_tool_config_dir(conn, "claude")?
                .join("plugins");
            if !binding.path.starts_with(&root) {
                return Err("MCP plugin scope changed; refresh before continuing".into());
            }
            binding.path.clone()
        }
        _ => return Err("Invalid MCP source scope".into()),
    };
    if crate::config_write::target_key(&configured)? != canonical
        || crate::config_write::target_key(&binding.path)? != canonical
    {
        return Err("MCP source location changed; refresh before continuing".into());
    }
    SourceSnapshot::read_bindings(std::slice::from_ref(binding))
}

pub(super) fn entry<'a>(
    snapshot: &'a SourceSnapshot,
    container: &str,
    name: &str,
) -> Option<&'a NativeOrigin> {
    snapshot
        .origins
        .iter()
        .find(|origin| origin.container == container && origin.native_name == name)
}

pub(super) fn checked_origin(
    conn: &Connection,
    origin: &NativeOrigin,
    absent: bool,
) -> Result<SourceSnapshot, String> {
    origin.validate()?;
    let first = &origin.bindings[0];
    let snapshot = read_at(conn, first, &origin.canonical_path)?;
    for binding in &origin.bindings[1..] {
        read_at(conn, binding, &origin.canonical_path)?;
    }
    match entry(&snapshot, &origin.container, &origin.native_name) {
        Some(actual) if actual.spec.same(&origin.spec)? => Ok(snapshot),
        None if absent => Ok(snapshot),
        None => Err("MCP source is missing; restore it before editing".into()),
        Some(_) => Err("MCP source changed externally; refresh before continuing".into()),
    }
}

pub(super) fn checked_projection(
    conn: &Connection,
    projection: &Projection,
) -> Result<SourceSnapshot, String> {
    let snapshot = read_at(conn, &projection.binding, &projection.canonical_path)?;
    if let Some(actual) = entry(&snapshot, &projection.container, &projection.native_name) {
        if !actual.spec.same(&projection.spec)? {
            return Err(
                "MCP synchronized copy changed externally; review it before continuing".into(),
            );
        }
    }
    Ok(snapshot)
}

pub(super) fn change(
    binding: SourceBinding,
    canonical_path: std::path::PathBuf,
    container: String,
    name: String,
    spec: Option<NativeSpec>,
    snapshot: &SourceSnapshot,
) -> crate::mcp::native_entry::Change {
    crate::mcp::native_entry::Change {
        binding,
        canonical_path,
        container,
        name,
        spec,
        original: snapshot.documents[0].original.clone(),
        revision: snapshot.documents[0].revision.clone(),
        aliases: snapshot.documents[0].bindings.clone(),
    }
}

pub(super) fn origin_change(
    origin: &NativeOrigin,
    spec: Option<NativeSpec>,
    snapshot: &SourceSnapshot,
) -> crate::mcp::native_entry::Change {
    let mut prepared = change(
        origin.bindings[0].clone(),
        origin.canonical_path.clone(),
        origin.container.clone(),
        origin.native_name.clone(),
        spec,
        snapshot,
    );
    prepared.aliases = origin.bindings.clone();
    prepared
}

pub(super) fn projection_change(
    projection: &Projection,
    spec: Option<NativeSpec>,
    snapshot: &SourceSnapshot,
) -> crate::mcp::native_entry::Change {
    change(
        projection.binding.clone(),
        projection.canonical_path.clone(),
        projection.container.clone(),
        projection.native_name.clone(),
        spec,
        snapshot,
    )
}

pub(super) fn save_row(
    conn: &Connection,
    origin: &NativeOrigin,
    status: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let args = serde_json::to_string(&origin.connection.args).map_err(|_| invalid())?;
    let env = serde_json::to_string(if origin.connection.transport == "stdio" {
        &origin.connection.env
    } else {
        &origin.connection.headers
    })
    .map_err(|_| invalid())?;
    conn.execute("INSERT INTO mcp_servers(id,name,transport,command,args,env,status,source,config_path,installed_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10) ON CONFLICT(id) DO UPDATE SET name=excluded.name,transport=excluded.transport,command=excluded.command,args=excluded.args,env=excluded.env,status=excluded.status,source=excluded.source,config_path=excluded.config_path,updated_at=excluded.updated_at", params![origin.id,origin.native_name,origin.connection.transport,origin.connection.command,args,env,status,origin.bindings[0].tool,origin.bindings[0].path.to_str().ok_or_else(invalid)?,now]).map_err(|_| invalid())?;
    Ok(())
}

pub(super) fn commit(
    conn: &Connection,
    state: CatalogState,
    changes: Vec<crate::mcp::native_entry::Change>,
    origin: Option<(&NativeOrigin, &str)>,
    removed: Option<&str>,
    event: (&str, &str),
) -> Result<(), String> {
    commit_with_result(conn, state, changes, |tx| {
        if let Some((origin, status)) = origin {
            save_row(tx, origin, status)?;
        }
        if let Some(id) = removed {
            revoke_access(tx, id)?;
            tx.execute("UPDATE mcp_servers SET status='removed' WHERE id=?1", [id])
                .map_err(|_| invalid())?;
        }
        activity(tx, event.0, event.1)?;
        Ok(())
    })
}

pub(super) fn activity(conn: &Connection, id: &str, event: &str) -> Result<(), String> {
    conn.execute("INSERT INTO activity_logs(server_id,request_type,status,recorded_at) VALUES(?1,?2,'success',?3)", params![id,event,chrono::Utc::now().to_rfc3339()]).map_err(|_| invalid())?;
    Ok(())
}

fn revoke_access(conn: &Connection, id: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id,server_access FROM mcp_clients")
        .map_err(|_| invalid())?;
    let clients = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| invalid())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    for (client, access) in clients {
        let mut access: BTreeMap<String, bool> =
            serde_json::from_str(&access).map_err(|_| invalid())?;
        if access.remove(id).is_some() {
            conn.execute(
                "UPDATE mcp_clients SET server_access=?1 WHERE id=?2",
                params![
                    serde_json::to_string(&access).map_err(|_| invalid())?,
                    client
                ],
            )
            .map_err(|_| invalid())?;
        }
    }
    Ok(())
}

pub(super) fn commit_with_result<T>(
    conn: &Connection,
    state: CatalogState,
    changes: Vec<crate::mcp::native_entry::Change>,
    catalog: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let plan = crate::mcp::native_entry::prepare(&changes)?;
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| invalid())?;
    let result = catalog(&tx)?;
    state.save(&tx)?;
    plan.commit_then(|| tx.commit().map_err(|_| invalid()))?;
    Ok(result)
}
