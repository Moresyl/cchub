use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::State;
use uuid::Uuid;

use crate::utils;

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
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotStartRequest {
    pub task_file: String,
    pub workdir: String,
    pub model: String,
    pub profile: String,
    pub interval: Option<u64>,
    pub max_attempts: Option<u32>,
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
        let mut slot = self.0.child.lock().map_err(|e| e.to_string())?;
        *slot = Some(child);
        Ok(())
    }

    fn clear_child(&self) -> Result<(), String> {
        let mut slot = self.0.child.lock().map_err(|e| e.to_string())?;
        *slot = None;
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
}

#[tauri::command]
pub fn get_autopilot_status(runtime: State<'_, AutopilotRuntime>) -> Result<AutopilotStatus, String> {
    runtime.inner().clone().snapshot()
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
pub fn start_autopilot(
    request: AutopilotStartRequest,
    runtime: State<'_, AutopilotRuntime>,
) -> Result<AutopilotStatus, String> {
    let runtime = runtime.inner().clone();
    if runtime.snapshot()?.running {
        return Err("Autopilot 正在运行，请先停止当前任务".to_string());
    }

    let task_file = canonicalize_existing_file(&request.task_file, "任务文件")?;
    let workdir = resolve_workdir(&request.workdir, &task_file)?;
    let codex_bin = resolve_codex_bin(request.codex_bin.as_deref().unwrap_or("codex"))?;
    let run_id = format!(
        "{}-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        Uuid::new_v4().simple()
    );
    let task_name = sanitize_task_name(
        task_file
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
        task_file_abs: task_file.clone(),
    };

    if request.fresh {
        let _ = clear_state_dir(&paths.state_dir);
    }

    let (nonce, done_token) = generate_nonce();
    let started_at = now_string();
    let mut initial_status = AutopilotStatus {
        running: true,
        stop_requested: false,
        status: AUTOPILOT_STATUS_RUNNING.to_string(),
        phase: "preparing".to_string(),
        summary: "准备启动 Autopilot".to_string(),
        message: "准备启动 Autopilot".to_string(),
        started_at: Some(started_at),
        finished_at: None,
        current_run_id: Some(run_id),
        task_file: task_file.to_string_lossy().to_string(),
        task_name,
        workdir: workdir.to_string_lossy().to_string(),
        codex_bin: codex_bin.clone(),
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
    };
    push_stage(
        &mut initial_status,
        "preparing",
        "准备启动原生 Autopilot".to_string(),
        None,
    );
    runtime.replace_status(initial_status)?;

    let context = NativeAutopilotContext {
        request,
        paths,
        codex_bin,
        nonce,
        done_token,
    };
    thread::spawn({
        let runtime = runtime.clone();
        move || run_native_autopilot(runtime, context)
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
        push_stage(status, "stopping", "正在停止任务".to_string(), None);
    })?;

    runtime.with_child(|slot| {
        if let Some(child) = slot.as_mut() {
            kill_process_tree(child)?;
        }
        Ok(())
    })?;

    runtime.snapshot()
}

#[tauri::command]
pub fn list_autopilot_logs(runtime: State<'_, AutopilotRuntime>) -> Result<Vec<AutopilotStatus>, String> {
    let runtime = runtime.inner().clone();
    let current_snapshot = runtime.snapshot()?;
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
        if current_snapshot.running && current_run_id.as_deref() == record.current_run_id.as_deref() {
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

fn run_native_autopilot(runtime: AutopilotRuntime, context: NativeAutopilotContext) {
    if let Err(error) = run_native_autopilot_inner(&runtime, &context) {
        let _ = append_main_log(&context.paths.main_log, "ERROR", &error);
        let _ = runtime.update_status(|status| {
            status.running = false;
            status.status = AUTOPILOT_STATUS_FAILED.to_string();
            status.finished_at = Some(now_string());
            status.last_error = error.clone();
            push_stage(status, "failed", error.clone(), Some(status.attempt));
        });
    }
    let _ = runtime.clear_child();
}

fn run_native_autopilot_inner(
    runtime: &AutopilotRuntime,
    context: &NativeAutopilotContext,
) -> Result<(), String> {
    let task_content = fs::read_to_string(&context.paths.task_file_abs)
        .map_err(|e| format!("读取任务文档失败: {e}"))?;
    if task_content.trim().is_empty() {
        return Err("任务文档内容为空".to_string());
    }

    let initial_prompt = build_initial_prompt(context, &task_content);
    let resume_prompt = build_resume_prompt(context);
    write_text_file(&context.paths.initial_prompt_file, &initial_prompt)?;
    write_text_file(&context.paths.resume_prompt_file, &resume_prompt)?;
    write_metadata(context)?;

    append_main_log(&context.paths.main_log, "INFO", "原生 Autopilot 已初始化")?;
    append_main_log(
        &context.paths.main_log,
        "INFO",
        &format!("任务文档: {}", context.paths.task_file_abs.display()),
    )?;
    append_main_log(
        &context.paths.main_log,
        "INFO",
        &format!("工作目录: {}", runtime.snapshot()?.workdir),
    )?;

    runtime.update_status(|status| {
        push_stage(status, "running", "开始执行任务".to_string(), None);
    })?;

    if context.request.dry_run {
        append_main_log(&context.paths.main_log, "INFO", "Dry Run 完成，未执行 Codex")?;
        runtime.update_status(|status| {
            status.running = false;
            status.status = AUTOPILOT_STATUS_COMPLETED.to_string();
            status.finished_at = Some(now_string());
            push_stage(status, "completed", "Dry Run 已完成".to_string(), None);
        })?;
        return Ok(());
    }

    let mut session_id = maybe_record_session_id(&context.paths)?;
    if let Some(existing) = session_id.clone() {
        runtime.update_status(|status| {
            status.session_id = existing;
            push_stage(status, "resume_ready", "检测到旧会话，可继续续跑".to_string(), None);
        })?;
    }

    let mut attempt: u32 = 0;
    let mut idle_streak: u32 = 0;
    let interval_secs = context.request.interval.unwrap_or(DEFAULT_INTERVAL);

    loop {
        if runtime.is_stop_requested() {
            finalize_stopped(runtime)?;
            return Ok(());
        }

        attempt += 1;
        if let Some(max_attempts) = context.request.max_attempts {
            if max_attempts > 0 && attempt > max_attempts {
                append_main_log(
                    &context.paths.main_log,
                    "WARN",
                    &format!("达到最大尝试次数 ({max_attempts})，停止"),
                )?;
                runtime.update_status(|status| {
                    status.running = false;
                    status.status = AUTOPILOT_STATUS_MAX_ATTEMPTS.to_string();
                    status.finished_at = Some(now_string());
                    push_stage(
                        status,
                        "max_attempts",
                        "达到最大尝试次数，任务已停止".to_string(),
                        Some(status.attempt),
                    );
                })?;
                return Ok(());
            }
        }

        runtime.update_status(|status| {
            push_stage(status, "attempt", format!("开始第 {attempt} 轮执行"), Some(attempt));
        })?;

        let round_start = Instant::now();
        let exec_mode = if attempt == 1 || session_id.is_none() {
            ExecMode::Initial
        } else {
            ExecMode::Resume
        };
        let exec_result =
            run_codex_round(runtime, context, exec_mode, session_id.as_deref(), attempt)?;
        let round_elapsed = round_start.elapsed();

        snapshot_last_message(&context.paths, attempt)?;
        session_id = maybe_record_session_id(&context.paths)?;
        if let Some(current_session_id) = session_id.clone() {
            runtime.update_status(|status| {
                if status.session_id != current_session_id {
                    status.session_id = current_session_id.clone();
                    push_stage(
                        status,
                        "session_bound",
                        "已绑定新的会话 ID".to_string(),
                        Some(attempt),
                    );
                }
            })?;
        }

        if completion_detected(&context.paths.last_message, &context.done_token)? {
            append_main_log(
                &context.paths.main_log,
                "INFO",
                "完成协议校验通过！所有任务已完成",
            )?;
            runtime.update_status(|status| {
                status.running = false;
                status.status = AUTOPILOT_STATUS_COMPLETED.to_string();
                status.finished_at = Some(now_string());
                push_stage(
                    status,
                    "completed",
                    "完成协议校验通过，任务已完成".to_string(),
                    Some(attempt),
                );
            })?;
            return Ok(());
        }

        let preview = update_last_message_preview(runtime, &context.paths.last_message, attempt)?;

        if runtime.is_stop_requested() {
            finalize_stopped(runtime)?;
            return Ok(());
        }

        if round_elapsed.as_secs() < DEFAULT_IDLE_THRESHOLD_SECS {
            idle_streak += 1;
            append_main_log(
                &context.paths.main_log,
                "WARN",
                &format!(
                    "本轮仅耗时 {}s（阈值 {}s），疑似空转 [{idle_streak}/{}]",
                    round_elapsed.as_secs(),
                    DEFAULT_IDLE_THRESHOLD_SECS,
                    DEFAULT_MAX_IDLE_STREAK
                ),
            )?;
            runtime.update_status(|status| {
                push_stage(
                    status,
                    "idle_warning",
                    "检测到疑似空转，本轮结果将继续观察".to_string(),
                    Some(attempt),
                );
            })?;

            if idle_streak >= DEFAULT_MAX_IDLE_STREAK {
                let summary = "检测到连续空转，已自动停止".to_string();
                append_main_log(&context.paths.main_log, "ERROR", "连续空转达到阈值，自动停止")?;
                runtime.update_status(|status| {
                    status.running = false;
                    status.status = AUTOPILOT_STATUS_IDLE_STOPPED.to_string();
                    status.finished_at = Some(now_string());
                    status.last_error = if preview.is_empty() {
                        summary.clone()
                    } else {
                        format!("{summary}。最近回复: {preview}")
                    };
                    push_stage(status, "idle_stopped", summary, Some(attempt));
                })?;
                return Ok(());
            }
        } else {
            idle_streak = 0;
        }

        append_main_log(
            &context.paths.main_log,
            "INFO",
            &format!(
                "第 {attempt} 轮未完成 (exit={}), {}s 后续跑",
                exec_result.return_code, interval_secs
            ),
        )?;
        runtime.update_status(|status| {
            push_stage(
                status,
                "waiting_retry",
                format!("第 {attempt} 轮未完成，等待下次续跑"),
                Some(attempt),
            );
        })?;

        if !sleep_with_stop(runtime, interval_secs) {
            finalize_stopped(runtime)?;
            return Ok(());
        }
    }
}

fn run_codex_round(
    runtime: &AutopilotRuntime,
    context: &NativeAutopilotContext,
    requested_mode: ExecMode,
    session_id: Option<&str>,
    attempt: u32,
) -> Result<ExecRoundResult, String> {
    let (mode, prompt, workdir, mut command) =
        build_codex_command(context, requested_mode, session_id)?;
    write_text_file(&context.paths.current_prompt_file, &prompt)?;
    let short_prompt = format!(
        "Read the file `{}` and execute every instruction in it exactly.",
        context.paths.current_prompt_file.display()
    );
    command.arg(short_prompt);
    command.current_dir(&workdir);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    utils::configure_background_command(&mut command);

    let phase_message = match mode {
        ExecMode::Initial => "正在启动新的 Codex 会话".to_string(),
        ExecMode::Resume => "正在续跑现有会话".to_string(),
    };
    runtime.update_status(|status| {
        push_stage(
            status,
            match mode {
                ExecMode::Initial => "starting_session",
                ExecMode::Resume => "resuming_session",
            },
            phase_message,
            Some(attempt),
        );
    })?;

    append_main_log(
        &context.paths.main_log,
        "RUN",
        match mode {
            ExecMode::Initial => "启动 Codex 新会话...",
            ExecMode::Resume => "续跑当前会话...",
        },
    )?;
    if context.request.verbose {
        append_main_log(
            &context.paths.main_log,
            "DEBUG",
            &format!("执行命令: {:?}", command),
        )?;
    }

    let mut child = command.spawn().map_err(|e| format!("启动 Codex 失败: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    runtime.set_child(child)?;

    let out_handle = stdout.map(|pipe| {
        let path = context.paths.event_log.clone();
        thread::spawn(move || stream_pipe_to_file(pipe, &path))
    });
    let err_handle = stderr.map(|pipe| {
        let path = context.paths.runner_log.clone();
        thread::spawn(move || stream_pipe_to_file(pipe, &path))
    });

    let return_code = runtime.with_child(|slot| {
        if let Some(child) = slot.as_mut() {
            child
                .wait()
                .map(|status| status.code().unwrap_or(-1))
                .map_err(|e| format!("等待 Codex 退出失败: {e}"))
        } else {
            Err("Codex 子进程句柄丢失".to_string())
        }
    })?;

    let _ = runtime.clear_child();
    if let Some(handle) = out_handle {
        let _ = handle.join();
    }
    if let Some(handle) = err_handle {
        let _ = handle.join();
    }

    if return_code != 0 && !runtime.is_stop_requested() {
        append_main_log(
            &context.paths.main_log,
            "WARN",
            &format!("Codex 退出码: {return_code}"),
        )?;
    }

    Ok(ExecRoundResult { return_code })
}

#[derive(Debug, Clone, Copy)]
struct ExecRoundResult {
    return_code: i32,
}

fn build_codex_command(
    context: &NativeAutopilotContext,
    requested_mode: ExecMode,
    session_id: Option<&str>,
) -> Result<(ExecMode, String, PathBuf, Command), String> {
    let workdir = resolve_workdir(&context.request.workdir, &context.paths.task_file_abs)?;
    let initial_prompt = fs::read_to_string(&context.paths.initial_prompt_file)
        .map_err(|e| format!("读取初始提示词失败: {e}"))?;
    let resume_prompt = fs::read_to_string(&context.paths.resume_prompt_file)
        .map_err(|e| format!("读取续跑提示词失败: {e}"))?;

    let mut command = Command::new(&context.codex_bin);
    command.arg("exec");

    let actual_mode = match requested_mode {
        ExecMode::Resume if session_id.is_some() => {
            command.arg("resume");
            ExecMode::Resume
        }
        _ => ExecMode::Initial,
    };

    command.arg("--json");
    command.arg("-o");
    command.arg(&context.paths.last_message);

    if context.request.bypass {
        command.arg("--dangerously-bypass-approvals-and-sandbox");
    } else if context.request.full_auto {
        command.arg("--full-auto");
    }
    if context.request.skip_git_check {
        command.arg("--skip-git-repo-check");
    }
    if let Some(model) = normalize_optional(&context.request.model) {
        command.arg("-m");
        command.arg(model);
    }
    if let Some(profile) = normalize_optional(&context.request.profile) {
        command.arg("--profile");
        command.arg(profile);
    }

    match actual_mode {
        ExecMode::Initial => {
            command.arg("-C");
            command.arg(&workdir);
            Ok((actual_mode, initial_prompt, workdir, command))
        }
        ExecMode::Resume => {
            command.arg(session_id.unwrap_or_default());
            Ok((actual_mode, resume_prompt, workdir, command))
        }
    }
}

fn build_initial_prompt(context: &NativeAutopilotContext, task_content: &str) -> String {
    format!(
        "You are continuing implementation work inside the repository at `{}`.\n\nPrimary objective:\n- Read the task document below carefully. It describes features, improvements, or fixes to implement.\n- The document may use any format: checkboxes ([ ] / [x]), numbered lists, headings, bullet points, etc.\n- Implement every item that has NOT yet been completed in the codebase.\n- If the document uses checkboxes, treat [x] as done and [ ] as remaining. Otherwise, judge completion by whether the described feature/fix already exists in the code.\n- Update `{}` in place as you make progress (e.g., mark items as done, add notes).\n- Do not stop after a summary, one feature, or one phase. Keep going until every feasible item is implemented and verified.\n- Run relevant verification commands after meaningful batches of changes.\n- Do not claim completion merely because the document contains historical progress entries.\n- Respect any explicit exclusions noted in the document.\n- Do not revert unrelated existing changes.\n\nTask document follows (the file itself remains the source of truth):\n\n{}\n\n{}",
        resolve_workdir(&context.request.workdir, &context.paths.task_file_abs)
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()),
        context.paths.task_file_abs.display(),
        task_content,
        completion_protocol(context)
    )
}

fn build_resume_prompt(context: &NativeAutopilotContext) -> String {
    format!(
        "You must respond to this message. Continue any unfinished user-requested work immediately from the current state. Do not restart. Do not summarize. Do not ask for confirmation. If all requested work is already complete, follow the completion protocol below.\n\n{}",
        completion_protocol(context)
    )
}

fn completion_protocol(context: &NativeAutopilotContext) -> String {
    format!(
        "When using the completion protocol, reply with EXACTLY two lines and nothing else: line 1 = same groups in reverse order for nonce `{}`; line 2 = `{}`.",
        context.nonce, DEFAULT_CONFIRM_TEXT
    )
}

fn completion_detected(last_message_path: &Path, done_token: &str) -> Result<bool, String> {
    let content = fs::read_to_string(last_message_path).unwrap_or_default();
    let lines = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Ok(
        lines.len() == 2
            && lines.first().copied().unwrap_or_default() == done_token
            && lines.get(1).copied().unwrap_or_default() == DEFAULT_CONFIRM_TEXT,
    )
}

fn maybe_record_session_id(paths: &AutopilotPaths) -> Result<Option<String>, String> {
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

fn find_session_id_in_value(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["session_id", "conversation_id", "thread_id"] {
                if let Some(candidate) = map.get(key).and_then(Value::as_str) {
                    if looks_like_uuid(candidate) {
                        return Some(candidate.to_string());
                    }
                }
            }
            for child in map.values() {
                if let Some(found) = find_session_id_in_value(child) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_session_id_in_value),
        _ => None,
    }
}

fn update_last_message_preview(
    runtime: &AutopilotRuntime,
    last_message_path: &Path,
    attempt: u32,
) -> Result<String, String> {
    let preview = fs::read_to_string(last_message_path)
        .unwrap_or_default()
        .trim()
        .chars()
        .take(300)
        .collect::<String>()
        .replace('\n', " ↵ ");
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

fn snapshot_last_message(paths: &AutopilotPaths, attempt: u32) -> Result<(), String> {
    if !paths.last_message.exists() {
        return Ok(());
    }
    let target = paths.state_dir.join(format!("attempt-{attempt:04}.last.txt"));
    fs::copy(&paths.last_message, target).map_err(|e| format!("保存回复快照失败: {e}"))?;
    Ok(())
}

fn write_metadata(context: &NativeAutopilotContext) -> Result<(), String> {
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

fn push_stage(status: &mut AutopilotStatus, phase: &str, message: String, attempt: Option<u32>) {
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

fn persist_status_summary(status: &AutopilotStatus) -> Result<(), String> {
    if status.run_dir.trim().is_empty() {
        return Ok(());
    }
    let path = Path::new(&status.run_dir).join(SUMMARY_FILE_NAME);
    let content =
        serde_json::to_string_pretty(status).map_err(|e| format!("序列化任务摘要失败: {e}"))?;
    utils::atomic_write_string(&path, &content).map_err(|e| format!("写入任务摘要失败: {e}"))?;
    Ok(())
}

fn append_main_log(path: &Path, level: &str, message: &str) -> Result<(), String> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建日志目录失败: {e}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("写入主日志失败: {e}"))?;
    file.write_all(format!("{timestamp} [{level}] {message}\n").as_bytes())
        .map_err(|e| format!("写入主日志失败: {e}"))?;
    file.flush().map_err(|e| format!("写入主日志失败: {e}"))
}

fn stream_pipe_to_file<R: std::io::Read>(pipe: R, path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let reader = BufReader::new(pipe);
    for line in reader.lines().map_while(Result::ok) {
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
}

fn finalize_stopped(runtime: &AutopilotRuntime) -> Result<(), String> {
    runtime.update_status(|status| {
        status.running = false;
        status.status = AUTOPILOT_STATUS_STOPPED.to_string();
        status.finished_at = Some(now_string());
        push_stage(status, "stopped", "任务已停止".to_string(), Some(status.attempt));
    })?;
    Ok(())
}

fn sleep_with_stop(runtime: &AutopilotRuntime, interval_secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(interval_secs);
    while Instant::now() < deadline {
        if runtime.is_stop_requested() {
            return false;
        }
        thread::sleep(Duration::from_millis(250));
    }
    true
}

fn resolve_codex_bin(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Codex 命令不能为空".to_string());
    }

    let direct_path = PathBuf::from(trimmed);
    if direct_path.components().count() > 1 || direct_path.is_absolute() {
        return direct_path
            .canonicalize()
            .map(|path| path.to_string_lossy().to_string())
            .map_err(|e| format!("找不到 Codex: {trimmed} ({e})"));
    }

    let path_env = env::var_os("PATH").unwrap_or_default();
    let path_dirs = env::split_paths(&path_env);
    let extensions: Vec<String> = if cfg!(windows) {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .map(|item| item.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|item| !item.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    for dir in path_dirs {
        let candidate = dir.join(trimmed);
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().to_string());
        }
        if cfg!(windows) {
            for ext in &extensions {
                let candidate = dir.join(format!("{trimmed}.{ext}"));
                if candidate.is_file() {
                    return Ok(candidate.to_string_lossy().to_string());
                }
            }
        }
    }

    Err(format!("找不到 Codex: {trimmed}（请确认已安装并在 PATH 中）"))
}

fn canonicalize_existing_file(path: &str, label: &str) -> Result<PathBuf, String> {
    let original = PathBuf::from(path.trim());
    if original.as_os_str().is_empty() {
        return Err(format!("{label} 路径不能为空"));
    }
    if !original.exists() {
        return Err(format!("{label}不存在: {}", original.display()));
    }
    if !original.is_file() {
        return Err(format!("{label}不是文件: {}", original.display()));
    }
    original
        .canonicalize()
        .map_err(|e| format!("解析{label}路径失败: {e}"))
}

fn resolve_workdir(value: &str, task_file: &Path) -> Result<PathBuf, String> {
    let raw = if value.trim().is_empty() {
        task_file
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(value.trim())
    };
    if !raw.exists() {
        return Err(format!("工作目录不存在: {}", raw.display()));
    }
    if !raw.is_dir() {
        return Err(format!("工作目录不是文件夹: {}", raw.display()));
    }
    raw.canonicalize()
        .map_err(|e| format!("解析工作目录失败: {e}"))
}

fn resolve_run_dir(run_id: &str) -> Result<PathBuf, String> {
    let trimmed = run_id.trim();
    if trimmed.is_empty() {
        return Err("日志记录 ID 不能为空".to_string());
    }
    let path = Path::new(trimmed);
    if path.components().any(|component| !matches!(component, Component::Normal(_))) {
        return Err("非法的日志记录 ID".to_string());
    }
    Ok(utils::autopilot_runs_dir().join(trimmed))
}

fn sanitize_task_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.trim_matches('_').is_empty() {
        "autopilot-task".to_string()
    } else {
        sanitized
    }
}

fn write_text_file(path: &Path, content: &str) -> Result<(), String> {
    utils::atomic_write_string(path, content)
        .map_err(|e| format!("写入文件失败 {}: {e}", path.display()))
}

fn read_optional_text(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn clear_state_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|e| format!("创建状态目录失败: {e}"))?;
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|e| format!("读取状态目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取状态目录失败: {e}"))?;
        let entry_path = entry.path();
        if entry_path.is_file() {
            fs::remove_file(&entry_path)
                .map_err(|e| format!("清理状态文件失败 {}: {e}", entry_path.display()))?;
        } else if entry_path.is_dir() {
            fs::remove_dir_all(&entry_path)
                .map_err(|e| format!("清理状态目录失败 {}: {e}", entry_path.display()))?;
        }
    }
    Ok(())
}

fn kill_process_tree(child: &mut Child) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let pid = child.id().to_string();
        let mut command = Command::new("taskkill");
        command.args(["/PID", pid.as_str(), "/T", "/F"]);
        utils::configure_background_command(&mut command);
        command.status().map_err(|e| format!("停止任务失败: {e}"))?;
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        child.kill().map_err(|e| format!("停止任务失败: {e}"))?;
        Ok(())
    }
}

