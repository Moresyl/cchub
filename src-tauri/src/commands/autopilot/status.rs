// 状态推进、阶段记录、路径派生、Codex 输出流转发
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::utils;

use super::helpers::{
    find_session_id_in_value, now_epoch_ms, now_string, read_last_message_preview,
    read_optional_text, resolve_workdir, write_text_file,
};
use super::{
    AutopilotPaths, AutopilotRuntime, AutopilotStageEntry, AutopilotStatus, NativeAutopilotContext,
    AUTOPILOT_STATUS_STOPPED, MAX_STAGE_ENTRIES, SUMMARY_FILE_NAME,
};

pub(super) fn refresh_runtime_status(runtime: AutopilotRuntime) -> Result<AutopilotStatus, String> {
    let snapshot = runtime.snapshot()?;
    let Some(paths) = derive_paths_from_status(&snapshot) else {
        return Ok(snapshot);
    };

    let latest_session_id = maybe_record_session_id(&paths)?;
    let latest_preview = read_last_message_preview(&paths.last_message);
    let session_changed = latest_session_id
        .as_deref()
        .map(|value| value != snapshot.session_id)
        .unwrap_or(false);
    let preview_changed =
        !latest_preview.is_empty() && latest_preview != snapshot.last_message_preview;

    if !session_changed && !preview_changed {
        return Ok(snapshot);
    }

    runtime.update_status(|status| {
        if let Some(session_id) = latest_session_id.clone() {
            if status.session_id != session_id {
                status.session_id = session_id;
            }
        }
        if !latest_preview.is_empty() && status.last_message_preview != latest_preview {
            status.last_message_preview = latest_preview.clone();
        }
    })
}

pub(super) fn derive_paths_from_status(status: &AutopilotStatus) -> Option<AutopilotPaths> {
    if status.state_dir.trim().is_empty() {
        return None;
    }

    let state_dir = PathBuf::from(&status.state_dir);
    let log_dir = PathBuf::from(&status.log_dir);
    Some(AutopilotPaths {
        log_dir: log_dir.clone(),
        state_dir: state_dir.clone(),
        main_log: PathBuf::from(&status.main_log_path),
        event_log: state_dir.join("events.jsonl"),
        runner_log: state_dir.join("runner.log"),
        last_message: state_dir.join("last-message.txt"),
        session_id_file: state_dir.join("session-id.txt"),
        meta_file: state_dir.join("meta.json"),
        initial_prompt_file: state_dir.join("initial-prompt.txt"),
        resume_prompt_file: state_dir.join("resume-prompt.txt"),
        current_prompt_file: state_dir.join("_current_prompt.txt"),
        task_file_abs: PathBuf::from(&status.task_file),
    })
}

pub(super) fn maybe_record_session_id(paths: &AutopilotPaths) -> Result<Option<String>, String> {
    let raw = match fs::read_to_string(&paths.event_log) {
        Ok(value) => value,
        Err(_) => return Ok(read_optional_text(&paths.session_id_file)),
    };
    let mut latest: Option<String> = None;
    for line in raw.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(found) = find_session_id_in_value(&value) {
            latest = Some(found);
        }
    }
    if let Some(session_id) = latest.clone() {
        write_text_file(&paths.session_id_file, &session_id)?;
    }
    Ok(latest.or_else(|| read_optional_text(&paths.session_id_file)))
}

pub(super) fn update_last_message_preview(
    runtime: &AutopilotRuntime,
    last_message_path: &Path,
    attempt: u32,
) -> Result<String, String> {
    let preview = read_last_message_preview(last_message_path);
    if preview.is_empty() {
        return Ok(String::new());
    }
    runtime.update_status(|status| {
        status.last_message_preview = preview.clone();
        push_stage(
            status,
            "assistant_reply",
            "收到 Codex 回复摘要".to_string(),
            Some(attempt),
        );
    })?;
    Ok(preview)
}

pub(super) fn snapshot_last_message(paths: &AutopilotPaths, attempt: u32) -> Result<(), String> {
    if !paths.last_message.exists() {
        return Ok(());
    }
    let target = paths
        .state_dir
        .join(format!("attempt-{attempt:04}.last.txt"));
    fs::copy(&paths.last_message, target).map_err(|e| format!("保存回复快照失败: {e}"))?;
    Ok(())
}

