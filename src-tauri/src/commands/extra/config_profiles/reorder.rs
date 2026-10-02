use rusqlite::{Connection, TransactionBehavior};
use std::collections::HashSet;

pub(super) fn save_order(
    conn: &mut Connection,
    tool_id: &str,
    ordered_ids: &[String],
) -> Result<(), String> {
    if ordered_ids.is_empty() {
        return Ok(());
    }
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let existing = {
        let mut statement = transaction
            .prepare(
                "SELECT id FROM config_profiles WHERE tool_id = ?1
                 ORDER BY COALESCE(sort_order, 0), updated_at DESC, created_at DESC, id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([tool_id], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    let available: HashSet<&str> = existing.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    for id in ordered_ids {
        if !seen.insert(id.as_str()) {
            return Err("配置排序中存在重复项目，请刷新后重试".into());
        }
        if !available.contains(id.as_str()) {
            return Err("配置已变化或不属于当前工具，请刷新后重试".into());
        }
    }
    // A concurrently added profile stays after the requested order. Omitted
    // profiles retain their relative order; another tool is never updated.
    let next = ordered_ids
        .iter()
        .chain(existing.iter().filter(|id| !seen.contains(id.as_str())));
    for (index, id) in next.enumerate() {
        transaction
            .execute(
                "UPDATE config_profiles SET sort_order = ?1 WHERE id = ?2 AND tool_id = ?3",
                rusqlite::params![index as i64, id, tool_id],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
