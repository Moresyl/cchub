use super::*;
use sha2::{Digest, Sha256};

#[derive(Debug, serde::Serialize)]
pub(crate) struct CatalogServer {
    #[serde(flatten)]
    pub server: McpServer,
    pub origin: Option<OriginSummary>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct OriginSummary {
    pub native_name: String,
    pub revision: String,
    pub bindings: Vec<SourceBinding>,
}

pub(super) fn revision(origin: &NativeOrigin) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&origin.spec).map_err(|_| invalid())?)
    ))
}

pub(super) fn checked<'a>(
    state: &'a CatalogState,
    id: &str,
    expected: Option<&str>,
) -> Result<&'a NativeOrigin, String> {
    let origin = state
        .origins
        .get(id)
        .ok_or("MCP source identity is unresolved; scan and choose an explicit source")?;
    if expected.is_some_and(|expected| revision(origin).as_deref() != Ok(expected)) {
        return Err(
            "MCP source changed since this form was opened; refresh before continuing".into(),
        );
    }
    Ok(origin)
}

pub(super) fn decorate(
    server: McpServer,
    origin: Option<&NativeOrigin>,
) -> Result<CatalogServer, String> {
    let origin = origin
        .map(|origin| {
            Ok::<_, String>(OriginSummary {
                native_name: origin.native_name.clone(),
                revision: revision(origin)?,
                bindings: origin.bindings.clone(),
            })
        })
        .transpose()?;
    Ok(CatalogServer { server, origin })
}

pub(crate) fn list(conn: &Connection) -> Result<Vec<CatalogServer>, String> {
    let state = CatalogState::load(conn)?;
    rows(conn)?
        .into_iter()
        .filter(|row| row.status != "removed")
        .map(|server| {
            let origin = state.origins.get(&server.id);
            decorate(server, origin)
        })
        .collect()
}

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct ToolStatus {
    pub state: String,
    pub disabled: bool,
}

pub(crate) fn status(conn: &Connection, id: &str) -> Result<BTreeMap<String, ToolStatus>, String> {
    let state = CatalogState::load(conn)?;
    let origin = checked(&state, id, None)?;
    let mut result = BTreeMap::new();
    for tool in [
        "claude",
        "claude-desktop",
        "codex",
        "gemini",
        "grokbuild",
        "opencode",
        "hermes",
        "mcode",
    ] {
        let binding = native::configured_binding(conn, tool)?;
        let canonical = crate::config_write::target_key(&binding.path)?;
        let container = super::super::native_read::Format::for_tool(tool)?.container();
        let snapshot = native::read_at(conn, &binding, &canonical)?;
        let actual = native::entry(&snapshot, container, &origin.native_name);
        let same_origin = canonical == origin.canonical_path && container == origin.container;
        let projection = state
            .projections
            .iter()
            .find(|projection| projection.source_id == id && projection.binding.tool == tool);
        let status = if same_origin {
            if let Some(actual) = actual {
                if actual.spec.same(&origin.spec)? {
                    "source"
                } else {
                    "conflict"
                }
            } else {
                "missing"
            }
        } else if let Some(projection) = projection {
            if projection.canonical_path != canonical || projection.container != container {
                "conflict"
            } else if let Some(actual) = actual {
                if actual.spec.same(&projection.spec)? {
                    "linked"
                } else {
                    "conflict"
                }
            } else {
                "missing"
            }
        } else if actual.is_some() {
            "unowned"
        } else {
            "missing"
        };
        result.insert(
            tool.into(),
            ToolStatus {
                state: status.into(),
                disabled: actual.is_some_and(|origin| origin.disabled),
            },
        );
    }
    Ok(result)
}

pub(crate) fn export(conn: &Connection, id: &str) -> Result<String, String> {
    let state = CatalogState::load(conn)?;
    let origin = checked(&state, id, None)?;
    native::checked_origin(conn, origin, true)?;
    serde_json::to_string_pretty(&origin.spec.to_json()?).map_err(|_| invalid())
}
