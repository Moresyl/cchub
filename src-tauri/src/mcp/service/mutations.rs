use super::super::config::McpServerConfig;
use super::*;

fn live_status(origin: &NativeOrigin) -> &'static str {
    if origin.disabled {
        "disabled"
    } else {
        "active"
    }
}

fn shared_spec(
    origin: &NativeOrigin,
    binding: &SourceBinding,
    canonical_path: &std::path::Path,
    container: &str,
    spec: &NativeSpec,
) -> Result<NativeSpec, String> {
    let alias = NativeOrigin::new(
        binding.clone(),
        canonical_path.into(),
        container.into(),
        origin.native_name.clone(),
        spec.clone(),
    )?;
    if alias.connection != origin.connection || alias.disabled != origin.disabled {
        return Err("MCP tools sharing a file interpret this definition differently; their configuration was preserved".into());
    }
    Ok(spec.clone())
}

fn add_projection(
    conn: &Connection,
    state: &mut CatalogState,
    origin: &NativeOrigin,
    tool: &str,
    changes: &mut Vec<crate::mcp::native_entry::Change>,
) -> Result<(), String> {
    let binding = native::configured_binding(conn, tool)?;
    let canonical_path = crate::config_write::target_key(&binding.path)?;
    let container = super::super::native_read::Format::for_tool(tool)?
        .container()
        .to_owned();
    let snapshot = native::read_at(conn, &binding, &canonical_path)?;
    let existing = native::entry(&snapshot, &container, &origin.native_name);
    if canonical_path == origin.canonical_path && container == origin.container {
        shared_spec(origin, &binding, &canonical_path, &container, &origin.spec)?;
        if let Some(actual) = existing {
            if !actual.spec.same(&origin.spec)? {
                return Err("MCP source changed externally; refresh before continuing".into());
            }
        }
        changes.push(native::change(
            binding,
            canonical_path,
            container,
            origin.native_name.clone(),
            Some(origin.spec.clone()),
            &snapshot,
        ));
        return Ok(());
    }
    let projection_index = state.projections.iter().position(|projection| {
        projection.source_id == origin.id && projection.binding.tool == tool
    });
    let physical_links: Vec<_> = state
        .projections
        .iter()
        .filter(|projection| {
            projection.canonical_path == canonical_path
                && projection.container == container
                && projection.native_name == origin.native_name
        })
        .collect();
    if physical_links
        .iter()
        .any(|projection| projection.source_id != origin.id)
        || state.origins.values().any(|source| {
            source.id != origin.id
                && source.canonical_path == canonical_path
                && source.container == container
                && source.native_name == origin.native_name
        })
    {
        return Err(
            "The MCP target belongs to another source; its configuration was preserved".into(),
        );
    }
    for projection in &physical_links {
        native::checked_projection(conn, projection)?;
    }
    if let Some(index) = projection_index {
        let projection = &state.projections[index];
        if projection.canonical_path != canonical_path || projection.container != container {
            return Err(
                "MCP synchronized target location changed; review the previous copy first".into(),
            );
        }
        native::checked_projection(conn, projection)?;
    } else if existing.is_some() && physical_links.is_empty() {
        return Err(
            "The target has an unrelated MCP entry with this name; its configuration was preserved"
                .into(),
        );
    }
    let spec = if let Some(physical) = physical_links.first() {
        // Keep one shared native representation rather than replacing it with
        // adapter defaults (for example an optional enabled=true marker).
        shared_spec(
            origin,
            &binding,
            &canonical_path,
            &container,
            &physical.spec,
        )?
    } else {
        crate::mcp::native_entry::project_connection(origin, tool)?
    };
    changes.push(native::change(
        binding.clone(),
        canonical_path.clone(),
        container.clone(),
        origin.native_name.clone(),
        Some(spec.clone()),
        &snapshot,
    ));
    let projection = Projection {
        source_id: origin.id.clone(),
        binding,
        canonical_path,
        container,
        native_name: origin.native_name.clone(),
        spec,
    };
    if let Some(index) = projection_index {
        state.projections[index] = projection;
    } else {
        state.projections.push(projection);
    }
    Ok(())
}

pub(crate) fn install(
    conn: &Connection,
    name: String,
    config: McpServerConfig,
    targets: Vec<String>,
) -> Result<CatalogServer, String> {
    let spec = crate::mcp::native_entry::patch_connection(None, &name, "claude", &config)?;
    install_batch(conn, "claude", vec![(name, spec)], targets)?
        .pop()
        .ok_or_else(invalid)
}

