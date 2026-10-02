#![allow(clippy::too_many_arguments)]
use tauri::State;

use crate::db::DbState;

use super::super::config_profiles::*;
use super::super::log_command_timing;
use super::super::types::*;
use super::session_tasks::{self, SessionAccessPlan, SessionDeletePlan};
use super::*;

#[tauri::command]
pub async fn get_sessions(
    tool_id: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
    db: State<'_, DbState>,
) -> Result<Vec<SessionSummary>, String> {
    let started_at = std::time::Instant::now();
    let plan = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        prepare_session_scan(&conn, tool_id, query, limit)?
    };
    let result = session_tasks::read(move || execute_session_scan(plan)).await;
    log_command_timing("get_sessions", started_at);
    result
}

/// Lightweight session-message endpoint used by external integrations. It
/// reuses the same path allow-list and parsers as the full session detail view.
#[tauri::command(rename_all = "camelCase")]
pub async fn get_session_messages(
    provider_id: String,
    source_path: String,
    db: State<'_, DbState>,
) -> Result<Vec<SessionEntry>, String> {
    let access = {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        SessionAccessPlan::prepare(&conn, &provider_id)?
    };
    let path = std::path::Path::new(&source_path);
    let is_jsonl = path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        || (provider_id == "codex" && crate::shared::session_archive::compressed(path));
    let session = SessionSummary {
        id: path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("session")
            .to_string(),
        tool_id: provider_id.clone(),
        tool_name: tool_label(&provider_id).to_string(),
        title: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Session")
            .to_string(),
        cwd: None,
        source_kind: if is_jsonl {
            if provider_id == "codex" {
                "codex_jsonl"
            } else {
                "jsonl"
            }
            .to_string()
        } else {
            "sqlite".to_string()
        },
        source_backend: if is_jsonl {
            "jsonl".to_string()
        } else {
            "sqlite".to_string()
        },
        source_path,
        created_at: None,
        updated_at: None,
        preview: String::new(),
        message_count: 0,
        input_tokens: None,
        output_tokens: None,
        tokens_used: None,
        search_hit_count: 0,
        can_resume: false,
        can_delete: false,
    };
    session_tasks::read(move || {
        if !access.allows(&session.source_path) {
            return Err("Invalid session source path".into());
        }
        load_session_detail(&session).map(|detail| detail.entries)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn get_session_detail(
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
    let access = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        SessionAccessPlan::prepare(&conn, &tool_id)?
    };
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
    let result = session_tasks::read(move || {
        if !access.allows(&summary.source_path) {
            return Err("Invalid session source path".into());
        }
        load_session_detail(&summary)
    })
    .await;
    log_command_timing("get_session_detail", started_at);
    result
}

#[tauri::command]
pub async fn delete_session(
    tool_id: String,
    session_id: String,
    source_path: String,
    source_backend: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let permit = session_tasks::mutation_permit().await;
    let plan = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        SessionDeletePlan::prepare(
            &conn,
            SessionDeleteTarget {
                tool_id,
                session_id,
                source_path,
                source_backend,
            },
        )?
    };
    session_tasks::mutate(permit, move || plan.execute()).await
}

#[tauri::command]
pub async fn delete_sessions(
    sessions: Vec<SessionDeleteTarget>,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let permit = session_tasks::mutation_permit().await;
    let plans = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        sessions
            .into_iter()
            .map(|target| SessionDeletePlan::prepare(&conn, target))
            .collect::<Vec<_>>()
    };
    session_tasks::mutate(permit, move || {
        let mut deleted = 0;
        for plan in plans {
            plan?.execute()?;
            deleted += 1;
        }
        Ok(deleted)
    })
    .await
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
) -> Result<CodexTomlStructuredRead, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (config_path, auth_path) = resolve_codex_structured_paths(&conn, path)?;
    read_codex_structured_files(&config_path, &auth_path)
}

#[tauri::command]
pub fn write_codex_toml_structured(
    path: Option<String>,
    raw_toml: String,
    api_key: String,
    expected_revision: String,
    db: State<'_, DbState>,
) -> Result<CodexTomlStructuredWrite, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (config_path, auth_path) = resolve_codex_structured_paths(&conn, path)?;
    write_codex_structured_files(
        &config_path,
        &auth_path,
        &raw_toml,
        &api_key,
        &expected_revision,
    )
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