pub(super) fn write_metadata(context: &NativeAutopilotContext) -> Result<(), String> {
    let workdir = resolve_workdir(&context.request.workdir, &context.paths.task_file_abs)?;
    let content = serde_json::json!({
        "task_file": context.paths.task_file_abs,
        "workdir": workdir,
        "state_dir": context.paths.state_dir,
        "log_dir": context.paths.log_dir,
        "main_log": context.paths.main_log,
        "nonce": context.nonce,
        "done_token": context.done_token,
        "codex_bin": context.codex_bin,
        "model": context.request.model,
        "profile": context.request.profile,
        "dry_run": context.request.dry_run,
    });
    let serialized =
        serde_json::to_string_pretty(&content).map_err(|e| format!("序列化元数据失败: {e}"))?;
    write_text_file(&context.paths.meta_file, &serialized)
}

pub(super) fn push_stage(
    status: &mut AutopilotStatus,
    phase: &str,
    message: String,
    attempt: Option<u32>,
) {
    status.phase = phase.to_string();
    status.message = message.clone();
    status.summary = message.clone();
    if let Some(value) = attempt {
        status.attempt = value;
    }
    let duplicated = status
        .recent_stages
        .last()
        .map(|last| last.phase == phase && last.message == message && last.attempt == attempt)
        .unwrap_or(false);
    if duplicated {
        return;
    }

    status.recent_stages.push(AutopilotStageEntry {
        at: now_string(),
        phase: phase.to_string(),
        message,
        attempt,
    });
    if status.recent_stages.len() > MAX_STAGE_ENTRIES {
        let drain_count = status.recent_stages.len() - MAX_STAGE_ENTRIES;
        status.recent_stages.drain(0..drain_count);
    }
}

pub(super) fn persist_status_summary(status: &AutopilotStatus) -> Result<(), String> {
    if status.run_dir.trim().is_empty() {
        return Ok(());
    }
    let path = Path::new(&status.run_dir).join(SUMMARY_FILE_NAME);
    let content =
        serde_json::to_string_pretty(status).map_err(|e| format!("序列化任务摘要失败: {e}"))?;
    utils::atomic_write_string(&path, &content).map_err(|e| format!("写入任务摘要失败: {e}"))?;
    Ok(())
}

pub(super) fn stream_codex_pipe<R: std::io::Read>(
    pipe: R,
    path: &Path,
    app: AppHandle,
    stream_kind: &'static str,
    last_event_ms: Arc<AtomicI64>,
    attempt: u32,
) {
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    // BufWriter 累积写入，不再每行 fsync；每 16 行或 stream 结束时 flush
    let mut writer = BufWriter::with_capacity(8 * 1024, file);
    let reader = BufReader::new(pipe);
    let mut line_count: u32 = 0;
    for line in reader.lines().map_while(Result::ok) {
        let _ = writeln!(writer, "{line}");
        line_count = line_count.saturating_add(1);
        if line_count.is_multiple_of(16) {
            let _ = writer.flush();
        }
        last_event_ms.store(now_epoch_ms(), Ordering::Relaxed);
        let payload = if stream_kind == "stdout" {
            // codex exec --json 模式下每行一个事件对象
            match serde_json::from_str::<Value>(&line) {
                Ok(value) => serde_json::json!({
                    "kind": "stdout",
                    "attempt": attempt,
                    "json": value,
                }),
                Err(_) => serde_json::json!({
                    "kind": "stdout",
                    "attempt": attempt,
                    "raw": line,
                }),
            }
        } else {
            serde_json::json!({
                "kind": "stderr",
                "attempt": attempt,
                "raw": line,
            })
        };
        let _ = app.emit("autopilot://codex-event", payload);
    }
    let _ = writer.flush();
}

pub(super) fn emit_stage(app: &AppHandle, phase: &str, message: &str, attempt: Option<u32>) {
    let _ = app.emit(
        "autopilot://stage",
        serde_json::json!({
            "phase": phase,
            "message": message,
            "attempt": attempt,
            "at_ms": now_epoch_ms(),
        }),
    );
}

pub(super) fn finalize_stopped(runtime: &AutopilotRuntime) -> Result<(), String> {
    runtime.update_status(|status| {
        status.running = false;
        status.status = AUTOPILOT_STATUS_STOPPED.to_string();
        status.finished_at = Some(now_string());
        push_stage(
            status,
            "stopped",
            "任务已停止".to_string(),
            Some(status.attempt),
        );
    })?;
    Ok(())
}

pub(super) fn sleep_with_stop(runtime: &AutopilotRuntime, interval_secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(interval_secs);
    while Instant::now() < deadline {
        if runtime.is_stop_requested() {
            return false;
        }
        thread::sleep(Duration::from_millis(250));
    }
    true
}
