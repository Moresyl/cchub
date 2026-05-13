#![allow(clippy::too_many_arguments)]
use tauri::State;

use crate::db::DbState;

use super::super::config_profiles::*;
use super::super::log_command_timing;
use super::super::statusline::*;
use super::super::types::*;
use super::*;

#[tauri::command]
pub fn get_sessions(
    tool_id: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
    db: State<'_, DbState>,
) -> Result<Vec<SessionSummary>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        scan_sessions_from_conn(&conn, tool_id, query, limit)
    })();
    log_command_timing("get_sessions", started_at);
    result
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn get_session_detail(
    tool_id: String,
    session_id: String,
    source_path: String,
    source_kind: String,
    source_backend: String,
    cwd: Option<String>,
    title: String,
    preview: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    message_count: usize,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    tokens_used: Option<u64>,
    can_resume: bool,
    can_delete: bool,
    db: State<'_, DbState>,
) -> Result<SessionDetail, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if !is_valid_session_source_path(&conn, &tool_id, &source_path) {
            return Err("Invalid session source path".to_string());
        }

        let summary = SessionSummary {
            id: session_id,
            tool_id: tool_id.clone(),
            tool_name: tool_label(&tool_id).to_string(),
            title,
            cwd,
            source_kind,
            source_backend,
            source_path,
            created_at,
            updated_at,
            preview,
            message_count,
            input_tokens,
            output_tokens,
            tokens_used,
            search_hit_count: 0,
            can_resume,
            can_delete,
        };
        load_session_detail(&summary)
    })();
    log_command_timing("get_session_detail", started_at);
    result
}

#[tauri::command]
pub fn delete_session(
    tool_id: String,
    session_id: String,
    source_path: String,
    source_backend: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    delete_session_impl(&conn, &tool_id, &session_id, &source_path, &source_backend)
}

#[tauri::command]
pub fn delete_sessions(
    sessions: Vec<SessionDeleteTarget>,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut deleted = 0usize;

    for session in sessions {
        delete_session_impl(
            &conn,
            &session.tool_id,
            &session.session_id,
            &session.source_path,
            &session.source_backend,
        )?;
        deleted += 1;
    }

    Ok(deleted)
}

/// Write a tool's config file content
#[tauri::command]
pub fn write_tool_config(
    tool_id: String,
    content: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    apply_tool_snapshot(&conn, &tool_id, &content)?;
    crate::utils::append_runtime_log(
        "info",
        "tools",
        &format!("Updated tool config for {tool_id}"),
    );
    Ok(())
}

#[tauri::command]
pub fn read_codex_toml_structured(
    path: Option<String>,
    db: State<'_, DbState>,
) -> Result<CodexTomlStructuredConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (config_path, auth_path) = resolve_codex_structured_paths(&conn, path)?;
    let content = if config_path.exists() {
        std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let auth = read_json_file_or_default(&auth_path)?;
    let api_key = auth
        .get("OPENAI_API_KEY")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    Ok(read_codex_structured_config_from_content(&content, api_key))
}

#[tauri::command]
pub fn write_codex_toml_structured(
    path: Option<String>,
    raw_toml: String,
    config: CodexTomlStructuredConfig,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (config_path, auth_path) = resolve_codex_structured_paths(&conn, path)?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let written_toml = write_codex_structured_config_to_text(&raw_toml, &config);
    crate::utils::atomic_write_string(&config_path, &written_toml).map_err(|e| e.to_string())?;

    let mut auth = read_json_file_or_default(&auth_path)?;
    if !auth.is_object() {
        auth = serde_json::json!({});
    }
    if let Some(api_key) = normalized_non_empty(&config.api_key) {
        auth["OPENAI_API_KEY"] = serde_json::json!(api_key);
    } else if let Some(auth_obj) = auth.as_object_mut() {
        auth_obj.remove("OPENAI_API_KEY");
    }
    write_json_file_pretty(&auth_path, &auth)?;

    Ok(written_toml)
}

#[tauri::command]
pub fn get_common_config_snippet(
    tool_id: String,
    db: State<'_, DbState>,
) -> Result<CommonConfigSnippet, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_common_config_snippet_from_conn(&conn, &tool_id)
}

#[tauri::command]
pub fn set_common_config_snippet(
    tool_id: String,
    snippet: CommonConfigSnippet,
    db: State<'_, DbState>,
) -> Result<CommonConfigSnippet, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    write_common_config_snippet_to_conn(&conn, &tool_id, snippet)
}

