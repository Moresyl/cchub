//! One catalog/projection service. Command activation follows the complete
//! mutation and renderer migration; never switch discovery alone to new IDs.
use super::sources::{NativeOrigin, NativeSpec, SourceBinding};
use crate::db::models::McpServer;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

mod catalog;
mod migration;
mod mutations;
mod native;
mod view;

const STATE_KEY: &str = "mcp_native_catalog_v1";
const COLUMNS: &str = "id,name,package_name,version,transport,command,args,env,status,source,config_path,installed_at,updated_at";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct Projection {
    pub source_id: String,
    pub binding: SourceBinding,
    pub canonical_path: std::path::PathBuf,
    pub container: String,
    pub native_name: String,
    pub spec: NativeSpec,
}

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct CatalogState {
    pub origins: BTreeMap<String, NativeOrigin>,
    pub projections: Vec<Projection>,
}

pub(super) fn invalid() -> String {
    "MCP catalog is unavailable or invalid; restore a valid catalog before continuing".into()
}

impl CatalogState {
    pub(super) fn load(conn: &Connection) -> Result<Self, String> {
        let content: Option<Option<String>> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                [STATE_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| invalid())?;
        let content = match content {
            None => return Ok(Self::default()),
            Some(None) => return Err(invalid()),
            Some(Some(content)) => content,
        };
        let state: Self = serde_json::from_str(&content).map_err(|_| invalid())?;
        state.validate()?;
        Ok(state)
    }

    fn validate(&self) -> Result<(), String> {
        for (id, origin) in &self.origins {
            if id != &origin.id || origin.bindings.is_empty() {
                return Err(invalid());
            }
            origin.validate().map_err(|_| invalid())?;
        }
        let mut targets = BTreeMap::new();
        let mut tools = std::collections::BTreeSet::new();
        for projection in &self.projections {
            if !self.origins.contains_key(&projection.source_id)
                || !projection.canonical_path.is_absolute()
                || !projection.binding.path.is_absolute()
                || projection.binding.role != super::sources::SourceRole::Primary
                || projection.container
                    != super::native_read::Format::for_tool(&projection.binding.tool)?.container()
            {
                return Err(invalid());
            }
            if !tools.insert((&projection.source_id, &projection.binding.tool)) {
                return Err(invalid());
            }
            projection
                .spec
                .validate_format(&projection.binding.tool)
                .map_err(|_| invalid())?;
            super::native_read::validate_entry(
                &projection.native_name,
                &projection.spec.entry().map_err(|_| invalid())?,
                &projection.binding.tool,
            )
            .map_err(|_| invalid())?;
            let previous = targets.insert(
                (
                    &projection.canonical_path,
                    &projection.container,
                    &projection.native_name,
                ),
                projection,
            );
            if let Some(previous) = previous {
                if previous.source_id != projection.source_id
                    || !previous
                        .spec
                        .same(&projection.spec)
                        .map_err(|_| invalid())?
                {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }

    pub(super) fn save(&self, conn: &Connection) -> Result<(), String> {
        self.validate()?;
        let content = serde_json::to_string(self).map_err(|_| invalid())?;
        conn.execute("INSERT INTO app_settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![STATE_KEY,content]).map_err(|_| invalid())?;
        Ok(())
    }
}

pub(super) fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<McpServer> {
    Ok(McpServer {
        id: row.get(0)?,
        name: row.get(1)?,
        package_name: row.get(2)?,
        version: row.get(3)?,
        transport: row.get(4)?,
        command: row.get(5)?,
        args: row.get(6)?,
        env: row.get(7)?,
        status: row.get(8)?,
        source: row.get(9)?,
        config_path: row.get(10)?,
        installed_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

pub(super) fn rows(conn: &Connection) -> Result<Vec<McpServer>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLUMNS} FROM mcp_servers ORDER BY name,id"
        ))
        .map_err(|_| invalid())?;
    let rows = stmt
        .query_map([], row)
        .map_err(|_| invalid())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    // Do not hide malformed catalog JSON or replace it with empty credentials.
    for row in &rows {
        let _: Vec<String> = serde_json::from_str(&row.args).map_err(|_| invalid())?;
        let _: BTreeMap<String, String> = serde_json::from_str(&row.env).map_err(|_| invalid())?;
    }
    Ok(rows)
}

pub(crate) use catalog::prepare_refresh;
pub(crate) use mutations::{install, sync, uninstall, unsync, update};
pub(crate) use view::{export, list, status, CatalogServer, ToolStatus};

#[cfg(test)]
mod tests;