fn generate_nonce() -> (String, String) {
    let raw = Uuid::new_v4().simple().to_string();
    let nonce = format!("{}-{}-{}", &raw[0..4], &raw[4..8], &raw[8..12]);
    let mut parts = nonce.split('-').collect::<Vec<_>>();
    parts.reverse();
    let done_token = parts.join("-");
    (nonce, done_token)
}

fn normalize_optional(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn looks_like_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if [8, 13, 18, 23].contains(&index) {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn now_string() -> String {
    chrono::Local::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::{
        completion_detected, generate_nonce, maybe_record_session_id, now_string,
        run_native_autopilot_inner, sanitize_task_name, AutopilotPaths, AutopilotRuntime,
        AutopilotStartRequest, NativeAutopilotContext, DEFAULT_CONFIRM_TEXT, looks_like_uuid,
    };
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn test_request(task_file: &std::path::Path, workdir: &std::path::Path) -> AutopilotStartRequest {
        AutopilotStartRequest {
            task_file: task_file.to_string_lossy().to_string(),
            workdir: workdir.to_string_lossy().to_string(),
            model: String::new(),
            profile: String::new(),
            interval: Some(1),
            max_attempts: Some(1),
            codex_bin: Some("codex".to_string()),
            fresh: false,
            dry_run: true,
            skip_git_check: true,
            bypass: true,
            full_auto: true,
            verbose: false,
        }
    }

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
        let _ = fs::write(
            &path,
            format!("cccc-bbbb-aaaa\n{DEFAULT_CONFIRM_TEXT}\n\n"),
        );
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
    fn dry_run_writes_prompts_metadata_and_summary() {
        let temp = tempdir().unwrap();
        let workdir = temp.path().join("repo");
        let run_dir = temp.path().join("run");
        let task_file = workdir.join("task.md");
        fs::create_dir_all(&workdir).unwrap();
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(&task_file, "- [ ] verify dry run artifact generation\n").unwrap();

        let request = test_request(&task_file, &workdir);
        let paths = test_paths(&run_dir, task_file.canonicalize().unwrap());
        let runtime = AutopilotRuntime::default();
        let summary_path = run_dir.join("summary.json");
        runtime
            .replace_status(super::AutopilotStatus {
                running: true,
                stop_requested: false,
                status: "running".to_string(),
                phase: "preparing".to_string(),
                summary: "准备启动 Autopilot".to_string(),
                message: "准备启动 Autopilot".to_string(),
                started_at: Some(now_string()),
                finished_at: None,
                current_run_id: Some("test-run".to_string()),
                task_file: task_file.to_string_lossy().to_string(),
                task_name: "task".to_string(),
                workdir: workdir.to_string_lossy().to_string(),
                codex_bin: "codex".to_string(),
                logs_root_dir: env::temp_dir().to_string_lossy().to_string(),
                run_dir: run_dir.to_string_lossy().to_string(),
                log_dir: paths.log_dir.to_string_lossy().to_string(),
                state_dir: paths.state_dir.to_string_lossy().to_string(),
                main_log_path: paths.main_log.to_string_lossy().to_string(),
                attempt: 0,
                session_id: String::new(),
                last_message_preview: String::new(),
                last_error: String::new(),
                dry_run: true,
                recent_stages: Vec::new(),
            })
            .unwrap();

        let context = NativeAutopilotContext {
            request,
            paths: paths.clone(),
            codex_bin: "codex".to_string(),
            nonce: "aaaa-bbbb-cccc".to_string(),
            done_token: "cccc-bbbb-aaaa".to_string(),
        };

        run_native_autopilot_inner(&runtime, &context).unwrap();

        assert!(paths.initial_prompt_file.exists());
        assert!(paths.resume_prompt_file.exists());
        assert!(paths.meta_file.exists());
        assert!(paths.main_log.exists());
        assert!(summary_path.exists());

        let summary = runtime.snapshot().unwrap();
        assert!(!summary.running);
        assert_eq!(summary.status, "completed");
        assert_eq!(summary.phase, "completed");
    }
}