/// Prepare every source and projection before committing any native file.
/// Imported specifications keep their native extension and policy fields.
pub(crate) fn import_document(
    conn: &Connection,
    source_tool: &str,
    text: &str,
    targets: Vec<String>,
) -> Result<Vec<CatalogServer>, String> {
    let (_, entries) = crate::mcp::native_read::parse_entries(text, source_tool, true)?;
    let entries = entries
        .into_iter()
        .map(|(name, entry)| Ok((name, NativeSpec::from_entry(&entry)?)))
        .collect::<Result<Vec<_>, String>>()?;
    install_batch(conn, source_tool, entries, targets)
}

pub(crate) fn install_batch(
    conn: &Connection,
    source_tool: &str,
    entries: Vec<(String, NativeSpec)>,
    targets: Vec<String>,
) -> Result<Vec<CatalogServer>, String> {
    if entries.is_empty() {
        return Err("MCP import contains no server entries".into());
    }
    let mut state = CatalogState::load(conn)?;
    rows(conn)?;
    let binding = native::configured_binding(conn, source_tool)?;
    let container = super::super::native_read::Format::for_tool(source_tool)?.container();
    let canonical = crate::config_write::target_key(&binding.path)?;
    let snapshot = native::read_at(conn, &binding, &canonical)?;
    let targets: std::collections::BTreeSet<_> = targets.into_iter().collect();
    let mut names = std::collections::BTreeSet::new();
    let mut origins = Vec::new();
    let mut changes = Vec::new();
    for (name, spec) in entries {
        if !names.insert(name.clone()) {
            return Err("MCP import contains duplicate server names".into());
        }
        if native::entry(&snapshot, container, &name).is_some() {
            return Err(
                "This source already has an MCP entry with this name; edit that entry instead"
                    .into(),
            );
        }
        let origin = NativeOrigin::new(
            binding.clone(),
            canonical.clone(),
            container.into(),
            name,
            spec,
        )?;
        if state.origins.contains_key(&origin.id)
            || state.projections.iter().any(|projection| {
                projection.canonical_path == canonical
                    && projection.container == container
                    && projection.native_name == origin.native_name
            })
        {
            return Err(
                "This MCP source already exists in the library; restore and edit it instead".into(),
            );
        }
        changes.push(native::origin_change(
            &origin,
            Some(origin.spec.clone()),
            &snapshot,
        ));
        for tool in &targets {
            if tool != source_tool {
                add_projection(conn, &mut state, &origin, tool, &mut changes)?;
            }
        }
        state.origins.insert(origin.id.clone(), origin.clone());
        origins.push(origin);
    }
    native::commit_with_result(conn, state, changes, |tx| {
        origins
            .iter()
            .map(|origin| {
                native::save_row(tx, origin, live_status(origin))?;
                native::activity(tx, &origin.id, "install")?;
                let server = tx
                    .query_row(
                        &format!("SELECT {COLUMNS} FROM mcp_servers WHERE id=?1"),
                        [&origin.id],
                        row,
                    )
                    .map_err(|_| invalid())?;
                view::decorate(server, Some(origin))
            })
            .collect()
    })
}

pub(crate) fn update(
    conn: &Connection,
    id: &str,
    command: String,
    args: Vec<String>,
    env: std::collections::HashMap<String, String>,
    expected: Option<&str>,
) -> Result<(), String> {
    let mut state = CatalogState::load(conn)?;
    rows(conn)?;
    let old = view::checked(&state, id, expected)?.clone();
    let snapshot = native::checked_origin(conn, &old, false)?;
    let config = McpServerConfig {
        command,
        args,
        env,
        transport_type: Some(old.connection.transport.clone()),
    };
    let spec = crate::mcp::native_entry::patch_connection(
        Some(&old.spec),
        &old.native_name,
        &old.bindings[0].tool,
        &config,
    )?;
    let mut origin = NativeOrigin::new(
        old.bindings[0].clone(),
        old.canonical_path.clone(),
        old.container.clone(),
        old.native_name.clone(),
        spec,
    )?;
    origin.bindings = old.bindings;
    origin.validate()?;
    let mut changes = vec![native::origin_change(
        &origin,
        Some(origin.spec.clone()),
        &snapshot,
    )];
    for index in 0..state.projections.len() {
        if state.projections[index].source_id != id {
            continue;
        }
        let projection = &state.projections[index];
        let snapshot = native::checked_projection(conn, projection)?;
        if native::entry(&snapshot, &projection.container, &projection.native_name).is_none() {
            return Err(
                "A synchronized MCP copy is missing; restore or unlink it before editing".into(),
            );
        }
        let spec = if let Some(shared) = changes
            .iter()
            .find(|change| {
                change.canonical_path == projection.canonical_path
                    && change.container == projection.container
                    && change.name == projection.native_name
            })
            .and_then(|change| change.spec.as_ref())
        {
            shared_spec(
                &origin,
                &projection.binding,
                &projection.canonical_path,
                &projection.container,
                shared,
            )?
        } else {
            crate::mcp::native_entry::project_connection(&origin, &projection.binding.tool)?
        };
        changes.push(native::projection_change(
            projection,
            Some(spec.clone()),
            &snapshot,
        ));
        state.projections[index].spec = spec;
    }
    state.origins.insert(id.into(), origin.clone());
    native::commit(
        conn,
        state,
        changes,
        Some((&origin, live_status(&origin))),
        None,
        (id, "config_update"),
    )
}

