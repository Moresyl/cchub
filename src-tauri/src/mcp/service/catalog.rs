use super::super::sources::SourceSnapshot;
use super::*;

/// Prepared catalog changes, with a complete source snapshot and its revisions.
/// No SQL/native mutation happens until the explicit immediate transaction.
pub(crate) struct CatalogRefresh {
    pub(super) snapshot: SourceSnapshot,
    pub(super) state: CatalogState,
    pub(super) legacy: Vec<(String, String)>,
    pub(super) unresolved: Vec<String>,
}

pub(crate) fn prepare_refresh(conn: &Connection) -> Result<CatalogRefresh, String> {
    let snapshot = SourceSnapshot::read_configured(conn)?;
    prepare_snapshot(conn, snapshot)
}

pub(super) fn prepare_snapshot(
    conn: &Connection,
    snapshot: SourceSnapshot,
) -> Result<CatalogRefresh, String> {
    let mut state = CatalogState::load(conn)?;
    let rows = rows(conn)?;
    let mut discovered = BTreeMap::new();
    for origin in &snapshot.origins {
        // A discovered projection is part of its logical source, even when an
        // external edit makes it conflicting. Presence never grants ownership.
        let projection = state.projections.iter().any(|projection| {
            projection.canonical_path == origin.canonical_path
                && projection.container == origin.container
                && projection.native_name == origin.native_name
        });
        if !projection {
            discovered.insert(origin.id.clone(), origin.clone());
        }
    }
    let mut legacy = Vec::new();
    let mut unresolved = Vec::new();
    for row in &rows {
        if state.origins.contains_key(&row.id)
            || state.archived.contains_key(&row.id)
            || discovered.contains_key(&row.id)
            || row.status == "removed"
        {
            continue;
        }
        let candidates = discovered
            .values()
            .filter(|origin| migration::matches(row, origin))
            .collect::<Vec<_>>();
        if candidates.len() == 1 {
            legacy.push((row.id.clone(), candidates[0].id.clone()));
        } else {
            unresolved.push(row.id.clone());
        }
    }
    state.origins.extend(discovered);
    Ok(CatalogRefresh {
        snapshot,
        state,
        legacy,
        unresolved,
    })
}

impl CatalogRefresh {
    pub(crate) fn commit(self, conn: &Connection) -> Result<Vec<McpServer>, String> {
        self.snapshot.verify()?;
        let tx =
            rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
                .map_err(|_| invalid())?;
        let result = self.apply(&tx)?;
        // Scanning can be expensive; prove sources are still the requested
        // complete revision immediately before committing catalog metadata.
        self.snapshot.verify()?;
        tx.commit().map_err(|_| invalid())?;
        Ok(result)
    }

    fn apply(&self, conn: &Connection) -> Result<Vec<McpServer>, String> {
        for origin in self.state.origins.values() {
            let present = self
                .snapshot
                .origins
                .iter()
                .find(|entry| entry.id == origin.id);
            let status = match present {
                None => "missing",
                Some(origin) if origin.disabled => "disabled",
                Some(_) => "active",
            };
            native::save_row(conn, origin, status)?;
        }
        for (old_id, new_id) in &self.legacy {
            migration::transfer(conn, old_id, new_id)?;
        }
        for id in &self.unresolved {
            conn.execute("UPDATE mcp_servers SET status='conflict' WHERE id=?1", [id])
                .map_err(|_| invalid())?;
        }
        self.state.save(conn)?;
        Ok(rows(conn)?
            .into_iter()
            .filter(|row| row.status != "removed")
            .collect())
    }
}
