use super::*;

pub(super) fn matches(row: &McpServer, origin: &NativeOrigin) -> bool {
    if row.name != origin.native_name
        || row.command.as_deref() != Some(&origin.connection.command)
        || row.transport != origin.connection.transport
    {
        return false;
    }
    let Some(path) = row.config_path.as_deref() else {
        return false;
    };
    if !std::path::Path::new(path).is_absolute() {
        return false;
    }
    if crate::config_write::target_key(std::path::Path::new(path))
        .ok()
        .as_ref()
        != Some(&origin.canonical_path)
    {
        return false;
    }
    let args: Result<Vec<String>, _> = serde_json::from_str(&row.args);
    let env: Result<BTreeMap<String, String>, _> = serde_json::from_str(&row.env);
    args.as_ref().ok() == Some(&origin.connection.args)
        && env.as_ref().ok()
            == Some(if origin.connection.transport == "stdio" {
                &origin.connection.env
            } else {
                &origin.connection.headers
            })
        && origin.bindings.iter().any(|binding| {
            binding.tool == row.source
                || (binding.role == super::super::sources::SourceRole::Plugin
                    && matches!(row.source.as_str(), "official-plugin" | "community-plugin")
                    && binding.tool == "claude")
                || matches!(
                    row.source.as_str(),
                    "local" | "import" | "marketplace" | "deeplink"
                ) && binding.tool == "claude"
        })
}

pub(super) fn transfer(conn: &Connection, old: &str, new: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE mcp_servers SET package_name=COALESCE(package_name,(SELECT package_name FROM mcp_servers WHERE id=?1)),version=COALESCE(version,(SELECT version FROM mcp_servers WHERE id=?1)),installed_at=COALESCE((SELECT installed_at FROM mcp_servers WHERE id=?1),installed_at) WHERE id=?2",
        params![old,new],
    ).map_err(|_| invalid())?;
    for table in ["metrics", "activity_logs"] {
        conn.execute(
            &format!("UPDATE {table} SET server_id=?1 WHERE server_id=?2"),
            params![new, old],
        )
        .map_err(|_| invalid())?;
    }
    conn.execute("UPDATE update_history SET item_id=?1 WHERE item_id=?2 AND item_type IN ('mcp','mcp_server')", params![new,old]).map_err(|_| invalid())?;
    let clients = {
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
        clients
    };
    for (id, content) in clients {
        let mut access: BTreeMap<String, bool> =
            serde_json::from_str(&content).map_err(|_| invalid())?;
        if let Some(value) = access.remove(old) {
            // A conflicting access grant cannot be silently changed by migration.
            if access.get(new).is_some_and(|current| *current != value) {
                return Err("MCP client selections conflict during source migration".into());
            }
            access.insert(new.into(), value);
            conn.execute(
                "UPDATE mcp_clients SET server_access=?1 WHERE id=?2",
                params![serde_json::to_string(&access).map_err(|_| invalid())?, id],
            )
            .map_err(|_| invalid())?;
        }
    }
    conn.execute("DELETE FROM mcp_servers WHERE id=?1", [old])
        .map_err(|_| invalid())?;
    Ok(())
}