#[tauri::command]
pub fn read_claude_config_toggles(db: State<'_, DbState>) -> Result<ClaudeConfigToggles, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    read_claude_config_toggles_from_conn(&conn)
}

#[tauri::command]
pub fn write_claude_config_toggle(
    key: String,
    enabled: bool,
    db: State<'_, DbState>,
) -> Result<ClaudeConfigToggles, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    write_claude_config_toggle_to_conn(&conn, &key, enabled)
}

/// Get Claude Code permissions level (0=strict, 1=standard, 2=relaxed, 3=bypass)
#[tauri::command]
pub fn get_claude_permissions_level() -> Result<u32, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok(0);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let mode = settings
        .get("permissions")
        .and_then(|p| p.get("defaultMode"))
        .and_then(|m| m.as_str())
        .unwrap_or("");

    if mode == "bypassPermissions" {
        return Ok(3);
    }

    let allow = settings
        .get("permissions")
        .and_then(|p| p.get("allow"))
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    // NOTE: level 3 is already short-circuited above via `mode == "bypassPermissions"`.
    // The setter writes level 2 with Write(*) but NOT Bash(*), so checking both
    // here misses level 2 and falsely reports it as level 1. Use Write(*) alone.
    if allow.contains(&"Write(*)") {
        Ok(2)
    } else if allow.contains(&"Read(*)") {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Set Claude Code permissions level (0=strict, 1=standard, 2=relaxed, 3=bypass)
#[tauri::command]
pub fn set_claude_permissions_level(level: u32) -> Result<u32, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");

    let mut settings: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };

    let (allow, mode, skip_prompt): (Vec<&str>, &str, bool) = match level {
        0 => (vec![], "normal", false),
        1 => (
            vec!["Read(*)", "Glob(*)", "Grep(*)", "WebSearch(*)"],
            "normal",
            false,
        ),
        2 => (
            vec![
                "Read(*)",
                "Write(*)",
                "Edit(*)",
                "Glob(*)",
                "Grep(*)",
                "WebFetch(*)",
                "WebSearch(*)",
                "Agent(*)",
                "NotebookEdit(*)",
            ],
            "normal",
            false,
        ),
        3 => (
            vec![
                "Bash(*)",
                "Read(*)",
                "Write(*)",
                "Edit(*)",
                "Glob(*)",
                "Grep(*)",
                "WebFetch(*)",
                "WebSearch(*)",
                "Agent(*)",
                "NotebookEdit(*)",
                "Skill(*)",
                "mcp__*",
            ],
            "bypassPermissions",
            true,
        ),
        _ => return Err("Invalid level".to_string()),
    };

    let allow_arr: Vec<serde_json::Value> = allow.iter().map(|s| serde_json::json!(s)).collect();
    settings["permissions"] = serde_json::json!({
        "allow": allow_arr,
        "deny": [],
        "defaultMode": mode,
    });
    settings["skipDangerousModePermissionPrompt"] = serde_json::json!(skip_prompt);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(level)
}

/// Get Claude Code auto-update channel
#[tauri::command]
pub fn get_claude_auto_update() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok("latest".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    // Canonical disable: env.DISABLE_AUTOUPDATER = "1". Check this first, since
    // Claude Code's autoUpdatesChannel only accepts "stable"/"latest" — there is
    // no "disabled" channel value, so removing the key alone round-trips back
    // to the default ("latest").
    let disabled = settings
        .get("env")
        .and_then(|e| e.get("DISABLE_AUTOUPDATER"))
        .and_then(|v| v.as_str())
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if disabled {
        return Ok("disabled".to_string());
    }
    Ok(settings
        .get("autoUpdatesChannel")
        .and_then(|v| v.as_str())
        .unwrap_or("latest")
        .to_string())
}

