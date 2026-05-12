import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { FolderOpen, Play, RefreshCw, Square } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ConfirmDialog from "../components/ConfirmDialog";
import { showToast } from "../components/Toast";
import { getLocale } from "../lib/i18n";
import { useAutopilotFormStore, type AutopilotFormState } from "../stores/autopilotForm";
import { useClearAutopilotLogsMutation, useDeleteAutopilotLogMutation } from "../hooks/mutations";

import {
  EMPTY_STATUS,
  MAX_LIVE_EVENTS,
  POLL_INTERVAL_MS,
  applyPermissionMode,
  formatRuntime,
  getPhaseLabel,
  getStatusLabel,
  getStatusTone,
  permissionModeFromForm,
  type AutopilotStatus,
  type CodexEventPayload,
  type DialogState,
  type LiveEvent,
  type PermissionMode,
  type RoundEndPayload,
  type RoundStartPayload,
  type StagePayload,
  type TickPayload,
} from "./autopilot/helpers";
import {
  HeartbeatBadge,
  LiveOutputPanel,
  MetricsRow,
  RunHistoryPanel,
  RunSetupPanel,
  RunSummaryPanel,
  StageTimelinePanel,
  TaskQueuePanel,
} from "./autopilot/panels";

export default function Autopilot() {
  const form = useAutopilotFormStore((state) => state.form);
  const setForm = useAutopilotFormStore((state) => state.setForm);
  const resetForm = useAutopilotFormStore((state) => state.resetForm);
  const [status, setStatus] = useState<AutopilotStatus>(EMPTY_STATUS);
  const [logs, setLogs] = useState<AutopilotStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [dialog, setDialog] = useState<DialogState>(null);
  const [busyRunId, setBusyRunId] = useState<string | null>(null);
  const [nowTs, setNowTs] = useState(() => Date.now());
  // L1 实时事件流：codex stdout/stderr + stage 事件累积
  const [liveEvents, setLiveEvents] = useState<LiveEvent[]>([]);
  // L2 心跳：本轮 elapsed 与最近事件距今毫秒数
  const [tick, setTick] = useState<TickPayload | null>(null);
  // 实时输出面板可折叠
  const [liveOutputOpen, setLiveOutputOpen] = useState(true);
  const liveEventIdRef = useRef(0);
  const liveListRef = useRef<HTMLDivElement | null>(null);
  const locale = getLocale();
  const deleteAutopilotLogMutation = useDeleteAutopilotLogMutation();
  const clearAutopilotLogsMutation = useClearAutopilotLogsMutation();
  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );

  const loadStatus = useCallback(async () => {
    const nextStatus = await invoke<AutopilotStatus>("get_autopilot_status");
    setStatus(nextStatus);
    return nextStatus;
  }, []);

  const loadLogs = useCallback(async () => {
    const nextLogs = await invoke<AutopilotStatus[]>("list_autopilot_logs");
    setLogs(nextLogs);
    return nextLogs;
  }, []);

  const loadAll = useCallback(
    async (silent = false) => {
      if (silent) {
        setRefreshing(true);
      } else {
        setLoading(true);
      }
      try {
        await Promise.all([loadStatus(), loadLogs()]);
      } catch (error) {
        console.error(error);
        if (!silent) {
          showToast(
            "error",
            uiText(
              `加载 Autopilot 状态失败: ${error}`,
              `Failed to load Autopilot status: ${error}`,
              `Autopilot 状態の読み込みに失敗しました: ${error}`,
            ),
          );
        }
      } finally {
        if (silent) {
          setRefreshing(false);
        } else {
          setLoading(false);
        }
      }
    },
    [loadLogs, loadStatus, uiText],
  );

  useEffect(() => {
    void loadAll();
  }, [loadAll]);

  // L1+L2 实时事件订阅：codex 输出流、心跳、轮次与阶段切换
  // Tauri 事件通道一旦建立就是进程内直通的，不依赖 status.running，
  // 因为 running=false → true 的翻转本身就是通过事件先到达前端的
  useEffect(() => {
    let unlistenAll: UnlistenFn[] = [];
    let cancelled = false;

    const appendEvent = (event: Omit<LiveEvent, "id">) => {
      liveEventIdRef.current += 1;
      const id = liveEventIdRef.current;
      setLiveEvents((prev) => {
        const next = [...prev, { ...event, id } as LiveEvent];
        if (next.length > MAX_LIVE_EVENTS) {
          next.splice(0, next.length - MAX_LIVE_EVENTS);
        }
        return next;
      });
    };

    (async () => {
      const subs = await Promise.all([
        listen<CodexEventPayload>("autopilot://codex-event", (e) => {
          appendEvent({ kind: "codex", payload: e.payload });
        }),
        listen<TickPayload>("autopilot://tick", (e) => {
          setTick(e.payload);
        }),
        listen<RoundStartPayload>("autopilot://round-start", (e) => {
          appendEvent({ kind: "round-start", payload: e.payload });
          setTick({ attempt: e.payload.attempt, elapsed_ms: 0, last_event_age_ms: -1 });
        }),
        listen<RoundEndPayload>("autopilot://round-end", (e) => {
          appendEvent({ kind: "round-end", payload: e.payload });
          // 轮次结束立即刷状态，不等 10s 轮询
          void loadAll(true);
        }),
        listen<StagePayload>("autopilot://stage", (e) => {
          appendEvent({ kind: "stage", payload: e.payload });
          // 阶段切换往往伴随 status/finished 变化，同步一次
          void loadAll(true);
        }),
      ]);
      if (cancelled) {
        subs.forEach((fn) => fn());
      } else {
        unlistenAll = subs;
      }
    })().catch((error) => console.error("autopilot listen failed:", error));

    return () => {
      cancelled = true;
      unlistenAll.forEach((fn) => fn());
    };
  }, [loadAll]);

  // 新一轮启动时自动滚到底部
  useEffect(() => {
    if (!liveOutputOpen) return;
    const el = liveListRef.current;
    if (!el) return;
    // 小幅超时让 DOM 更新先完成
    requestAnimationFrame(() => {
      el.scrollTop = el.scrollHeight;
    });
  }, [liveEvents, liveOutputOpen]);

  useEffect(() => {
    if (!status.running) return undefined;
    const timer = window.setInterval(() => {
      void loadAll(true);
    }, POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [loadAll, status.running]);

  // running 结束后清理心跳 state，避免旧值残留
  useEffect(() => {
    if (!status.running) {
      setTick(null);
    }
  }, [status.running]);

  useEffect(() => {
    if (!status.running) return undefined;
    const timer = window.setInterval(() => {
      setNowTs(Date.now());
    }, 1000);
    return () => window.clearInterval(timer);
  }, [status.running]);

  const updateField = useCallback(
    <K extends keyof AutopilotFormState>(key: K, value: AutopilotFormState[K]) => {
      setForm((current) => ({ ...current, [key]: value }));
    },
    [setForm],
  );

  const updatePermissionMode = useCallback(
    (mode: PermissionMode) => {
      setForm((current) => ({ ...current, ...applyPermissionMode(mode) }));
    },
    [setForm],
  );

  const handlePickFiles = useCallback(async () => {
    try {
      const picked = await invoke<string[]>("pick_autopilot_files");
      if (picked && picked.length > 0) {
        setForm((current) => {
          const existing = new Set(current.taskFiles);
          const merged = [...current.taskFiles];
          for (const p of picked) {
            if (!existing.has(p)) merged.push(p);
          }
          return { ...current, taskFiles: merged };
        });
      }
    } catch (error) {
      showToast("error", `${error}`);
    }
  }, [setForm]);

  const handleRemoveTaskFile = useCallback(
    (idx: number) => {
      setForm((current) => ({
        ...current,
        taskFiles: current.taskFiles.filter((_, i) => i !== idx),
      }));
    },
    [setForm],
  );

  const handleMoveTaskFile = useCallback(
    (idx: number, direction: -1 | 1) => {
      setForm((current) => {
        const next = [...current.taskFiles];
        const target = idx + direction;
        if (target < 0 || target >= next.length) return current;
        [next[idx], next[target]] = [next[target], next[idx]];
        return { ...current, taskFiles: next };
      });
    },
    [setForm],
  );

  const handleClearTaskFiles = useCallback(() => {
    setForm((current) => ({ ...current, taskFiles: [] }));
  }, [setForm]);

  const handlePickFolder = useCallback(async () => {
    try {
      const picked = await invoke<string | null>("pick_folder");
      if (picked) {
        updateField("workdir", picked);
      }
    } catch (error) {
      showToast("error", `${error}`);
    }
  }, [updateField]);

  const executeStart = useCallback(async () => {
    setStarting(true);
    try {
      const nextStatus = await invoke<AutopilotStatus>("start_autopilot", {
        request: {
          taskFile: form.taskFiles[0] ?? "",
          taskFiles: form.taskFiles,
          workdir: form.workdir,
          model: form.model,
          profile: form.profile,
          interval: Number(form.interval || "0"),
          maxAttempts: Number(form.maxAttempts || "0"),
          fresh: form.fresh,
          dryRun: form.dryRun,
          skipGitCheck: form.skipGitCheck,
          bypass: form.bypass,
          fullAuto: form.fullAuto,
          verbose: form.verbose,
        },
      });
      setStatus(nextStatus);
      await loadLogs();
      showToast("success", uiText("Autopilot 已启动", "Autopilot started", "Autopilot を起動しました"));
    } catch (error) {
      console.error(error);
      showToast("error", `${error}`);
      await loadAll(true);
    } finally {
      setStarting(false);
    }
  }, [form, loadAll, loadLogs, uiText]);

  const handleStart = useCallback(() => {
    if (permissionModeFromForm(form) === "bypass") {
      setDialog({ type: "start-bypass" });
      return;
    }
    void executeStart();
  }, [executeStart, form]);

  const handleStop = useCallback(async () => {
    setStopping(true);
    try {
      const nextStatus = await invoke<AutopilotStatus>("stop_autopilot");
      setStatus(nextStatus);
      showToast("success", uiText("停止请求已发送", "Stop request sent", "停止要求を送信しました"));
    } catch (error) {
      console.error(error);
      showToast("error", `${error}`);
    } finally {
      setStopping(false);
    }
  }, [uiText]);

  const handleRefresh = useCallback(() => {
    void loadAll(true);
  }, [loadAll]);

  const handleOpenLogsRoot = useCallback(async () => {
    const target = status.logsRootDir || logs[0]?.logsRootDir;
    if (!target) {
      showToast(
        "error",
        uiText("日志目录尚未创建", "Log directory is not ready yet", "ログディレクトリはまだ作成されていません"),
      );
      return;
    }
    try {
      await invoke("open_in_system", { target });
    } catch (error) {
      showToast("error", `${error}`);
    }
  }, [logs, status.logsRootDir, uiText]);

  const handleOpenRunDir = useCallback(async (target: string) => {
    try {
      await invoke("open_in_system", { target });
    } catch (error) {
      showToast("error", `${error}`);
    }
  }, []);

  const handleResetForm = useCallback(() => {
    resetForm();
  }, [resetForm]);

  const performDelete = useCallback(
    async (runId: string) => {
      setBusyRunId(runId);
      try {
        await deleteAutopilotLogMutation.mutateAsync({ runId });
        setLogs((current) => current.filter((entry) => entry.currentRunId !== runId));
        showToast("success", uiText("日志记录已删除", "Log record deleted", "ログ記録を削除しました"));
      } catch (error) {
        showToast("error", `${error}`);
      } finally {
        setBusyRunId(null);
        setDialog(null);
      }
    },
    [deleteAutopilotLogMutation, uiText],
  );

  const performClear = useCallback(async () => {
    setBusyRunId("clear");
    try {
      const result = await clearAutopilotLogsMutation.mutateAsync();
      showToast(
        "success",
        uiText(
          `已清理 ${result.deletedCount} 条日志记录`,
          `Removed ${result.deletedCount} log record(s)`,
          `${result.deletedCount} 件のログ記録を削除しました`,
        ),
      );
      setLogs((current) => current.filter((entry) => status.running && entry.currentRunId === status.currentRunId));
    } catch (error) {
      showToast("error", `${error}`);
    } finally {
      setBusyRunId(null);
      setDialog(null);
    }
  }, [clearAutopilotLogsMutation, status.currentRunId, status.running, uiText]);

  const canStart = useMemo(
    () => !status.running && !starting && form.taskFiles.length > 0,
    [form.taskFiles, starting, status.running],
  );
  const stageItems = status.recentStages.slice().reverse();
  const currentSummary =
    status.summary || status.message || uiText("尚未开始执行", "No run in progress", "まだ実行されていません");
  const latestRun = logs[0] ?? null;
  const statusLabel = getStatusLabel(status.status, uiText);
  const phaseLabel = getPhaseLabel(status.phase, uiText);
  const statusTone = getStatusTone(status.status);
  const runtimeLabel = formatRuntime(status.startedAt, status.finishedAt, nowTs);
  const runtimeTone = status.running ? "#2563eb" : "var(--text-primary)";
  const permissionMode = permissionModeFromForm(form);

  // 心跳展示：本轮已执行 Xm Ys · 最近事件 Zs 前
  const heartbeatLabel = useMemo(() => {
    if (!status.running || !tick) return null;
    const elapsedSec = Math.max(0, Math.floor(tick.elapsed_ms / 1000));
    const m = Math.floor(elapsedSec / 60);
    const s = elapsedSec % 60;
    const elapsedText = m > 0 ? `${m}m ${String(s).padStart(2, "0")}s` : `${s}s`;
    if (tick.last_event_age_ms < 0) {
      return uiText(`本轮 ${elapsedText}`, `Round ${elapsedText}`, `ラウンド ${elapsedText}`);
    }
    const ageSec = Math.max(0, Math.floor(tick.last_event_age_ms / 1000));
    const stall = ageSec >= 120;
    const ageText = ageSec >= 60 ? `${Math.floor(ageSec / 60)}m${ageSec % 60}s` : `${ageSec}s`;
    return uiText(
      `本轮 ${elapsedText} · 最近事件 ${ageText} 前${stall ? " ⚠" : ""}`,
      `Round ${elapsedText} · last event ${ageText} ago${stall ? " ⚠" : ""}`,
      `ラウンド ${elapsedText} · 最新イベント ${ageText} 前${stall ? " ⚠" : ""}`,
    );
  }, [status.running, tick, uiText]);

  const dialogTitle =
    dialog?.type === "clear"
      ? uiText("清理全部日志", "Clear All Logs", "すべてのログを削除")
      : dialog?.type === "start-bypass"
        ? uiText("确认使用 Bypass", "Confirm Bypass", "Bypass を確認")
        : uiText("删除日志记录", "Delete Log Record", "ログ記録を削除");
  const dialogMessage =
    dialog?.type === "clear"
      ? uiText(
          "将删除所有已保存的 Autopilot 运行记录。正在运行的任务会被保留。",
          "All saved Autopilot records will be removed. Any active run will be preserved.",
          "保存済みの Autopilot 記録を削除します。実行中のタスクは保持されます。",
        )
      : dialog?.type === "start-bypass"
        ? uiText(
            "Bypass 会使用 --dangerously-bypass-approvals-and-sandbox 启动 Codex，跳过审批与沙箱限制。确认继续？",
            "Bypass starts Codex with --dangerously-bypass-approvals-and-sandbox and skips approval plus sandbox checks. Continue?",
            "Bypass は --dangerously-bypass-approvals-and-sandbox で Codex を起動し、承認とサンドボックス制限をスキップします。続行しますか？",
          )
        : uiText(
            `确定删除「${dialog?.name || ""}」这条运行记录吗？`,
            `Delete the run record "${dialog?.name || ""}"?`,
            `「${dialog?.name || ""}」の実行記録を削除しますか？`,
          );
  const dialogConfirmText =
    dialog?.type === "clear"
      ? uiText("全部清理", "Clear All", "すべて削除")
      : dialog?.type === "start-bypass"
        ? uiText("确认启动", "Start Anyway", "確認して開始")
        : uiText("删除", "Delete", "削除");

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
          {uiText("正在加载 Autopilot...", "Loading Autopilot...", "Autopilot を読み込み中...")}
        </span>
      </div>
    );
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column", minHeight: 0 }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{uiText("Autopilot", "Autopilot", "Autopilot")}</h2>
          <p className="page-subtitle">
            {uiText(
              "由 Rust 原生托管 Codex 续跑流程，统一管理阶段、运行状态与独立日志目录",
              "Native Rust orchestration for Codex resume loops with centralized stages and logs",
              "Rust ネイティブで Codex 継続実行を管理し、段階とログを一元管理します",
            )}
          </p>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", justifyContent: "flex-end" }}>
          <button className="btn btn-secondary btn-sm" onClick={handleOpenLogsRoot} style={{ gap: 6 }}>
            <FolderOpen size={14} />
            {uiText("打开日志目录", "Open Logs Folder", "ログフォルダを開く")}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={handleRefresh} style={{ gap: 6 }} disabled={refreshing}>
            <RefreshCw size={14} className={refreshing ? "spin" : undefined} />
            {uiText("刷新", "Refresh", "更新")}
          </button>
          {status.running ? (
            <button
              className="btn btn-danger btn-sm"
              onClick={() => void handleStop()}
              style={{ gap: 6 }}
              disabled={stopping}
            >
              <Square size={14} />
              {stopping ? uiText("停止中...", "Stopping...", "停止中...") : uiText("停止任务", "Stop Run", "停止")}
            </button>
          ) : (
            <button
              className="btn btn-primary btn-sm"
              onClick={() => void handleStart()}
              style={{ gap: 6 }}
              disabled={!canStart}
            >
              <Play size={14} />
              {starting ? uiText("启动中...", "Starting...", "開始中...") : uiText("启动任务", "Start Run", "開始")}
            </button>
          )}
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 16 }}>
        <RunSetupPanel
          uiText={uiText}
          form={form}
          permissionMode={permissionMode}
          updateField={updateField}
          updatePermissionMode={updatePermissionMode}
          handlePickFiles={() => void handlePickFiles()}
          handleRemoveTaskFile={handleRemoveTaskFile}
          handleMoveTaskFile={handleMoveTaskFile}
          handleClearTaskFiles={handleClearTaskFiles}
          handlePickFolder={() => void handlePickFolder()}
          handleResetForm={handleResetForm}
        />

        <MetricsRow
          uiText={uiText}
          status={status}
          statusLabel={statusLabel}
          statusTone={statusTone}
          phaseLabel={phaseLabel}
          runtimeLabel={runtimeLabel}
          runtimeTone={runtimeTone}
          latestRun={latestRun}
        />

        <TaskQueuePanel uiText={uiText} status={status} />

        <HeartbeatBadge label={heartbeatLabel} />

        <RunSummaryPanel uiText={uiText} status={status} currentSummary={currentSummary} />

        <LiveOutputPanel
          uiText={uiText}
          liveEvents={liveEvents}
          liveOutputOpen={liveOutputOpen}
          setLiveOutputOpen={setLiveOutputOpen}
          setLiveEvents={setLiveEvents}
          liveListRef={liveListRef}
        />

        <StageTimelinePanel uiText={uiText} stageItems={stageItems} />

        <RunHistoryPanel
          uiText={uiText}
          logs={logs}
          busyRunId={busyRunId}
          setDialog={setDialog}
          handleOpenRunDir={(t) => void handleOpenRunDir(t)}
        />
      </div>

      <ConfirmDialog
        isOpen={dialog !== null}
        title={dialogTitle}
        message={dialogMessage}
        confirmText={dialogConfirmText}
        cancelText={uiText("取消", "Cancel", "キャンセル")}
        onCancel={() => setDialog(null)}
        onConfirm={() => {
          if (dialog?.type === "start-bypass") {
            setDialog(null);
            void executeStart();
            return;
          }
          if (dialog?.type === "clear") {
            void performClear();
            return;
          }
          if (dialog?.type === "delete" && dialog.runId) {
            void performDelete(dialog.runId);
          }
        }}
      />
    </div>
  );
}
