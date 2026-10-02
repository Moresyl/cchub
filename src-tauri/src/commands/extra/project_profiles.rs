use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::DbState;

use super::config_profiles::{apply_profile_group, get_active_config_profile_ids_from_conn};

const MAX_NAME_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 2_000;
const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfileSnapshot {
    pub version: u32,
    pub workspace_id: Option<String>,
    pub config_profile_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub snapshot: ProjectProfileSnapshot,
    pub created_at: String,
    pub updated_at: String,
    pub last_applied_at: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfileMutationResult {
    pub profile: ProjectProfile,
    pub applied_profile_ids: Vec<String>,
}

fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_NAME_CHARS {
        return Err("Project profile name must contain 1 to 120 characters".to_string());
    }
    if trimmed.chars().any(|character| character.is_control()) {
        return Err("Project profile name contains invalid control characters".to_string());
    }
    Ok(trimmed.to_string())
}

fn normalize_description(description: Option<String>) -> Result<Option<String>, String> {
    let Some(value) = description else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err("Project profile description is too long".to_string());
    }
    if trimmed.chars().any(|character| {
        character.is_control() && character != '\n' && character != '\r' && character != '\t'
    }) {
        return Err("Project profile description contains invalid control characters".to_string());
    }
    Ok((!trimmed.is_empty()).then(|| trimmed.to_string()))
}

fn current_workspace_id(conn: &rusqlite::Connection) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT id FROM workspaces WHERE is_active = 1 ORDER BY created_at ASC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn capture_snapshot(conn: &rusqlite::Connection) -> Result<ProjectProfileSnapshot, String> {
    let mut ids = get_active_config_profile_ids_from_conn(conn)?;
    ids.sort();
    ids.dedup();
    Ok(ProjectProfileSnapshot {
        version: SNAPSHOT_VERSION,
        workspace_id: current_workspace_id(conn)?,
        config_profile_ids: ids,
    })
}

fn snapshot_matches_state(
    snapshot: &ProjectProfileSnapshot,
    workspace_id: &Option<String>,
    current_ids: &[String],
) -> bool {
    if snapshot.version != SNAPSHOT_VERSION
        || snapshot.workspace_id.as_ref() != workspace_id.as_ref()
    {
        return false;
    }
    let mut expected = snapshot.config_profile_ids.clone();
    expected.sort();
    expected.dedup();
    current_ids == expected
}

fn parse_snapshot(raw: &str) -> Result<ProjectProfileSnapshot, String> {
    let snapshot: ProjectProfileSnapshot = serde_json::from_str(raw)
        .map_err(|error| format!("Invalid project profile snapshot: {error}"))?;
    if snapshot.version != SNAPSHOT_VERSION {
        return Err(format!(
            "Unsupported project profile snapshot version: {}",
            snapshot.version
        ));
    }
    if snapshot.config_profile_ids.len() > 64 {
        return Err("Project profile contains too many configuration profiles".to_string());
    }
    if snapshot
        .config_profile_ids
        .iter()
        .any(|id| id.trim().is_empty() || id.len() > 128)
    {
        return Err("Project profile contains an invalid configuration profile id".to_string());
    }
    Ok(snapshot)
}

type ProjectProfileRow = (
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
);

fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectProfileRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
    ))
}

fn load_profiles(conn: &rusqlite::Connection) -> Result<Vec<ProjectProfile>, String> {
    let mut current_ids = get_active_config_profile_ids_from_conn(conn)?;
    current_ids.sort();
    current_ids.dedup();
    load_profiles_with_state(conn, &current_ids)
}

fn load_profiles_with_state(
    conn: &rusqlite::Connection,
    current_ids: &[String],
) -> Result<Vec<ProjectProfile>, String> {
    let workspace_id = current_workspace_id(conn)?;
    let mut statement = conn
        .prepare(
            "SELECT id, name, description, snapshot, created_at, updated_at, last_applied_at
             FROM project_profiles ORDER BY updated_at DESC, name COLLATE NOCASE ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], row_to_profile)
        .map_err(|error| error.to_string())?;
    let mut profiles = Vec::new();
    for row in rows {
        let (id, name, description, raw_snapshot, created_at, updated_at, last_applied_at) =
            row.map_err(|error| error.to_string())?;
        let snapshot = parse_snapshot(&raw_snapshot)?;
        let is_active = snapshot_matches_state(&snapshot, &workspace_id, &current_ids);
        profiles.push(ProjectProfile {
            id,
            name,
            description,
            snapshot,
            created_at,
            updated_at,
            last_applied_at,
            is_active,
        });
    }
    Ok(profiles)
}

