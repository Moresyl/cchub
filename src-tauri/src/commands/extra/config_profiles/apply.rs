#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;

use crate::hermes;

use super::*;

mod claude;
mod prepare;
pub(crate) use prepare::prepare_tool_snapshot;
#[cfg(test)]
mod reasoning_tests;

pub fn sync_profiles_from_compatible_databases(
    conn: &rusqlite::Connection,
    now: &str,
) -> Result<HashMap<String, usize>, String> {
    let mut counts = HashMap::new();
    let mut seen_ids = std::collections::HashSet::new();

    for db_path in compatible_db_paths() {
        let external = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;

        let mut stmt = external
            .prepare(
                "SELECT id, app_type, name, settings_config
                 FROM providers
                 WHERE app_type IN ('claude', 'codex', 'gemini')
                 ORDER BY app_type, name",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (provider_id, tool_id, name, settings_config) = row.map_err(|e| e.to_string())?;
            let Some(config_snapshot) =
                normalize_external_profile_snapshot(&tool_id, &settings_config)
            else {
                continue;
            };
            let id = format!("compat-{}-{}", tool_id, provider_id);
            let source_key = format!("{}#{}", db_path.display(), provider_id);

            upsert_synced_profile(
                conn,
                &id,
                &name,
                &tool_id,
                &config_snapshot,
                "compatible",
                Some(&source_key),
                now,
            )?;

            *counts.entry(tool_id).or_insert(0) += 1;
            seen_ids.insert(id);
        }
    }

    let mut stale_stmt = conn
        .prepare("SELECT id FROM config_profiles WHERE source_type = 'compatible'")
        .map_err(|e| e.to_string())?;
    let stale_ids: Vec<String> = stale_stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .filter(|id: &String| !seen_ids.contains(id))
        .collect();

    for id in stale_ids {
        conn.execute(
            "DELETE FROM config_profiles WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(counts)
}

pub fn sync_live_profiles(
    conn: &rusqlite::Connection,
    imported_counts: &HashMap<String, usize>,
    now: &str,
) -> Result<(), String> {
    for tool_id in [
        "claude",
        "codex",
        "gemini",
        "grokbuild",
        "opencode",
        "openclaw",
        "hermes",
        "pi",
    ] {
        let id = format!("live-{}", tool_id);

        if imported_counts.get(tool_id).copied().unwrap_or(0) > 0 {
            conn.execute(
                "DELETE FROM config_profiles WHERE id = ?1",
                rusqlite::params![id],
            )
            .map_err(|e| e.to_string())?;
            continue;
        }

        match read_tool_snapshot(conn, tool_id) {
            Ok(config_snapshot) => {
                let name = format!("{} 当前配置", tool_id);
                upsert_synced_profile(
                    conn,
                    &id,
                    &name,
                    tool_id,
                    &config_snapshot,
                    "live",
                    Some(tool_id),
                    now,
                )?;
            }
            Err(_) => {
                // An unreadable, temporarily absent or malformed native file
                // does not invalidate the last successfully captured profile.
                crate::utils::append_runtime_log("warn", "profiles", &format!("Could not refresh live configuration for {tool_id}; keeping the previous snapshot"));
            }
        }
    }

    Ok(())
}

pub fn config_contents_match(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }

    match (
        crate::json_config::parse_json_object(left),
        crate::json_config::parse_json_object(right),
    ) {
        (Ok(mut a), Ok(mut b)) => {
            // Strip metadata keys used for claude profile splitting
            for key in &["__claude_json_keys__", "__settings_json_keys__"] {
                a.as_object_mut().map(|o| o.remove(*key));
                b.as_object_mut().map(|o| o.remove(*key));
            }
            a == b
        }
        _ => left.trim() == right.trim(),
    }
}

pub fn read_tool_snapshot(conn: &rusqlite::Connection, tool_id: &str) -> Result<String, String> {
    match tool_id {
        "mcode" => Err("MiniMax Code providers must be managed individually".to_string()),
        "codex" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            let auth_path = dir.join("auth.json");
            if !auth_path.exists() {
                return Err(format!("Config file not found: {}", auth_path.display()));
            }
            let auth: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&auth_path).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let config_path = resolve_tool_config_path(conn, tool_id)?;
            let config = if config_path.exists() {
                std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?
            } else {
                String::new()
            };
            serde_json::to_string_pretty(&serde_json::json!({
                "auth": auth,
                "config": config,
            }))
            .map_err(|e| e.to_string())
        }
        "gemini" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            let env_path = dir.join(".env");
            if !env_path.exists() {
                return Err(format!("Config file not found: {}", env_path.display()));
            }
            let env_text = std::fs::read_to_string(&env_path).map_err(|e| e.to_string())?;
            let env: HashMap<String, String> = env_text
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                .filter_map(|l| {
                    l.split_once('=')
                        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                })
                .collect();
            let settings_path = resolve_tool_config_path(conn, tool_id)?;
            let config = if settings_path.exists() {
                serde_json::from_str::<serde_json::Value>(
                    &std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?
            } else {
                serde_json::json!({})
            };
            serde_json::to_string_pretty(&serde_json::json!({
                "env": env,
                "config": config,
            }))
            .map_err(|e| e.to_string())
        }
        "hermes" => hermes::snapshot::read_snapshot(conn),
        "opencode" => {
            crate::opencode_profiles::read_profile(&resolve_tool_config_path(conn, tool_id)?)
        }
        "grokbuild" => {
            crate::grok_config::read_snapshot_at(&resolve_tool_config_path(conn, tool_id)?)
        }
        "pi" => {
            let config_path = resolve_tool_config_path(conn, tool_id)?;
            if !config_path.exists() {
                return Err(format!("Config file not found: {}", config_path.display()));
            }
            std::fs::read_to_string(&config_path).map_err(|e| e.to_string())
        }
        "claude" => {
            let (claude_json, settings_json) = resolve_claude_paths(conn)?;
            claude::read_at([&claude_json, &settings_json])
        }
        _ => {
            let config_path = resolve_tool_config_path(conn, tool_id)?;
            if !config_path.exists() {
                return Err(format!("Config file not found: {}", config_path.display()));
            }
            std::fs::read_to_string(&config_path).map_err(|e| e.to_string())
        }
    }
}

