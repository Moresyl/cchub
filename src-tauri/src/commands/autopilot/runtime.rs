// Codex 子进程托管：原生 autopilot 主循环 + 单轮执行 + 命令构造 + 提示词模板
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::utils;

use super::helpers::{
    append_main_log, completion_detected, normalize_optional, now_epoch_ms, now_string,
    resolve_workdir, write_text_file,
};
use super::status::{
    emit_stage, finalize_stopped, maybe_record_session_id, push_stage, sleep_with_stop,
    snapshot_last_message, stream_codex_pipe, update_last_message_preview, write_metadata,
};
use super::{
    AutopilotRuntime, ExecMode, NativeAutopilotContext, AUTOPILOT_STATUS_COMPLETED,
    AUTOPILOT_STATUS_FAILED, AUTOPILOT_STATUS_IDLE_STOPPED, AUTOPILOT_STATUS_MAX_ATTEMPTS,
    DEFAULT_CONFIRM_TEXT, DEFAULT_IDLE_THRESHOLD_SECS, DEFAULT_INTERVAL, DEFAULT_MAX_IDLE_STREAK,
};

pub(super) fn run_native_autopilot(
    app: AppHandle,
    runtime: AutopilotRuntime,
    context: NativeAutopilotContext,
) {
    if let Err(error) = run_native_autopilot_inner(&app, &runtime, &context) {
        let _ = append_main_log(&context.paths.main_log, "ERROR", &error);
        let _ = runtime.update_status(|status| {
            status.running = false;
            status.status = AUTOPILOT_STATUS_FAILED.to_string();
            status.finished_at = Some(now_string());
            status.last_error = error.clone();
            push_stage(status, "failed", error.clone(), Some(status.attempt));
        });
        emit_stage(&app, "failed", &error, None);
    }
    let _ = runtime.clear_child();
}

fn run_native_autopilot_inner(
    app: &AppHandle,
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
    emit_stage(app, "running", "开始执行任务", None);

    if context.request.dry_run {
        append_main_log(
            &context.paths.main_log,
            "INFO",
            "Dry Run 完成，未执行 Codex",
        )?;
        runtime.update_status(|status| {
            status.running = false;
            status.status = AUTOPILOT_STATUS_COMPLETED.to_string();
            status.finished_at = Some(now_string());
            push_stage(status, "completed", "Dry Run 已完成".to_string(), None);
        })?;
        emit_stage(app, "completed", "Dry Run 已完成", None);
        return Ok(());
    }

    let mut session_id = maybe_record_session_id(&context.paths)?;
    if let Some(existing) = session_id.clone() {
        runtime.update_status(|status| {
            status.session_id = existing;
            push_stage(
                status,
                "resume_ready",
                "检测到旧会话，可继续续跑".to_string(),
                None,
            );
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
                emit_stage(
                    app,
                    "max_attempts",
                    "达到最大尝试次数，任务已停止",
                    Some(attempt),
                );
                return Ok(());
            }
        }

        runtime.update_status(|status| {
            push_stage(
                status,
                "attempt",
                format!("开始第 {attempt} 轮执行"),
                Some(attempt),
            );
        })?;
        emit_stage(
            app,
            "attempt",
            &format!("开始第 {attempt} 轮执行"),
            Some(attempt),
        );

        let round_start = Instant::now();
        let exec_mode = if attempt == 1 || session_id.is_none() {
            ExecMode::Initial
        } else {
            ExecMode::Resume
        };
        let exec_result = run_codex_round(
            app,
            runtime,
            context,
            exec_mode,
            session_id.as_deref(),
            attempt,
        )?;
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
            emit_stage(
                app,
                "completed",
                "完成协议校验通过，任务已完成",
                Some(attempt),
            );
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
                append_main_log(
                    &context.paths.main_log,
                    "ERROR",
                    "连续空转达到阈值，自动停止",
                )?;
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
                emit_stage(
                    app,
                    "idle_stopped",
                    "检测到连续空转，已自动停止",
                    Some(attempt),
                );
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
        emit_stage(
            app,
            "waiting_retry",
            &format!("第 {attempt} 轮未完成，等待下次续跑"),
            Some(attempt),
        );

        if !sleep_with_stop(runtime, interval_secs) {
            finalize_stopped(runtime)?;
            return Ok(());
        }
    }
}

