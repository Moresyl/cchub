//! Shared identity and snapshot reads for command/renderer migration.
use super::*;
use crate::mcp::sources::SourceSnapshot;

const TOOLS: [&str; 8] = [
    "claude",
    "claude-desktop",
    "codex",
    "gemini",
    "grokbuild",
    "opencode",
    "hermes",
    "mcode",
];

/// Accept an exact source ID first. Legacy name-only callers must resolve one
/// live source, never an arbitrary first match or an unresolved imported row.
pub(crate) fn resolve_id(conn: &Connection, reference: &str) -> Result<String, String> {
    let state = CatalogState::load(conn)?;
    let rows = rows(conn)?;
    if state.origins.contains_key(reference) || state.archived.contains_key(reference) {
        return Ok(reference.into());
    }
    if rows.iter().any(|row| row.id == reference) {
        return Err(
            "MCP source identity is unresolved; refresh and choose an explicit source".into(),
        );
    }
    let mut matches = state
        .origins
        .values()
        .filter(|origin| origin.native_name == reference);
    let first = matches
        .next()
        .ok_or("MCP source was not found; refresh before continuing")?;
    if matches.next().is_some() {
        return Err("Multiple MCP sources share this name; select the source by its ID".into());
    }
    Ok(first.id.clone())
}

struct Scope {
    tool: &'static str,
    canonical: std::path::PathBuf,
    container: &'static str,
    snapshot: SourceSnapshot,
}

/// Read each configured tool once for the entire page, keeping unknown/error
/// distinct from a known missing entry. No database or native file is written.
pub(crate) fn statuses(
    conn: &Connection,
    ids: &[String],
) -> Result<BTreeMap<String, BTreeMap<String, ToolStatus>>, String> {
    let state = CatalogState::load(conn)?;
    let origins = ids
        .iter()
        .filter(|id| !state.archived.contains_key(*id))
        .map(|id| view::checked(&state, id, None))
        .collect::<Result<Vec<_>, _>>()?;
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let scopes = TOOLS
        .into_iter()
        .map(|tool| {
            let binding = native::configured_binding(conn, tool)?;
            let canonical = crate::config_write::target_key(&binding.path)?;
            let container = crate::mcp::native_read::Format::for_tool(tool)?.container();
            let snapshot = native::read_at(conn, &binding, &canonical)?;
            Ok(Scope {
                tool,
                canonical,
                container,
                snapshot,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut result = origins
        .into_iter()
        .map(|origin| {
            let tools = scopes
                .iter()
                .map(|scope| {
                    let actual =
                        native::entry(&scope.snapshot, scope.container, &origin.native_name);
                    let projection = state.projections.iter().find(|projection| {
                        projection.source_id == origin.id && projection.binding.tool == scope.tool
                    });
                    let status = if scope.canonical == origin.canonical_path
                        && scope.container == origin.container
                    {
                        match actual {
                            Some(actual) if actual.spec.same(&origin.spec)? => "source",
                            Some(_) => "conflict",
                            None => "missing",
                        }
                    } else if let Some(projection) = projection {
                        if projection.canonical_path != scope.canonical
                            || projection.container != scope.container
                        {
                            "conflict"
                        } else {
                            match actual {
                                Some(actual) if actual.spec.same(&projection.spec)? => "linked",
                                Some(_) => "conflict",
                                None => "missing",
                            }
                        }
                    } else if actual.is_some() {
                        "unowned"
                    } else {
                        "missing"
                    };
                    Ok((
                        scope.tool.into(),
                        ToolStatus {
                            state: status.into(),
                            disabled: actual.is_some_and(|entry| entry.disabled),
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, String>>()?;
            Ok((origin.id.clone(), tools))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    for id in ids {
        if let Some(source) = state.archived.get(id) {
            let tools = scopes
                .iter()
                .map(|scope| {
                    let actual = native::entry(&scope.snapshot, scope.container, &source.name);
                    (
                        scope.tool.into(),
                        ToolStatus {
                            state: if actual.is_some() {
                                "unowned"
                            } else {
                                "missing"
                            }
                            .into(),
                            disabled: actual.is_some_and(|entry| entry.disabled),
                        },
                    )
                })
                .collect();
            result.insert(id.clone(), tools);
        }
    }
    // Refuse a mixed result if any file/alias changed during the full read.
    for scope in scopes {
        scope.snapshot.verify()?;
    }
    Ok(result)
}

pub(crate) fn status(conn: &Connection, id: &str) -> Result<BTreeMap<String, ToolStatus>, String> {
    statuses(conn, &[id.into()])?.remove(id).ok_or_else(invalid)
}
