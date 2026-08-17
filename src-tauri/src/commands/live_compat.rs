use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::State;

use crate::commands::extra_commands::{
    apply_tool_snapshot, get_active_config_profile_ids_from_conn,
    read_all_config_profiles_from_conn, read_tool_snapshot,
};
use crate::db::DbState;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointSortUpdate {
    pub id: String,
    pub sort_order: i64,
}

#[tauri::command]
pub fn read_live_provider_settings(
    app: String,
    db: State<'_, DbState>,
) -> Result<serde_json::Value, String> {
    let tool = normalize_tool(&app)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let snapshot = read_tool_snapshot(&conn, tool)?;
    serde_json::from_str(&snapshot).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn sync_current_providers_live(db: State<'_, DbState>) -> Result<serde_json::Value, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let active_ids = get_active_config_profile_ids_from_conn(&conn)?;
    let profiles = read_all_config_profiles_from_conn(&conn)?;
    let mut synced = Vec::new();
    for profile_id in active_ids {
        let Some(profile) = profiles.iter().find(|item| item.id == profile_id) else {
            continue;
        };
        apply_tool_snapshot(&conn, &profile.tool_id, &profile.config_snapshot)?;
        synced.push(profile.id.clone());
    }
    Ok(serde_json::json!({"success": true, "syncedProfileIds": synced}))
}

#[tauri::command]
pub fn update_providers_sort_order(
    app: String,
    updates: Vec<EndpointSortUpdate>,
    db: State<'_, DbState>,
) -> Result<bool, String> {
    let tool = normalize_tool(&app)?;
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    for update in updates {
        let belongs: Option<String> = conn
            .query_row(
                "SELECT tool_id FROM config_profiles WHERE id = ?1",
                rusqlite::params![&update.id],
                |row| row.get(0),
            )
            .ok();
        if belongs.as_deref() != Some(tool) {
            return Err(format!(
                "Profile does not belong to tool {tool}: {}",
                update.id
            ));
        }
        conn.execute(
            "UPDATE config_profiles SET sort_order = ?1 WHERE id = ?2",
            rusqlite::params![update.sort_order, &update.id],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(true)
}

#[tauri::command(rename_all = "camelCase")]
pub fn update_endpoint_last_used(
    app: String,
    provider_id: String,
    url: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let _ = normalize_tool(&app)?;
    let normalized_url = reqwest::Url::parse(url.trim())
        .map_err(|error| format!("Invalid endpoint URL: {error}"))?
        .to_string();
    let key = format!("endpoint_last_used:{}:{}", app.trim(), provider_id.trim());
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, normalized_url],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_opencode_live_provider_ids(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    let value = read_live_provider_settings("opencode".to_string(), db)?;
    Ok(value
        .pointer("/providers")
        .and_then(serde_json::Value::as_object)
        .map(|providers| providers.keys().cloned().collect())
        .unwrap_or_default())
}

#[tauri::command]
pub fn get_opencode_models(db: State<'_, DbState>) -> Result<Vec<serde_json::Value>, String> {
    let value = read_live_provider_settings("opencode".to_string(), db)?;
    let mut models = Vec::new();
    if let Some(providers) = value
        .pointer("/providers")
        .and_then(serde_json::Value::as_object)
    {
        for (provider_id, provider) in providers {
            if let Some(entries) = provider
                .get("models")
                .and_then(serde_json::Value::as_object)
            {
                for (model_id, model) in entries {
                    models.push(serde_json::json!({
                        "id": format!("{provider_id}/{model_id}"),
                        "provider": provider_id,
                        "model": model_id,
                        "metadata": model,
                    }));
                }
            }
        }
    }
    models.sort_by(|left, right| {
        left["id"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["id"].as_str().unwrap_or_default())
    });
    Ok(models)
}

const OPENCODE_RUNTIME_TIMEOUT: Duration = Duration::from_secs(20);

#[tauri::command]
pub async fn get_opencode_runtime_models(db: State<'_, DbState>) -> Result<Vec<String>, String> {
    let (config_dir, cli_path) = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let config_dir = conn
            .query_row(
                "SELECT config_dir FROM custom_paths WHERE tool_id = 'opencode'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".opencode")))
            .ok_or("Cannot determine OpenCode config directory")?;
        (
            config_dir,
            crate::commands::extra_commands::resolve_cli_path("opencode"),
        )
    };

    tokio::task::spawn_blocking(move || run_opencode_models(&cli_path, &config_dir))
        .await
        .map_err(|error| format!("OpenCode model discovery task failed: {error}"))?
}

fn run_opencode_models(
    cli_path: &str,
    config_dir: &std::path::Path,
) -> Result<Vec<String>, String> {
    let mut command = if cfg!(target_os = "windows")
        && (cli_path.ends_with(".cmd") || cli_path.ends_with(".bat"))
    {
        let mut command = Command::new("cmd.exe");
        command.args(["/D", "/S", "/C"]);
        command.arg(format!("\"{}\" models", cli_path));
        command
    } else {
        let mut command = Command::new(cli_path);
        command.arg("models");
        command
    };
    crate::utils::configure_background_command(&mut command);
    command
        .env("OPENCODE_CONFIG_DIR", config_dir)
        .env("OPENCODE_DISABLE_PROJECT_CONFIG", "true")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Unable to run OpenCode models: {error}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut pipe) = child.stdout.take() {
                    pipe.read_to_end(&mut stdout)
                        .map_err(|error| error.to_string())?;
                }
                if let Some(mut pipe) = child.stderr.take() {
                    pipe.read_to_end(&mut stderr)
                        .map_err(|error| error.to_string())?;
                }
                if !status.success() {
                    let detail = String::from_utf8_lossy(&stderr).trim().to_string();
                    return Err(if detail.is_empty() {
                        "OpenCode model discovery failed".to_string()
                    } else {
                        format!("OpenCode model discovery failed: {detail}")
                    });
                }
                return Ok(parse_runtime_models(&String::from_utf8_lossy(&stdout)));
            }
            Ok(None) if started.elapsed() >= OPENCODE_RUNTIME_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("OpenCode model discovery timed out".to_string());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn parse_runtime_models(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let value = line.trim();
            let (provider, model) = value.split_once('/')?;
            if provider.is_empty()
                || model.is_empty()
                || !provider.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
                || model
                    .chars()
                    .any(|character| character.is_whitespace() || character.is_control())
            {
                return None;
            }
            Some(value.to_string())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_tool(value: &str) -> Result<&str, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "claude" => Ok("claude"),
        "codex" => Ok("codex"),
        "gemini" => Ok("gemini"),
        "opencode" => Ok("opencode"),
        "openclaw" => Ok("openclaw"),
        "hermes" => Ok("hermes"),
        _ => Err(format!("Unsupported app: {value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_tool, parse_runtime_models};

    #[test]
    fn validates_live_tools() {
        assert!(normalize_tool("opencode").is_ok());
        assert!(normalize_tool("pi").is_err());
    }

    #[test]
    fn runtime_models_are_sorted_deduplicated_and_validated() {
        assert_eq!(
            parse_runtime_models("openai/gpt-5\nzen/free\nopenai/gpt-5\ninvalid\nopenai/bad model"),
            vec!["openai/gpt-5".to_string(), "zen/free".to_string()]
        );
    }
}