pub fn apply_tool_snapshot(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
) -> Result<(), String> {
    apply_tool_snapshot_with_options(conn, tool_id, snapshot, false)
}

/// Codex TOML keys that are managed by the Tools page / ConfigFiles page and should survive
/// startup reapply of the active profile. On explicit profile switch we intentionally overwrite
/// them, but on unattended startup reapply (whose only real job is to rewrite the proxy base_url)
/// we want the user's last-known values to persist across restarts.
const CODEX_USER_MANAGED_KEYS: &[&str] = &[
    "personality",
    "model_reasoning_effort",
    "disable_response_storage",
    "model_context_window",
    "model_auto_compact_token_limit",
];

/// Overlay the Tools-page-managed codex fields from the existing config.toml on disk onto
/// the snapshot's inline config TOML. Returns the updated snapshot JSON string.
fn overlay_codex_user_fields_into_snapshot(
    snapshot_json: &str,
    existing_config_path: &std::path::Path,
) -> String {
    if !existing_config_path.exists() {
        return snapshot_json.to_string();
    }
    let Ok(existing_text) = std::fs::read_to_string(existing_config_path) else {
        return snapshot_json.to_string();
    };
    let Ok(existing_doc) = existing_text.parse::<toml_edit::DocumentMut>() else {
        return snapshot_json.to_string();
    };
    let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(snapshot_json) else {
        return snapshot_json.to_string();
    };
    let Some(obj) = parsed.as_object_mut() else {
        return snapshot_json.to_string();
    };
    let Some(config_text) = obj
        .get("config")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    else {
        return snapshot_json.to_string();
    };
    let Ok(mut snapshot_doc) = config_text.parse::<toml_edit::DocumentMut>() else {
        return snapshot_json.to_string();
    };

    for key in CODEX_USER_MANAGED_KEYS {
        if let Some(existing_value) = existing_doc.get(key) {
            snapshot_doc[*key] = existing_value.clone();
        } else if *key == "model_reasoning_effort" {
            // An omitted effort is a deliberate model default, not a missing edit.
            snapshot_doc.as_table_mut().remove(key);
        }
    }

    obj.insert(
        "config".to_string(),
        serde_json::Value::String(snapshot_doc.to_string()),
    );
    serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| snapshot_json.to_string())
}

pub fn apply_tool_snapshot_with_options(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
    preserve_user_edits: bool,
) -> Result<(), String> {
    let _guard = crate::json_config::write_lock()?;
    prepare_tool_snapshot(conn, tool_id, snapshot, preserve_user_edits)?.commit()
}