pub(crate) fn sync(conn: &Connection, id: &str, tool: &str) -> Result<(), String> {
    let mut state = CatalogState::load(conn)?;
    rows(conn)?;
    let origin = view::checked(&state, id, None)?.clone();
    let source = native::checked_origin(conn, &origin, true)?;
    let mut changes = Vec::new();
    add_projection(conn, &mut state, &origin, tool, &mut changes)?;
    // Keep the source revision guarded even when only a target is changed.
    let mut plan = crate::mcp::native_entry::prepare(&changes)?;
    if !changes
        .iter()
        .any(|change| change.canonical_path == origin.canonical_path)
    {
        plan.plan.guards.push((
            origin.canonical_path.clone(),
            source.documents[0].original.clone(),
        ));
        plan.guard_source(&origin, &source);
    }
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| invalid())?;
    state.save(&tx)?;
    native::activity(&tx, id, &format!("sync_to_{tool}"))?;
    if changes
        .iter()
        .any(|change| change.canonical_path == origin.canonical_path)
    {
        native::save_row(&tx, &origin, live_status(&origin))?;
    }
    plan.commit_then(|| tx.commit().map_err(|_| invalid()))
}

pub(crate) fn unsync(conn: &Connection, id: &str, tool: &str) -> Result<(), String> {
    let mut state = CatalogState::load(conn)?;
    rows(conn)?;
    let origin = view::checked(&state, id, None)?.clone();
    let binding = native::configured_binding(conn, tool)?;
    let canonical = crate::config_write::target_key(&binding.path)?;
    let container = super::super::native_read::Format::for_tool(tool)?.container();
    if canonical == origin.canonical_path && container == origin.container {
        let snapshot = native::checked_origin(conn, &origin, true)?;
        return native::commit(
            conn,
            state,
            vec![native::origin_change(&origin, None, &snapshot)],
            Some((&origin, "missing")),
            None,
            (id, &format!("unsync_from_{tool}")),
        );
    }
    let index = state
        .projections
        .iter()
        .position(|projection| projection.source_id == id && projection.binding.tool == tool)
        .ok_or(
            "This MCP target is not owned by the selected source; its configuration was preserved",
        )?;
    let projection = state.projections.remove(index);
    let snapshot = native::checked_projection(conn, &projection)?;
    let mut removal = native::projection_change(&projection, None, &snapshot);
    for alias in state.projections.iter().filter(|other| {
        other.source_id == id
            && other.canonical_path == projection.canonical_path
            && other.container == projection.container
            && other.native_name == projection.native_name
    }) {
        native::checked_projection(conn, alias)?;
        removal.aliases.push(alias.binding.clone());
    }
    // Removing one physical copy unlinks every tool alias of that copy.
    state.projections.retain(|other| {
        !(other.source_id == id
            && other.canonical_path == projection.canonical_path
            && other.container == projection.container
            && other.native_name == projection.native_name)
    });
    native::commit(
        conn,
        state,
        vec![removal],
        None,
        None,
        (id, &format!("unsync_from_{tool}")),
    )
}

pub(crate) fn uninstall(conn: &Connection, id: &str, expected: Option<&str>) -> Result<(), String> {
    let mut state = CatalogState::load(conn)?;
    rows(conn)?;
    let origin = view::checked(&state, id, expected)?.clone();
    let snapshot = native::checked_origin(conn, &origin, true)?;
    let mut changes = vec![native::origin_change(&origin, None, &snapshot)];
    for projection in state
        .projections
        .iter()
        .filter(|projection| projection.source_id == id)
    {
        let snapshot = native::checked_projection(conn, projection)?;
        changes.push(native::projection_change(projection, None, &snapshot));
    }
    state
        .projections
        .retain(|projection| projection.source_id != id);
    state.origins.remove(id);
    native::commit(conn, state, changes, None, Some(id), (id, "uninstall"))
}

#[cfg(test)]
mod tests;
