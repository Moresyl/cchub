use super::*;
use crate::config_write::{FilePlan, FileUpdate};
use serde_json::Value;

fn strict_json(source: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(source).map_err(|_| "Invalid native JSON configuration")?;
    crate::json_config::parse_json_object(source)
}

fn toml(source: &str) -> Result<(), String> {
    source
        .parse::<toml_edit::DocumentMut>()
        .map(|_| ())
        .map_err(|_| "Invalid native TOML configuration".into())
}

// Preparation only reads. The caller holds the application write lock through
// the eventual group commit, and can attach a database transaction finalizer.
pub(crate) fn prepare_tool_snapshot(
    conn: &rusqlite::Connection,
    tool_id: &str,
    snapshot: &str,
    preserve_user_edits: bool,
) -> Result<FilePlan, String> {
    let snapshot = if tool_id == "opencode" {
        crate::opencode_profiles::normalize_profile(snapshot)?
    } else {
        snapshot.to_owned()
    };
    let effective =
        crate::provider_proxy::materialize_tool_snapshot_for_runtime(conn, tool_id, &snapshot)?;
    let mut plan = FilePlan::default();
    match tool_id {
        "mcode" => return Err("MiniMax Code providers must be managed individually".into()),
        "claude" => {
            let (global, settings) = resolve_claude_paths(conn)?;
            return claude::prepare_at([&global, &settings], &effective)
                .map(|prepared| prepared.into_plan());
        }
        "opencode" => {
            return crate::opencode_profiles::prepare_profile(
                &resolve_tool_config_path(conn, tool_id)?,
                &effective,
            )
        }
        "hermes" => {
            return crate::hermes::snapshot::prepare_snapshot(conn, &effective, true)
                .map(|(plan, _)| plan)
        }
        "grokbuild" => plan.replace(
            resolve_tool_config_path(conn, tool_id)?,
            crate::grok_config::snapshot_to_toml(&effective)?.into_bytes(),
        )?,
        "codex" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            let config_path = dir.join("config.toml");
            let config_original = crate::config_write::read(&config_path)?;
            let source = if preserve_user_edits {
                if let Some(bytes) = &config_original {
                    toml(
                        std::str::from_utf8(bytes)
                            .map_err(|_| "Codex configuration must be UTF-8")?,
                    )?;
                }
                overlay_codex_user_fields_into_snapshot(&effective, &config_path)
            } else {
                effective
            };
            // Legacy plain TOML remains supported. A malformed JSON envelope
            // cannot fall through and be written into a .toml file.
            if source.trim_start().starts_with('{') {
                let value = strict_json(&source)?;
                let auth = value
                    .get("auth")
                    .filter(|auth| auth.is_object())
                    .ok_or("Codex snapshot requires an auth object")?;
                let config = value
                    .get("config")
                    .and_then(Value::as_str)
                    .ok_or("Codex snapshot requires a TOML config string")?;
                toml(config)?;
                plan.replace(
                    dir.join("auth.json"),
                    serde_json::to_vec_pretty(auth)
                        .map_err(|_| "Cannot encode Codex authentication")?,
                )?;
                plan.updates.push(FileUpdate {
                    path: config_path,
                    original: config_original,
                    desired: config.as_bytes().to_vec(),
                });
            } else {
                toml(&source)?;
                plan.updates.push(FileUpdate {
                    path: config_path,
                    original: config_original,
                    desired: source.into_bytes(),
                });
            }
        }
        "gemini" => {
            let dir = resolve_tool_config_dir(conn, tool_id)?;
            let value = strict_json(&effective)?;
            if value.get("config").is_some() || value.get("env").is_some() {
                let config = value
                    .get("config")
                    .filter(|config| config.is_object())
                    .ok_or("Gemini snapshot requires a config object")?;
                let env: HashMap<String, String> = serde_json::from_value(
                    value
                        .get("env")
                        .cloned()
                        .ok_or("Gemini snapshot requires an env object")?,
                )
                .map_err(|_| "Gemini environment must contain string values")?;
                let env_text = crate::hermes::env::render_env_map(&env)?;
                plan.replace(dir.join(".env"), env_text.into_bytes())?;
                plan.replace(
                    dir.join("settings.json"),
                    serde_json::to_vec_pretty(config)
                        .map_err(|_| "Cannot encode Gemini settings")?,
                )?;
            } else {
                plan.replace(dir.join("settings.json"), effective.into_bytes())?;
            }
        }
        "pi" | "openclaw" => {
            if tool_id == "pi" {
                strict_json(&effective)?;
            } else {
                crate::json_config::parse_json5_object(&effective)?;
            }
            plan.replace(
                resolve_tool_config_path(conn, tool_id)?,
                effective.into_bytes(),
            )?;
        }
        _ => return Err("Unsupported tool configuration snapshot".into()),
    }
    Ok(plan)
}

#[cfg(test)]
mod tests;
