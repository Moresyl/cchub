//! Backup definitions are portable; filesystem ownership and client grants are not.
use super::*;
use rusqlite::{params_from_iter, types::Value};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct ArchivedSource {
    pub name: String,
    pub tool: String,
    pub spec: NativeSpec,
}

impl ArchivedSource {
    pub(super) fn validate(&self) -> Result<(), String> {
        self.spec.validate_format(&self.tool)?;
        crate::mcp::native_read::validate_entry(&self.name, &self.spec.entry()?, &self.tool)
    }
}

pub(crate) struct BackupLibrary(CatalogState);

pub(crate) fn prepare_backup_library(conn: &Connection) -> Result<BackupLibrary, String> {
    Ok(BackupLibrary(CatalogState::load(conn)?))
}

// Identifiers/column lists below are compile-time constants, never backup SQL.
fn copy_rows(
    live: &Connection,
    prepared: &Connection,
    table: &str,
    columns: &str,
    replace: bool,
    predicate: &str,
) -> Result<usize, String> {
    let mut stmt = live
        .prepare(&format!("SELECT {columns} FROM {table} {predicate}"))
        .map_err(|_| invalid())?;
    let width = stmt.column_count();
    let rows = stmt
        .query_map([], |row| {
            (0..width)
                .map(|column| row.get::<_, Value>(column))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|_| invalid())?;
    let parameters = (1..=width)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let query = if replace {
        format!("INSERT OR REPLACE INTO {table} ({columns}) VALUES ({parameters})")
    } else {
        let equality = columns
            .split(',')
            .enumerate()
            .map(|(index, column)| format!("{column} IS ?{}", index + 1))
            .collect::<Vec<_>>()
            .join(" AND ");
        format!("INSERT INTO {table} ({columns}) SELECT {parameters} WHERE NOT EXISTS (SELECT 1 FROM {table} WHERE {equality})")
    };
    let mut count = 0;
    for row in rows {
        count += prepared
            .execute(&query, params_from_iter(row.map_err(|_| invalid())?))
            .map_err(|_| invalid())?;
    }
    Ok(count)
}

/// Merge definitions/history into the staged backup without writing native files.
/// Imported sources become explicit, restorable library entries with no paths.
pub(crate) fn restore_backup_library(
    live: &Connection,
    prepared: &Connection,
    imported: BackupLibrary,
) -> Result<usize, String> {
    let mut state = CatalogState::load(live)?;
    let imported = imported.0;
    let tx =
        rusqlite::Transaction::new_unchecked(prepared, rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| invalid())?;
    for (id, source) in imported.archived {
        state.archived.entry(id).or_insert(source);
    }
    for (old_id, origin) in imported.origins {
        // A same-device snapshot can retain an already verified local identity.
        // Different definitions are retained separately rather than discarded.
        if state
            .origins
            .get(&old_id)
            .is_some_and(|local| local.spec == origin.spec)
        {
            continue;
        }
        let source = ArchivedSource {
            name: origin.native_name,
            tool: origin.bindings[0].tool.clone(),
            spec: origin.spec,
        };
        source.validate()?;
        let id = format!(
            "mcp-archive-{:x}",
            Sha256::digest(serde_json::to_vec(&(&old_id, &source)).map_err(|_| invalid())?)
        );
        save_archived_row(&tx, &id, &source)?;
        migration::transfer(&tx, &old_id, &id)?;
        state.archived.insert(id, source);
    }
    for (id, source) in &state.archived {
        save_archived_row(&tx, id, source)?;
    }
    let mut preserved = copy_rows(live, &tx, "mcp_servers", COLUMNS, true, "")?;
    // Local client paths/grants must remain paired with local source identities.
    tx.execute("DELETE FROM mcp_clients", [])
        .map_err(|_| invalid())?;
    preserved += copy_rows(
        live,
        &tx,
        "mcp_clients",
        "id,name,config_path,server_access,created_at",
        true,
        "",
    )?;
    for (table, columns, predicate) in [
        (
            "metrics",
            "server_id,request_count,error_count,avg_latency_ms,recorded_at",
            "",
        ),
        (
            "activity_logs",
            "server_id,request_type,status,latency_ms,recorded_at",
            "",
        ),
        (
            "update_history",
            "item_type,item_id,old_version,new_version,status,updated_at",
            "WHERE item_type IN ('mcp','mcp_server')",
        ),
    ] {
        preserved += copy_rows(live, &tx, table, columns, false, predicate)?;
    }
    // Only projections from the live device retain ownership. Never accept
    // imported paths or projection claims just because bytes happen to match.
    let had_catalog: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key=?1)",
            [STATE_KEY],
            |row| row.get(0),
        )
        .map_err(|_| invalid())?;
    state.save(&tx)?;
    if !had_catalog {
        preserved += 1;
    }
    tx.commit().map_err(|_| invalid())?;
    Ok(preserved)
}

pub(super) fn save_archived_row(
    conn: &Connection,
    id: &str,
    source: &ArchivedSource,
) -> Result<(), String> {
    let connection =
        crate::mcp::sources::ConnectionFields::from_entry(&source.spec.entry()?, &source.tool)?;
    let env = if connection.transport == "stdio" {
        &connection.env
    } else {
        &connection.headers
    };
    conn.execute("INSERT INTO mcp_servers(id,name,transport,command,args,env,status,source,config_path) VALUES(?1,?2,?3,?4,?5,?6,'archived',?7,NULL) ON CONFLICT(id) DO UPDATE SET name=excluded.name,transport=excluded.transport,command=excluded.command,args=excluded.args,env=excluded.env,status='archived',source=excluded.source,config_path=NULL", params![id,source.name,connection.transport,connection.command,serde_json::to_string(&connection.args).map_err(|_| invalid())?,serde_json::to_string(env).map_err(|_| invalid())?,source.tool]).map_err(|_| invalid())?;
    Ok(())
}

pub(super) fn restore(conn: &Connection, id: &str, tool: &str) -> Result<(), String> {
    let state = CatalogState::load(conn)?;
    let source = state.archived.get(id).ok_or_else(invalid)?;
    // Preserve native extensions exactly. Cross-format projection is a separate
    // action after this source has been restored to its original tool format.
    if tool != source.tool {
        return Err(
            "Restore this library entry to its original tool before synchronizing it".into(),
        );
    }
    super::mutations::install_batch_restoring(
        conn,
        tool,
        vec![(source.name.clone(), source.spec.clone())],
        vec![],
        Some(id),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