fn load_profile_snapshot(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<ProjectProfileSnapshot, String> {
    let raw: String = conn
        .query_row(
            "SELECT snapshot FROM project_profiles WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Project profile not found: {error}"))?;
    parse_snapshot(&raw)
}

fn validate_snapshot_targets(
    conn: &rusqlite::Connection,
    snapshot: &ProjectProfileSnapshot,
) -> Result<HashMap<String, String>, String> {
    let mut statement = conn
        .prepare("SELECT id, tool_id FROM config_profiles WHERE id = ?1")
        .map_err(|error| error.to_string())?;
    let mut tools = HashMap::new();
    for profile_id in &snapshot.config_profile_ids {
        let tool_id: String = statement
            .query_row(params![profile_id], |row| row.get(1))
            .map_err(|error| {
                format!("Configuration profile {profile_id} is unavailable: {error}")
            })?;
        if tools.insert(tool_id.clone(), profile_id.clone()).is_some() {
            return Err(format!(
                "Project profile contains multiple profiles for tool {tool_id}"
            ));
        }
    }
    if let Some(workspace_id) = snapshot.workspace_id.as_deref() {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?1)",
                params![workspace_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if !exists {
            return Err(format!("Workspace {workspace_id} is unavailable"));
        }
    }
    Ok(tools)
}

#[tauri::command]
pub fn get_project_profiles(db: State<'_, DbState>) -> Result<Vec<ProjectProfile>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    load_profiles(&conn)
}

#[tauri::command]
pub fn create_project_profile(
    name: String,
    description: Option<String>,
    db: State<'_, DbState>,
) -> Result<ProjectProfile, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    create_project_from_conn(&conn, &name, description)
}

fn create_project_from_conn(
    conn: &rusqlite::Connection,
    name: &str,
    description: Option<String>,
) -> Result<ProjectProfile, String> {
    let name = validate_name(&name)?;
    let description = normalize_description(description)?;
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| "Cannot start project profile save")?;
    let snapshot = capture_snapshot(&tx)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let raw_snapshot = serde_json::to_string(&snapshot).map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO project_profiles (id, name, description, snapshot, created_at, updated_at, last_applied_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5, NULL)",
        params![id, name, description, raw_snapshot, now],
    )
    .map_err(|_| "Cannot save project profile")?;
    let profile = ProjectProfile {
        id,
        name,
        description,
        snapshot,
        created_at: now.clone(),
        updated_at: now,
        last_applied_at: None,
        is_active: true,
    };
    tx.commit()
        .map_err(|_| "Cannot commit project profile save")?;
    Ok(profile)
}

#[tauri::command]
pub fn update_project_profile(
    id: String,
    name: String,
    description: Option<String>,
    resnapshot: bool,
    db: State<'_, DbState>,
) -> Result<ProjectProfile, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    update_project_from_conn(&conn, &id, &name, description, resnapshot)
}

