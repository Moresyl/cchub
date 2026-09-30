//! Native accounting with durable identities across V1/V2 migration and copies.
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;

use super::session_usage_compat::SessionSyncResult;
use crate::opencode_accounting::{read_usage, UsageRecord};

const DATA_SOURCE: &str = "opencode_session";
const BATCH_SIZE: usize = 500;

enum Change {
    Imported,
    Updated,
    Unchanged,
}

fn identity(record: &UsageRecord) -> String {
    let mut hasher = Sha256::new();
    // Length prefixes avoid delimiter collisions; paths and table layout are
    // intentionally excluded so moving or migrating a database cannot rebill it.
    for part in [&record.session, &record.message] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    format!("opencode-session:{:x}", hasher.finalize())
}

fn persist(conn: &Connection, record: &UsageRecord) -> Result<Change, String> {
    let id = identity(record);
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(record).map_err(|error| error.to_string())?)
    );
    let previous: Option<String> = conn.query_row(
        "SELECT semantic_id FROM session_usage_dedup WHERE data_source = ?1 AND request_id = ?2",
        rusqlite::params![DATA_SOURCE, id], |row| row.get(0),
    ).optional().map_err(|error| error.to_string())?;
    if previous.as_deref() == Some(&fingerprint) {
        return Ok(Change::Unchanged);
    }
    let tokens = record.tokens;
    let cost = match record.reported_cost {
        Some(cost) => cost,
        None => super::session_usage_pricing::estimate_cost(
            conn,
            &record.model,
            [
                tokens.input,
                tokens.output,
                tokens.cache_read,
                tokens.cache_write,
            ],
        )?,
    };
    // Once a log was deliberately removed, keep its durable identity. Corrections
    // may update existing logs but must never resurrect cleaned-up usage rows.
    let cost_text = format!("{cost:.9}");
    let provider = format!("{} (Session)", record.provider);
    let profile = format!("opencode-session:{}", record.session);
    let status = if record.failed { 500 } else { 200 };
    let error = if record.failed {
        Some("Native request failed")
    } else {
        None
    };
    let input = tokens.input.min(i64::MAX as u64) as i64;
    let output = tokens.output.min(i64::MAX as u64) as i64;
    let cache_read = tokens.cache_read.min(i64::MAX as u64) as i64;
    let cache_write = tokens.cache_write.min(i64::MAX as u64) as i64;
    let values = rusqlite::params![
        id,
        record.model,
        input,
        output,
        cache_read,
        cache_write,
        cost_text,
        status,
        error,
        record.created_at,
        provider,
        profile
    ];
    let changed = if previous.is_some() {
        conn.execute("UPDATE proxy_request_logs SET request_model = ?2, response_model = ?2,
            input_tokens = ?3, output_tokens = ?4, cache_read_tokens = ?5, cache_creation_tokens = ?6,
            total_cost_usd = ?7, status_code = ?8, error_message = ?9, created_at = ?10,
            provider_name = ?11, profile_id = ?12 WHERE request_id = ?1 AND tool_id = 'opencode'", values)
    } else {
        conn.execute("INSERT OR IGNORE INTO proxy_request_logs (request_id, tool_id, profile_id, provider_name,
            request_model, response_model, input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            total_cost_usd, latency_ms, status_code, is_streaming, error_message, created_at)
            VALUES (?1, 'opencode', ?12, ?11, ?2, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, 0, ?9, ?10)", values)
    }.map_err(|error| error.to_string())?;
    conn.execute("INSERT INTO session_usage_dedup (data_source, request_id, semantic_id, has_entry_id, created_at)
        VALUES (?1, ?2, ?3, 1, ?4) ON CONFLICT(data_source, request_id) DO UPDATE SET semantic_id = excluded.semantic_id",
        rusqlite::params![DATA_SOURCE, id, fingerprint, record.created_at],
    ).map_err(|error| error.to_string())?;
    Ok(if changed == 0 {
        Change::Unchanged
    } else if previous.is_some() {
        Change::Updated
    } else {
        Change::Imported
    })
}

pub(crate) fn sync_from_path(
    conn: &mut Connection,
    path: &Path,
) -> Result<SessionSyncResult, String> {
    if !path.try_exists().map_err(|error| error.to_string())? {
        return Ok(SessionSyncResult::default());
    }
    let mut source = crate::opencode_storage::open_readonly(path)?;
    let snapshot = source.transaction().map_err(|error| error.to_string())?;
    let batch = read_usage(&snapshot, None)?;
    drop(snapshot);
    let mut result = SessionSyncResult {
        files_scanned: 1,
        skipped: batch.malformed,
        deferred_files: u32::from(batch.pending > 0),
        ..Default::default()
    };
    for records in batch.records.chunks(BATCH_SIZE) {
        let transaction = conn.transaction().map_err(|error| error.to_string())?;
        let mut changes = SessionSyncResult::default();
        let mut failed = None;
        for record in records {
            match persist(&transaction, record) {
                Ok(Change::Imported) => changes.imported += 1,
                Ok(Change::Updated) => changes.updated += 1,
                Ok(Change::Unchanged) => {
                    changes.skipped += 1;
                    changes.suspected_duplicates += 1;
                }
                Err(error) => {
                    failed = Some(error);
                    break;
                }
            }
        }
        if let Some(error) = failed {
            result
                .errors
                .push(format!("OpenCode accounting batch was not saved: {error}"));
            continue; // Dropping the transaction rolls back logs and identities together.
        }
        transaction.commit().map_err(|error| error.to_string())?;
        result.merge(changes);
    }
    Ok(result)
}

pub(crate) fn sync_opencode_usage(conn: &mut Connection) -> Result<SessionSyncResult, String> {
    let home = dirs::home_dir().ok_or("Cannot determine the home directory")?;
    sync_from_path(conn, &crate::opencode_paths::database_path(&home))
}

#[cfg(test)]
#[path = "opencode_session_usage_tests.rs"]
mod tests;
