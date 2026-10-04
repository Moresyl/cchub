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
    spec_revision(&origin.spec)
}

pub(super) fn spec_revision(spec: &NativeSpec) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(spec).map_err(|_| invalid())?)
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
            if let Some(source) = state.archived.get(&server.id) {
                return Ok(CatalogServer {
                    server,
                    origin: Some(OriginSummary {
                        native_name: source.name.clone(),
                        revision: spec_revision(&source.spec)?,
                        bindings: vec![],
                    }),
                });
            }
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

pub(crate) fn export(conn: &Connection, id: &str) -> Result<String, String> {
    let state = CatalogState::load(conn)?;
    if let Some(source) = state.archived.get(id) {
        return serde_json::to_string_pretty(&source.spec.to_json()?).map_err(|_| invalid());
    }
    let origin = checked(&state, id, None)?;
    native::checked_origin(conn, origin, true)?;
    serde_json::to_string_pretty(&origin.spec.to_json()?).map_err(|_| invalid())
}
