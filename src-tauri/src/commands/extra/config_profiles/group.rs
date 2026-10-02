use super::*;
use rusqlite::{Connection, Transaction, TransactionBehavior};

pub(crate) struct AppliedProfile {
    pub id: String,
    pub tool: String,
    pub snapshot: String,
}

pub(crate) fn apply_profile_group<T>(
    conn: &Connection,
    ids: &[String],
    preserve_user_edits: bool,
    finalize: impl FnOnce(&Connection, &[AppliedProfile], &[String]) -> Result<T, String>,
) -> Result<T, String> {
    if ids.len() > 64 {
        return Err("Too many configuration profiles in one operation".into());
    }
    // Resolve untouched tool selections before taking the write lock: live
    // snapshot readers take that lock themselves. Callbacks below only read SQL.
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|_| "Cannot start configuration selection transaction")?;
    let mut active_ids = get_active_config_profile_ids_from_conn(&tx)?;
    let _guard = crate::json_config::write_lock()?;
    let mut files = crate::config_write::FilePlan::default();
    let mut profiles = Vec::with_capacity(ids.len());
    let mut tools = std::collections::HashSet::new();
    for id in ids {
        let (tool, snapshot): (String, String) = tx
            .query_row(
                "SELECT tool_id,config_snapshot FROM config_profiles WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| "Configuration profile is unavailable")?;
        if !tools.insert(tool.clone()) {
            return Err("Multiple configuration profiles target the same tool".into());
        }
        files.extend(prepare_tool_snapshot(
            &tx,
            &tool,
            &snapshot,
            preserve_user_edits,
        )?);
        if tool == "claude" {
            files.extend(crate::commands::claude_extension::prepare_for_profile(
                &tx, &snapshot,
            )?);
        }
        profiles.push(AppliedProfile {
            id: id.clone(),
            tool,
            snapshot,
        });
    }
    let now = chrono::Utc::now().to_rfc3339();
    for profile in &profiles {
        let changed = tx
            .execute(
                "UPDATE config_profiles SET updated_at=?1 WHERE id=?2",
                rusqlite::params![now, profile.id],
            )
            .map_err(|_| "Cannot record configuration selection")?;
        if changed != 1 {
            return Err("Configuration profile changed while applying".into());
        }
        tx.execute(
            "INSERT OR REPLACE INTO app_settings(key,value) VALUES(?1,?2)",
            rusqlite::params![current_profile_setting_key(&profile.tool), profile.id],
        )
        .map_err(|_| "Cannot record active configuration")?;
        tx.execute("INSERT INTO activity_logs(server_id,request_type,status,recorded_at) VALUES(?1,'profile_switch','success',?2)",
            rusqlite::params![profile.tool, now]).map_err(|_| "Cannot record configuration activity")?;
    }
    // Keep untouched selections, replace only selections for the applied tools.
    let mut retained = Vec::new();
    for id in active_ids.drain(..) {
        let tool: String = tx
            .query_row(
                "SELECT tool_id FROM config_profiles WHERE id=?1",
                [&id],
                |row| row.get(0),
            )
            .map_err(|_| "Cannot confirm active configuration")?;
        if !tools.contains(&tool) {
            retained.push(id);
        }
    }
    retained.extend(profiles.iter().map(|profile| profile.id.clone()));
    retained.sort();
    retained.dedup();
    let result = finalize(&tx, &profiles, &retained)?;
    files.commit_then(move || {
        tx.commit()
            .map_err(|_| "Cannot commit configuration selection".into())
    })?;
    for profile in &profiles {
        crate::utils::append_runtime_log(
            "info",
            "profiles",
            &format!("Applied profile {} for tool {}", profile.id, profile.tool),
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
