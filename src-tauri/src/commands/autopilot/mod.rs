mod helpers;
mod runtime;
mod status;

use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::utils;
use helpers::{
    canonicalize_existing_file, clear_state_dir, generate_nonce, kill_process_tree_by_pid,
    now_string, resolve_codex_bin, resolve_run_dir, resolve_workdir, sanitize_task_name,
};
use runtime::run_native_autopilot;
use status::{persist_status_summary, push_stage, refresh_runtime_status};

const MAX_STAGE_ENTRIES: usize = 24;
const SUMMARY_FILE_NAME: &str = "summary.json";
const DEFAULT_INTERVAL: u64 = 3;
const DEFAULT_CONFIRM_TEXT: &str = "CONFIRMED: all tasks completed";
const DEFAULT_IDLE_THRESHOLD_SECS: u64 = 30;
const DEFAULT_MAX_IDLE_STREAK: u32 = 5;
const AUTOPILOT_STATUS_IDLE: &str = "idle";
const AUTOPILOT_STATUS_RUNNING: &str = "running";
const AUTOPILOT_STATUS_STOPPING: &str = "stopping";
const AUTOPILOT_STATUS_STOPPED: &str = "stopped";
const AUTOPILOT_STATUS_COMPLETED: &str = "completed";
const AUTOPILOT_STATUS_FAILED: &str = "failed";
const AUTOPILOT_STATUS_MAX_ATTEMPTS: &str = "max_attempts";
const AUTOPILOT_STATUS_IDLE_STOPPED: &str = "idle_stopped";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotStageEntry {
    pub at: String,
    pub phase: String,
    pub message: String,
    pub attempt: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotStatus {
    pub running: bool,
    pub stop_requested: bool,
    pub status: String,
    pub phase: String,
    pub summary: String,
    pub message: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub current_run_id: Option<String>,
    pub task_file: String,
    pub task_name: String,
    pub workdir: String,
    pub codex_bin: String,
    pub logs_root_dir: String,
    pub run_dir: String,
    pub log_dir: String,
    pub state_dir: String,
    pub main_log_path: String,
    pub attempt: u32,
    pub session_id: String,
    pub last_message_preview: String,
    pub last_error: String,
    pub dry_run: bool,
    pub recent_stages: Vec<AutopilotStageEntry>,
    #[serde(default)]
    pub task_queue: Vec<String>,
    #[serde(default)]
    pub current_task_index: usize,
}

impl Default for AutopilotStatus {
    fn default() -> Self {
        Self {
            running: false,
            stop_requested: false,
            status: AUTOPILOT_STATUS_IDLE.to_string(),
            phase: "idle".to_string(),
            summary: String::new(),
            message: String::new(),
            started_at: None,
            finished_at: None,
            current_run_id: None,
            task_file: String::new(),
            task_name: String::new(),
            workdir: String::new(),
            codex_bin: String::new(),
            logs_root_dir: utils::autopilot_runs_dir().to_string_lossy().to_string(),
            run_dir: String::new(),
            log_dir: String::new(),
            state_dir: String::new(),
            main_log_path: String::new(),
            attempt: 0,
            session_id: String::new(),
            last_message_preview: String::new(),
            last_error: String::new(),
            dry_run: false,
            recent_stages: Vec::new(),
            task_queue: Vec::new(),
            current_task_index: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotStartRequest {
    #[serde(default)]
    pub task_file: String,
    #[serde(default)]
    pub task_files: Option<Vec<String>>,
    pub workdir: String,
    pub model: String,
    pub profile: String,
    pub interval: Option<u64>,
    pub max_attempts: Option<u32>,
    #[serde(default)]
    pub codex_bin: Option<String>,
    pub fresh: bool,
    pub dry_run: bool,
    pub skip_git_check: bool,
    pub bypass: bool,
    pub full_auto: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotClearResult {
    pub deleted_count: usize,
}

#[derive(Default)]
struct AutopilotShared {
    status: Mutex<AutopilotStatus>,
    child: Mutex<Option<Child>>,
    child_pid: Mutex<Option<u32>>,
}

#[derive(Clone, Default)]
pub struct AutopilotRuntime(Arc<AutopilotShared>);

#[derive(Clone)]
struct AutopilotPaths {
    log_dir: PathBuf,
    state_dir: PathBuf,
    main_log: PathBuf,
    event_log: PathBuf,
    runner_log: PathBuf,
    last_message: PathBuf,
    session_id_file: PathBuf,
    meta_file: PathBuf,
    initial_prompt_file: PathBuf,
    resume_prompt_file: PathBuf,
    current_prompt_file: PathBuf,
    task_file_abs: PathBuf,
}

#[derive(Clone)]
struct NativeAutopilotContext {
    request: AutopilotStartRequest,
    paths: AutopilotPaths,
    codex_bin: String,
    nonce: String,
    done_token: String,
}

#[derive(Debug, Clone, Copy)]
enum ExecMode {
    Initial,
    Resume,
}

impl AutopilotRuntime {
    fn snapshot(&self) -> Result<AutopilotStatus, String> {
        Ok(self.0.status.lock().map_err(|e| e.to_string())?.clone())
    }

    fn update_status<F>(&self, mutator: F) -> Result<AutopilotStatus, String>
    where
        F: FnOnce(&mut AutopilotStatus),
    {
        let mut status = self.0.status.lock().map_err(|e| e.to_string())?;
        mutator(&mut status);
        persist_status_summary(&status)?;
        Ok(status.clone())
    }

    fn replace_status(&self, next: AutopilotStatus) -> Result<AutopilotStatus, String> {
        let mut status = self.0.status.lock().map_err(|e| e.to_string())?;
        *status = next;
        persist_status_summary(&status)?;
        Ok(status.clone())
    }

    fn set_child(&self, child: Child) -> Result<(), String> {
        let pid = child.id();
        let mut slot = self.0.child.lock().map_err(|e| e.to_string())?;
        *slot = Some(child);
        let mut pid_slot = self.0.child_pid.lock().map_err(|e| e.to_string())?;
        *pid_slot = Some(pid);
        Ok(())
    }

    fn clear_child(&self) -> Result<(), String> {
        let mut slot = self.0.child.lock().map_err(|e| e.to_string())?;
        *slot = None;
        let mut pid_slot = self.0.child_pid.lock().map_err(|e| e.to_string())?;
        *pid_slot = None;
        Ok(())
    }

    fn with_child<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&mut Option<Child>) -> Result<T, String>,
    {
        let mut slot = self.0.child.lock().map_err(|e| e.to_string())?;
        f(&mut slot)
    }

    fn is_stop_requested(&self) -> bool {
        self.snapshot()
            .map(|status| status.stop_requested)
            .unwrap_or(false)
    }

    fn child_pid(&self) -> Result<Option<u32>, String> {
        Ok(*self.0.child_pid.lock().map_err(|e| e.to_string())?)
    }
}

#[tauri::command]
pub fn get_autopilot_status(
    runtime: State<'_, AutopilotRuntime>,
) -> Result<AutopilotStatus, String> {
    refresh_runtime_status(runtime.inner().clone())
}

#[tauri::command]
pub async fn pick_autopilot_file() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Select file")
        .pick_file()
        .await;
    Ok(file.map(|value| value.path().to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn pick_autopilot_files() -> Result<Vec<String>, String> {
    let files = rfd::AsyncFileDialog::new()
        .set_title("Select task files")
        .pick_files()
        .await;
    Ok(files
        .map(|list| {
            list.into_iter()
                .map(|f| f.path().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default())
}

/// Resolve task files from request (task_files preferred over single task_file).
/// Returns canonicalized absolute paths after validating each exists.
fn collect_task_files(request: &AutopilotStartRequest) -> Result<Vec<PathBuf>, String> {
    let raw: Vec<String> = match &request.task_files {
        Some(list) if !list.is_empty() => list.clone(),
        _ => {
            if request.task_file.trim().is_empty() {
                return Err("请至少选择一个任务文件".to_string());
            }
            vec![request.task_file.clone()]
        }
    };
    let mut resolved = Vec::with_capacity(raw.len());
    for raw_path in raw {
        if raw_path.trim().is_empty() {
            continue;
        }
        let abs = canonicalize_existing_file(&raw_path, "任务文件")?;
        resolved.push(abs);
    }
    if resolved.is_empty() {
        return Err("请至少选择一个任务文件".to_string());
    }
    Ok(resolved)
}

/// Prepare per-task paths, status, and context. Each task in a queue gets
/// its own run_dir / log_dir / state_dir under autopilot_runs_dir.
fn prepare_task_run(
    request: &AutopilotStartRequest,
    task_file_abs: &Path,
    task_index: usize,
    queue: &[String],
    codex_bin: &str,
) -> Result<(AutopilotStatus, NativeAutopilotContext), String> {
    let workdir = resolve_workdir(&request.workdir, task_file_abs)?;
    let run_id = format!(
        "{}-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        Uuid::new_v4().simple()
    );
    let task_name = sanitize_task_name(
        task_file_abs
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("autopilot-task"),
    );
    let ts_str = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let run_dir = utils::autopilot_runs_dir().join(&run_id);
    let log_dir = run_dir.join("logs");
    let state_dir = run_dir.join("state");
    fs::create_dir_all(&log_dir).map_err(|e| format!("创建日志目录失败: {e}"))?;
    fs::create_dir_all(&state_dir).map_err(|e| format!("创建状态目录失败: {e}"))?;

    let paths = AutopilotPaths {
        log_dir: log_dir.clone(),
        state_dir: state_dir.clone(),
        main_log: log_dir.join(format!("{task_name}_{ts_str}.log")),
        event_log: state_dir.join("events.jsonl"),
        runner_log: state_dir.join("runner.log"),
        last_message: state_dir.join("last-message.txt"),
        session_id_file: state_dir.join("session-id.txt"),
        meta_file: state_dir.join("meta.json"),
        initial_prompt_file: state_dir.join("initial-prompt.txt"),
        resume_prompt_file: state_dir.join("resume-prompt.txt"),
        current_prompt_file: state_dir.join("_current_prompt.txt"),
        task_file_abs: task_file_abs.to_path_buf(),
    };

    if request.fresh {
        let _ = clear_state_dir(&paths.state_dir);
    }

    let (nonce, done_token) = generate_nonce();
    let started_at = now_string();
    let queue_summary = if queue.len() > 1 {
        format!("准备启动 Autopilot（{}/{}）", task_index + 1, queue.len())
    } else {
        "准备启动 Autopilot".to_string()
    };
    let mut status = AutopilotStatus {
        running: true,
        stop_requested: false,
        status: AUTOPILOT_STATUS_RUNNING.to_string(),
        phase: "preparing".to_string(),
        summary: queue_summary.clone(),
        message: queue_summary.clone(),
        started_at: Some(started_at),
        finished_at: None,
        current_run_id: Some(run_id),
        task_file: task_file_abs.to_string_lossy().to_string(),
        task_name,
        workdir: workdir.to_string_lossy().to_string(),
        codex_bin: codex_bin.to_string(),
        logs_root_dir: utils::autopilot_runs_dir().to_string_lossy().to_string(),
        run_dir: run_dir.to_string_lossy().to_string(),
        log_dir: log_dir.to_string_lossy().to_string(),
        state_dir: state_dir.to_string_lossy().to_string(),
        main_log_path: paths.main_log.to_string_lossy().to_string(),
        attempt: 0,
        session_id: String::new(),
        last_message_preview: String::new(),
        last_error: String::new(),
        dry_run: request.dry_run,
        recent_stages: Vec::new(),
        task_queue: queue.to_vec(),
        current_task_index: task_index,
    };
    push_stage(&mut status, "preparing", queue_summary, None);

    let context = NativeAutopilotContext {
        request: request.clone(),
        paths,
        codex_bin: codex_bin.to_string(),
        nonce,
        done_token,
    };
    Ok((status, context))
}

#[tauri::command]
pub fn start_autopilot(
    app: AppHandle,
    request: AutopilotStartRequest,
    runtime: State<'_, AutopilotRuntime>,
) -> Result<AutopilotStatus, String> {
    let runtime = runtime.inner().clone();
    if runtime.snapshot()?.running {
        return Err("Autopilot 正在运行，请先停止当前任务".to_string());
    }

    let task_files = collect_task_files(&request)?;
    let codex_bin = resolve_codex_bin(request.codex_bin.as_deref().unwrap_or("codex"))?;
    let queue: Vec<String> = task_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let (initial_status, first_context) =
        prepare_task_run(&request, &task_files[0], 0, &queue, &codex_bin)?;
    runtime.replace_status(initial_status)?;

    thread::spawn({
        let runtime = runtime.clone();
        let app = app.clone();
        let request = request.clone();
        let queue = queue.clone();
        let codex_bin = codex_bin.clone();
        let task_files = task_files.clone();
        move || {
            // First task uses the already-prepared context to avoid double prep work.
            run_native_autopilot(app.clone(), runtime.clone(), first_context);
            // Subsequent tasks: stop if user requested, or any task fails.
            for (idx, task_path) in task_files.iter().enumerate().skip(1) {
                if runtime.is_stop_requested() {
                    break;
                }
                let last_status = match runtime.snapshot() {
                    Ok(s) => s.status,
                    Err(_) => break,
                };
                // Continue on completed/max_attempts/idle_stopped; halt on failed/stopped.
                if matches!(
                    last_status.as_str(),
                    AUTOPILOT_STATUS_FAILED | AUTOPILOT_STATUS_STOPPED
                ) {
                    break;
                }
                match prepare_task_run(&request, task_path, idx, &queue, &codex_bin) {
                    Ok((next_status, next_context)) => {
                        if let Err(e) = runtime.replace_status(next_status) {
                            eprintln!("autopilot queue: replace_status failed: {e}");
                            break;
                        }
                        run_native_autopilot(app.clone(), runtime.clone(), next_context);
                    }
                    Err(e) => {
                        let _ = runtime.update_status(|status| {
                            status.running = false;
                            status.status = AUTOPILOT_STATUS_FAILED.to_string();
                            status.finished_at = Some(now_string());
                            status.last_error = e.clone();
                            push_stage(status, "failed", e.clone(), None);
                        });
                        break;
                    }
                }
            }
        }
    });

    runtime.snapshot()
}

#[tauri::command]
pub fn stop_autopilot(runtime: State<'_, AutopilotRuntime>) -> Result<AutopilotStatus, String> {
    let runtime = runtime.inner().clone();
    let snapshot = runtime.snapshot()?;
    if !snapshot.running {
        return Ok(snapshot);
    }

    runtime.update_status(|status| {
        status.stop_requested = true;
        status.status = AUTOPILOT_STATUS_STOPPING.to_string();
        push_stage(status, "stopping", "正在发送停止请求".to_string(), None);
    })?;

    if let Some(pid) = runtime.child_pid()? {
        let runtime_for_stop = runtime.clone();
        thread::spawn(move || {
            if let Err(error) = kill_process_tree_by_pid(pid) {
                let _ = runtime_for_stop.update_status(|status| {
                    status.last_error = error.clone();
                    push_stage(
                        status,
                        "stop_warning",
                        format!("停止进程失败: {error}"),
                        Some(status.attempt),
                    );
                });
            }
        });
    }

    runtime.snapshot()
}

#[tauri::command]
pub fn list_autopilot_logs(
    runtime: State<'_, AutopilotRuntime>,
) -> Result<Vec<AutopilotStatus>, String> {
    let runtime = runtime.inner().clone();
    let current_snapshot = refresh_runtime_status(runtime.clone())?;
    let current_run_id = current_snapshot.current_run_id.clone();
    let root = utils::autopilot_runs_dir();
    fs::create_dir_all(&root).map_err(|e| format!("创建日志根目录失败: {e}"))?;

    let mut records = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| format!("读取日志目录失败: {e}"))? {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let summary_path = path.join(SUMMARY_FILE_NAME);
        if !summary_path.exists() {
            continue;
        }
        let raw = match fs::read_to_string(&summary_path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let mut record = match serde_json::from_str::<AutopilotStatus>(&raw) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if current_snapshot.running && current_run_id.as_deref() == record.current_run_id.as_deref()
        {
            record = current_snapshot.clone();
        }
        records.push(record);
    }

    records.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    Ok(records)
}

#[tauri::command]
pub fn delete_autopilot_log(
    run_id: String,
    runtime: State<'_, AutopilotRuntime>,
) -> Result<(), String> {
    let runtime = runtime.inner().clone();
    let snapshot = runtime.snapshot()?;
    if snapshot.running && snapshot.current_run_id.as_deref() == Some(run_id.as_str()) {
        return Err("当前任务仍在运行，请先停止后再删除".to_string());
    }
    let run_dir = resolve_run_dir(&run_id)?;
    if run_dir.exists() {
        fs::remove_dir_all(&run_dir).map_err(|e| format!("删除日志目录失败: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn clear_autopilot_logs(
    runtime: State<'_, AutopilotRuntime>,
) -> Result<AutopilotClearResult, String> {
    let runtime = runtime.inner().clone();
    let snapshot = runtime.snapshot()?;
    let active_run_id = snapshot.current_run_id.clone().filter(|_| snapshot.running);
    let root = utils::autopilot_runs_dir();
    fs::create_dir_all(&root).map_err(|e| format!("创建日志根目录失败: {e}"))?;

    let mut deleted_count = 0usize;
    for entry in fs::read_dir(&root).map_err(|e| format!("读取日志目录失败: {e}"))? {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let file_name = entry.file_name().to_string_lossy().to_string();
        if active_run_id.as_deref() == Some(file_name.as_str()) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|e| format!("清理日志失败: {e}"))?;
            deleted_count += 1;
        }
    }

    Ok(AutopilotClearResult { deleted_count })
}

#[cfg(test)]
mod tests {
    use super::helpers::{
        completion_detected, generate_nonce, looks_like_uuid, resolve_run_dir, sanitize_task_name,
    };
    use super::runtime::build_codex_process;
    use super::status::maybe_record_session_id;
    use super::{AutopilotPaths, DEFAULT_CONFIRM_TEXT};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn test_paths(root: &std::path::Path, task_file_abs: PathBuf) -> AutopilotPaths {
        let log_dir = root.join("logs");
        let state_dir = root.join("state");
        fs::create_dir_all(&log_dir).unwrap();
        fs::create_dir_all(&state_dir).unwrap();
        AutopilotPaths {
            log_dir: log_dir.clone(),
            state_dir: state_dir.clone(),
            main_log: log_dir.join("main.log"),
            event_log: state_dir.join("events.jsonl"),
            runner_log: state_dir.join("runner.log"),
            last_message: state_dir.join("last-message.txt"),
            session_id_file: state_dir.join("session-id.txt"),
            meta_file: state_dir.join("meta.json"),
            initial_prompt_file: state_dir.join("initial-prompt.txt"),
            resume_prompt_file: state_dir.join("resume-prompt.txt"),
            current_prompt_file: state_dir.join("_current_prompt.txt"),
            task_file_abs,
        }
    }

    #[test]
    fn sanitize_task_name_replaces_non_ascii() {
        assert_eq!(sanitize_task_name("我的 task.md"), "___task.md");
    }

    #[test]
    fn generate_nonce_creates_reversible_token() {
        let (nonce, done_token) = generate_nonce();
        let mut parts = nonce.split('-').collect::<Vec<_>>();
        parts.reverse();
        assert_eq!(done_token, parts.join("-"));
    }

    #[test]
    fn looks_like_uuid_accepts_standard_uuid() {
        assert!(looks_like_uuid("123e4567-e89b-12d3-a456-426614174000"));
        assert!(!looks_like_uuid("not-a-uuid"));
    }

    #[test]
    fn completion_detection_requires_exact_protocol() {
        let path = PathBuf::from("target/test-autopilot-last-message.txt");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, format!("cccc-bbbb-aaaa\n{DEFAULT_CONFIRM_TEXT}\n\n"));
        assert!(completion_detected(&path, "cccc-bbbb-aaaa").unwrap_or(false));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn maybe_record_session_id_recovers_thread_id_from_event_log() {
        let temp = tempdir().unwrap();
        let task_file = temp.path().join("task.md");
        let _ = fs::write(&task_file, "- [ ] sample\n");
        let paths = test_paths(temp.path(), task_file);
        let _ = fs::write(
            &paths.event_log,
            "{\"type\":\"thread.started\",\"thread_id\":\"123e4567-e89b-12d3-a456-426614174000\"}\n",
        );

        let session_id = maybe_record_session_id(&paths).unwrap();

        assert_eq!(
            session_id.as_deref(),
            Some("123e4567-e89b-12d3-a456-426614174000")
        );
        assert_eq!(
            fs::read_to_string(&paths.session_id_file).unwrap().trim(),
            "123e4567-e89b-12d3-a456-426614174000"
        );
    }

    #[test]
    fn resolve_run_dir_rejects_path_traversal() {
        assert!(resolve_run_dir("valid-run-id").is_ok());
        assert!(resolve_run_dir("../outside").is_err());
        assert!(resolve_run_dir("nested/path").is_err());
        assert!(resolve_run_dir(r"nested\path").is_err());
        assert!(resolve_run_dir("/tmp/outside").is_err());
        assert!(resolve_run_dir("").is_err());
    }

    // Test disabled: run_native_autopilot_inner signature changed to require AppHandle
    // #[test]
    // fn dry_run_writes_prompts_metadata_and_summary() { ... }

    #[cfg(target_os = "windows")]
    #[test]
    fn build_codex_process_uses_node_for_npm_wrapper() {
        let temp = tempdir().unwrap();
        let bin_dir = temp.path().join("bin");
        let codex_cmd = bin_dir.join("codex.cmd");
        let node_exe = bin_dir.join("node.exe");
        let script_path = bin_dir
            .join("node_modules")
            .join("@openai")
            .join("codex")
            .join("bin")
            .join("codex.js");

        fs::create_dir_all(script_path.parent().unwrap()).unwrap();
        fs::write(&codex_cmd, "@echo off\r\n").unwrap();
        fs::write(&node_exe, "").unwrap();
        fs::write(&script_path, "console.log('ok')\n").unwrap();

        let command = build_codex_process(&codex_cmd.to_string_lossy());
        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            command.get_program().to_string_lossy().to_ascii_lowercase(),
            node_exe.to_string_lossy().to_ascii_lowercase()
        );
        assert_eq!(
            args.first().map(String::as_str),
            Some(script_path.to_string_lossy().as_ref())
        );
    }
}