fn run_codex_round(
    app: &AppHandle,
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
    let phase_tag = match mode {
        ExecMode::Initial => "starting_session",
        ExecMode::Resume => "resuming_session",
    };
    runtime.update_status(|status| {
        push_stage(status, phase_tag, phase_message.clone(), Some(attempt));
    })?;
    emit_stage(app, phase_tag, &phase_message, Some(attempt));

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

    let mut child = command
        .spawn()
        .map_err(|e| format!("启动 Codex 失败: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    runtime.set_child(child)?;

    // round_start 事件 + heartbeat 状态
    let round_start_ms = now_epoch_ms();
    let last_event_ms: Arc<AtomicI64> = Arc::new(AtomicI64::new(round_start_ms));
    let _ = app.emit(
        "autopilot://round-start",
        serde_json::json!({
            "attempt": attempt,
            "mode": match mode {
                ExecMode::Initial => "initial",
                ExecMode::Resume => "resume",
            },
            "started_at_ms": round_start_ms,
        }),
    );

    let out_handle = stdout.map(|pipe| {
        let path = context.paths.event_log.clone();
        let app = app.clone();
        let last = last_event_ms.clone();
        thread::spawn(move || stream_codex_pipe(pipe, &path, app, "stdout", last, attempt))
    });
    let err_handle = stderr.map(|pipe| {
        let path = context.paths.runner_log.clone();
        let app = app.clone();
        let last = last_event_ms.clone();
        thread::spawn(move || stream_codex_pipe(pipe, &path, app, "stderr", last, attempt))
    });

    // 心跳：child 跑期间每 5s 推送 elapsed + last_event_age
    let heartbeat_stop = Arc::new(AtomicBool::new(false));
    let hb_handle = {
        let stop = heartbeat_stop.clone();
        let app = app.clone();
        let last_event_ms = last_event_ms.clone();
        thread::spawn(move || {
            // 启动后延迟首帧 1s，让 round-start 先到 UI
            thread::sleep(Duration::from_millis(1000));
            while !stop.load(Ordering::Relaxed) {
                let now = now_epoch_ms();
                let elapsed_ms = now - round_start_ms;
                let last_evt = last_event_ms.load(Ordering::Relaxed);
                let last_event_age_ms = if last_evt > 0 { now - last_evt } else { -1 };
                let _ = app.emit(
                    "autopilot://tick",
                    serde_json::json!({
                        "attempt": attempt,
                        "elapsed_ms": elapsed_ms,
                        "last_event_age_ms": last_event_age_ms,
                    }),
                );
                // 分片 sleep，使 stop 响应 ≤ 500ms
                for _ in 0..10 {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            }
        })
    };

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
    heartbeat_stop.store(true, Ordering::Relaxed);
    let _ = hb_handle.join();
    if let Some(handle) = out_handle {
        let _ = handle.join();
    }
    if let Some(handle) = err_handle {
        let _ = handle.join();
    }

    let round_end_ms = now_epoch_ms();
    let _ = app.emit(
        "autopilot://round-end",
        serde_json::json!({
            "attempt": attempt,
            "return_code": return_code,
            "elapsed_ms": round_end_ms - round_start_ms,
        }),
    );

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

    let mut command = build_codex_process(&context.codex_bin);
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

pub(super) fn build_codex_process(codex_bin: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let codex_path = Path::new(codex_bin);
        if let Some((node_bin, script_path)) = try_resolve_windows_node_wrapper(codex_path) {
            let mut command = Command::new(node_bin);
            command.arg(script_path);
            return command;
        }
    }

    Command::new(codex_bin)
}

#[cfg(target_os = "windows")]
fn try_resolve_windows_node_wrapper(codex_path: &Path) -> Option<(PathBuf, PathBuf)> {
    let parent = codex_path.parent()?;
    let script_path = parent
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("bin")
        .join("codex.js");
    if !script_path.is_file() {
        return None;
    }

    let node_bin = parent.join("node.exe");
    if node_bin.is_file() {
        return Some((node_bin, script_path));
    }

    Some((PathBuf::from("node"), script_path))
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