fn update_project_from_conn(
    conn: &rusqlite::Connection,
    id: &str,
    name: &str,
    description: Option<String>,
    resnapshot: bool,
) -> Result<ProjectProfile, String> {
    let name = validate_name(&name)?;
    let description = normalize_description(description)?;
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| "Cannot start project profile update")?;
    let current_snapshot = load_profile_snapshot(&tx, id)?;
    let (created_at, last_applied_at): (String, Option<String>) = tx
        .query_row(
            "SELECT created_at,last_applied_at FROM project_profiles WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "Project profile is unavailable")?;
    let current = capture_snapshot(&tx)?;
    let snapshot = if resnapshot {
        current.clone()
    } else {
        current_snapshot
    };
    let now = chrono::Utc::now().to_rfc3339();
    let raw_snapshot = serde_json::to_string(&snapshot).map_err(|error| error.to_string())?;
    let changed = tx
        .execute(
            "UPDATE project_profiles SET name = ?1, description = ?2, snapshot = ?3, updated_at = ?4 WHERE id = ?5",
            params![name, description, raw_snapshot, now, id],
        )
        .map_err(|_| "Cannot update project profile")?;
    if changed != 1 {
        return Err("Project profile not found".to_string());
    }
    let is_active = snapshot_matches_state(
        &snapshot,
        &current.workspace_id,
        &current.config_profile_ids,
    );
    let profile = ProjectProfile {
        id: id.to_owned(),
        name,
        description,
        snapshot,
        created_at,
        updated_at: now,
        last_applied_at,
        is_active,
    };
    tx.commit()
        .map_err(|_| "Cannot commit project profile update")?;
    Ok(profile)
}

#[tauri::command]
pub fn delete_project_profile(id: String, db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let changed = conn
        .execute("DELETE FROM project_profiles WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("Project profile not found".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn apply_project_profile(
    id: String,
    db: State<'_, DbState>,
) -> Result<ProjectProfileMutationResult, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    apply_project_from_conn(&conn, &id)
}

fn apply_project_from_conn(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<ProjectProfileMutationResult, String> {
    let snapshot = load_profile_snapshot(conn, id)?;
    apply_profile_group(
        conn,
        &snapshot.config_profile_ids,
        false,
        |tx, profiles, active| {
            // Re-read within the immediate transaction, so an external SQL edit
            // cannot silently replace the project that was prepared.
            if load_profile_snapshot(tx, id)? != snapshot {
                return Err("Project profile changed; reload before applying".into());
            }
            validate_snapshot_targets(tx, &snapshot)?;
            if let Some(workspace) = &snapshot.workspace_id {
                super::workspaces::activate_in_transaction(tx, workspace)?;
            } else {
                tx.execute("UPDATE workspaces SET is_active=0", [])
                    .map_err(|_| "Cannot restore project workspace selection")?;
            }
            let now = chrono::Utc::now().to_rfc3339();
            if tx
                .execute(
                    "UPDATE project_profiles SET last_applied_at=?1,updated_at=?1 WHERE id=?2",
                    params![now, id],
                )
                .map_err(|_| "Cannot record project profile application")?
                != 1
            {
                return Err("Project profile changed while applying".into());
            }
            let profile = load_profiles_with_state(tx, active)?
                .into_iter()
                .find(|profile| profile.id == id)
                .ok_or("Cannot reload applied project profile")?;
            Ok(ProjectProfileMutationResult {
                profile,
                applied_profile_ids: profiles.iter().map(|profile| profile.id.clone()).collect(),
            })
        },
    )
}

#[cfg(test)]
mod application_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_names_and_descriptions() {
        assert_eq!(validate_name("  demo  ").unwrap(), "demo");
        assert!(validate_name("").is_err());
        assert!(validate_name("\n").is_err());
        assert!(normalize_description(Some("  note  ".to_string()))
            .unwrap()
            .is_some());
        assert!(normalize_description(Some("\u{0001}".to_string())).is_err());
    }

    #[test]
    fn snapshot_deduplicates_active_profiles() {
        let mut ids = vec!["b".to_string(), "a".to_string(), "b".to_string()];
        ids.sort();
        ids.dedup();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn target_validation_rejects_duplicate_tool_profiles() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::schema::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, created_at, updated_at) VALUES (?1, ?2, ?3, '{}', 'now', 'now')",
            params!["one", "One", "claude"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO config_profiles (id, name, tool_id, config_snapshot, created_at, updated_at) VALUES (?1, ?2, ?3, '{}', 'now', 'now')",
            params!["two", "Two", "claude"],
        )
        .unwrap();
        let snapshot = ProjectProfileSnapshot {
            version: SNAPSHOT_VERSION,
            workspace_id: None,
            config_profile_ids: vec!["one".to_string(), "two".to_string()],
        };
        assert!(validate_snapshot_targets(&conn, &snapshot)
            .unwrap_err()
            .contains("multiple profiles"));
    }
}
