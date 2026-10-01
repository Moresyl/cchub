use super::*;
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

fn key(tool: &str) -> Result<String, String> {
    if !matches!(
        tool,
        "claude" | "codex" | "gemini" | "grokbuild" | "opencode" | "openclaw" | "hermes"
    ) {
        return Err("Routing is not supported for this tool".into());
    }
    Ok(format!("provider_routing:{tool}"))
}

pub(crate) fn load(conn: &Connection, tool: &str) -> Result<RoutingDocument, String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [key(tool)?],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| "Could not read routing settings")?;
    match raw {
        None => Ok(RoutingDocument::default()),
        Some(raw) if raw.len() <= 128 * 1024 => serde_json::from_str(&raw).map_err(|_| {
            "Saved routing settings are invalid; repair or reset them before routing".into()
        }),
        _ => Err("Saved routing settings exceed supported limits".into()),
    }
}

pub(crate) fn save(
    conn: &Connection,
    tool: &str,
    expected_revision: Option<&str>,
    policy: RoutingPolicy,
) -> Result<RoutingDocument, String> {
    let setting_key = key(tool)?;
    validation::validate(&policy)?;
    let document = RoutingDocument {
        revision: Some(Uuid::new_v4().to_string()),
        policy,
    };
    let payload =
        serde_json::to_string(&document).map_err(|_| "Could not encode routing settings")?;
    if payload.len() > 128 * 1024 {
        return Err("Routing settings exceed supported limits".into());
    }
    let tx = conn
        .unchecked_transaction()
        .map_err(|_| "Could not start routing settings update")?;
    let current = load(&tx, tool)?;
    if current.revision.as_deref() != expected_revision {
        return Err("Routing settings changed elsewhere; reload before saving".into());
    }
    let available = profile_ids(&tx, tool)?;
    if document.policy.enabled && document.policy.groups.iter().flat_map(|group| &group.members).any(|member|
        matches!(member, RoutingMember::Profile {profile_id} if !available.contains(profile_id))) {
        return Err("A routing member is missing or belongs to another tool".into());
    }
    tx.execute("INSERT INTO app_settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (setting_key, payload))
        .map_err(|_| "Could not save routing settings")?;
    tx.commit()
        .map_err(|_| "Could not finish routing settings update")?;
    Ok(document)
}

fn profile_ids(conn: &Connection, tool: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare("SELECT id FROM config_profiles WHERE tool_id=?1 ORDER BY COALESCE(sort_order,0),updated_at DESC,created_at DESC")
        .map_err(|_| "Could not read routing profiles")?;
    let rows = stmt
        .query_map([tool], |row| row.get::<_, String>(0))
        .map_err(|_| "Could not read routing profiles")?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Could not read routing profiles".into())
}

pub(crate) fn preview(
    conn: &Connection,
    tool: &str,
    policy: RoutingPolicy,
    body: Value,
    path: &str,
    request_bytes: Option<u64>,
) -> Result<RoutingPreview, String> {
    key(tool)?;
    validation::validate(&policy)?;
    let bytes = serde_json::to_vec(&body).map_err(|_| "Could not read routing preview input")?;
    if bytes.len() > super::super::MAX_PROXY_BODY_BYTES
        || request_bytes.is_some_and(|size| size > super::super::MAX_PROXY_BODY_BYTES as u64)
    {
        return Err("Routing preview input is too large".into());
    }
    let available = super::super::profiles::read_profile_candidates_for_tool(conn, tool)?
        .into_iter()
        .map(|profile| profile.profile_id)
        .collect::<Vec<_>>();
    plan::resolve(
        &RoutingDocument {
            revision: None,
            policy,
        },
        &bytes,
        path,
        &available,
        request_bytes,
        |_, _| 0,
    )
}
