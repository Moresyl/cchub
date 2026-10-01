use super::snapshot::{check_library, check_live, read_live};
use super::*;
use crate::config_write::{self, FileUpdate};

fn preserve_original(
    conn: &Connection,
    app: &str,
    replaced_id: &str,
    content: &str,
) -> Result<(), String> {
    let retained: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM prompt_library WHERE app_id=?1 AND id!=?2 AND content=?3)",
            params![app, replaced_id, content],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !retained {
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO prompt_library(app_id,id,name,content,description,enabled,created_at,updated_at) VALUES(?1,?2,'Previous live instructions',?3,'Saved before replacement',0,?4,?4)",
            params![app, uuid::Uuid::new_v4().to_string(), content, now],
        ).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn save_at(
    conn: &mut Connection,
    app: &str,
    id: &str,
    prompt: PromptInput,
    live_path: Option<&Path>,
    expected_library: Option<&str>,
    expected_live: Option<&str>,
) -> Result<PromptRecord, String> {
    validate_prompt(id, &prompt)?;
    let _guard = crate::json_config::write_lock()?;
    let transaction = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    check_library(&transaction, app, expected_library)?;
    let mut updates = Vec::new();
    if prompt.enabled {
        if let Some(path) = live_path {
            let live = read_live(path)?;
            check_live(&live, expected_live)?;
            if let Some(original) = &live.content {
                if original != &prompt.content {
                    preserve_original(&transaction, app, id, original)?;
                }
            }
            updates.push(FileUpdate {
                path: path.into(),
                original: live.content.map(String::into_bytes),
                desired: prompt.content.as_bytes().to_vec(),
            });
        }
    }
    let now = Utc::now().timestamp_millis();
    if prompt.enabled {
        transaction
            .execute(
                "UPDATE prompt_library SET enabled=0,updated_at=?2 WHERE app_id=?1 AND enabled=1",
                params![app, now],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.execute(
        "INSERT INTO prompt_library(app_id,id,name,content,description,enabled,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?7)
         ON CONFLICT(app_id,id) DO UPDATE SET name=excluded.name,content=excluded.content,
         description=excluded.description,enabled=excluded.enabled,updated_at=excluded.updated_at",
        params![app,id,prompt.name.trim(),prompt.content,
            prompt.description.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()),
            i64::from(prompt.enabled),now],
    ).map_err(|error| error.to_string())?;
    let record = load_prompts(&transaction, app)?
        .remove(id)
        .ok_or("Cannot reload saved prompt")?;
    config_write::commit_then(updates, move || {
        transaction.commit().map_err(|error| error.to_string())
    })?;
    Ok(record)
}

pub(super) fn enable_at(
    conn: &mut Connection,
    app: &str,
    id: &str,
    live_path: Option<&Path>,
    expected_library: Option<&str>,
    expected_live: Option<&str>,
) -> Result<(), String> {
    let mut records = load_prompts(conn, app)?;
    let revision = super::snapshot::library_revision(app, &records)?;
    let record = records.remove(id).ok_or("Prompt not found")?;
    save_at(
        conn,
        app,
        id,
        PromptInput {
            name: record.name,
            content: record.content,
            description: record.description,
            enabled: true,
        },
        live_path,
        expected_library.or(Some(&revision)),
        expected_live,
    )
    .map(|_| ())
}

pub(super) fn import_at(
    conn: &mut Connection,
    app: &str,
    path: &Path,
    expected_library: Option<&str>,
    expected_live: Option<&str>,
) -> Result<String, String> {
    let live = read_live(path)?;
    check_live(&live, expected_live)?;
    let content = live.content.ok_or("Prompt file does not exist")?;
    let records = load_prompts(conn, app)?;
    let revision = super::snapshot::library_revision(app, &records)?;
    let existing = records
        .into_values()
        .find(|record| record.content == content);
    let (id, name, description) = existing
        .map(|record| (record.id, record.name, record.description))
        .unwrap_or_else(|| {
            (
                uuid::Uuid::new_v4().to_string(),
                format!("Imported {app} instructions"),
                None,
            )
        });
    // Validate the read again when committing; importing never rewrites the file.
    save_at(
        conn,
        app,
        &id,
        PromptInput {
            name,
            content,
            description,
            enabled: true,
        },
        Some(path),
        expected_library.or(Some(&revision)),
        Some(&live.revision),
    )?;
    Ok(id)
}

#[cfg(test)]
mod tests;