/// Set Claude Code auto-update channel
#[tauri::command]
pub fn set_claude_auto_update(channel: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };
    if channel == "disabled" {
        // Canonical disable: env.DISABLE_AUTOUPDATER = "1". Also clear the
        // channel key so the getter has a single source of truth.
        if !settings.is_object() {
            settings = serde_json::json!({});
        }
        let obj = ensure_json_object(&mut settings);
        obj.remove("autoUpdatesChannel");
        let env_entry = obj
            .entry("env".to_string())
            .or_insert_with(|| serde_json::json!({}));
        ensure_json_object(env_entry)
            .insert("DISABLE_AUTOUPDATER".to_string(), serde_json::json!("1"));
    } else {
        // Re-enable: clear the env flag (if present) and write the channel.
        if let Some(env_obj) = settings.get_mut("env").and_then(|v| v.as_object_mut()) {
            env_obj.remove("DISABLE_AUTOUPDATER");
        }
        // Drop an empty env object to keep settings.json tidy.
        if settings
            .get("env")
            .and_then(|v| v.as_object())
            .map(|o| o.is_empty())
            .unwrap_or(false)
        {
            settings.as_object_mut().map(|o| o.remove("env"));
        }
        settings["autoUpdatesChannel"] = serde_json::json!(channel);
    }
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(channel)
}

/// Get Codex CLI settings (approval_mode, reasoning_effort, disable_response_storage)
#[tauri::command]
pub fn get_codex_settings() -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".codex").join("config.toml");
    if !path.exists() {
        return Ok(serde_json::json!({
            "approval_mode": "suggest",
            "reasoning_effort": "medium",
            "disable_response_storage": false,
            "context_window_1m": false,
        }));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // NOTE: `content.parse::<toml::Value>()` is broken in toml 1.0 — the
    // `FromStr for Value` impl only parses a single TOML value expression,
    // not a whole document, so it fails on any real config.toml with
    // "unexpected content, expected nothing". Parse as `toml::Table` instead.
    let doc: toml::Table = content
        .parse()
        .map_err(|e: toml::de::Error| e.to_string())?;

    // Read approval mode from personality or dedicated field
    let personality = doc
        .get("personality")
        .and_then(|v| v.as_str())
        .unwrap_or("pragmatic");
    let approval_mode = if personality == "full-auto" {
        "full-auto"
    } else if personality == "auto-edit" {
        "auto-edit"
    } else {
        "suggest"
    };

    let reasoning = doc
        .get("model_reasoning_effort")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");
    let disable_storage = doc
        .get("disable_response_storage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let context_window_1m = doc
        .get("model_context_window")
        .and_then(|v| v.as_integer())
        .is_some_and(|value| value == 1_000_000);

    Ok(serde_json::json!({
        "approval_mode": approval_mode,
        "reasoning_effort": reasoning,
        "disable_response_storage": disable_storage,
        "context_window_1m": context_window_1m,
    }))
}

/// Set a Codex CLI setting
#[tauri::command]
pub fn set_codex_setting(key: String, value: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".codex").join("config.toml");

    let content = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;

    match key.as_str() {
        "approval_mode" => {
            // Codex doesn't have approval_mode directly, map to personality
            doc["personality"] = toml_edit::value(&value);
        }
        "reasoning_effort" => {
            doc["model_reasoning_effort"] = toml_edit::value(&value);
        }
        "disable_response_storage" => {
            doc["disable_response_storage"] = toml_edit::value(value == "true");
        }
        "context_window_1m" => {
            if value == "true" {
                doc["model_context_window"] = toml_edit::value(1_000_000);
            } else {
                doc.remove("model_context_window");
            }
        }
        _ => return Err(format!("Unknown setting: {}", key)),
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::utils::atomic_write_string(&path, &doc.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get Claude Code model setting
#[tauri::command]
pub fn get_claude_model() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok("".to_string());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(settings
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

/// Set Claude Code model
#[tauri::command]
pub fn set_claude_model(model: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if path.exists() {
        let c = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&c).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({})
    };
    settings["model"] = serde_json::json!(model);
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    crate::utils::atomic_write_string(&path, &content).map_err(|e| e.to_string())?;
    Ok(settings["model"].as_str().unwrap_or_default().to_string())
}

/// Get Claude Code Tool Search (ENABLE_TOOL_SEARCH) status from settings.local.json
#[tauri::command]
pub fn get_claude_tool_search(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(read_claude_config_toggles_from_conn(&conn)?.enable_tool_search)
}

/// Set Claude Code Tool Search (ENABLE_TOOL_SEARCH) in settings.local.json
#[tauri::command]
pub fn set_claude_tool_search(enabled: bool, db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let _ = write_claude_config_toggle_to_conn(&conn, "enableToolSearch", enabled)?;
    Ok(enabled)
}
