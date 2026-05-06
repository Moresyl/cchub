import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  Bot,
  FileSearch,
  FolderOpen,
  Play,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  SlidersHorizontal,
  Square,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import ConfirmDialog from "../components/ConfirmDialog";
import { showToast } from "../components/Toast";
import { getLocale } from "../lib/i18n";
import { useAutopilotFormStore, type AutopilotFormState } from "../stores/autopilotForm";
import { useClearAutopilotLogsMutation, useDeleteAutopilotLogMutation } from "../hooks/mutations";

interface AutopilotStageEntry {
  at: string;
  phase: string;
  message: string;
  attempt: number | null;
}

interface AutopilotStatus {
  running: boolean;
  stopRequested: boolean;
  status: string;
  phase: string;
  summary: string;
  message: string;
  startedAt: string | null;
  finishedAt: string | null;
  currentRunId: string | null;
  taskFile: string;
  taskName: string;
  workdir: string;
  codexBin: string;
  logsRootDir: string;
  runDir: string;
  logDir: string;
  stateDir: string;
  mainLogPath: string;
  attempt: number;
  sessionId: string;
  lastMessagePreview: string;
  lastError: string;
  dryRun: boolean;
  recentStages: AutopilotStageEntry[];
  taskQueue: string[];
  currentTaskIndex: number;
}

type PermissionMode = "approval" | "fullAuto" | "bypass";

// 后端实时事件：codex 子进程 stdout/stderr + 心跳 + 轮次/阶段切换
interface CodexEventPayload {
  kind: "stdout" | "stderr";
  attempt: number;
  json?: unknown;
  raw?: string;
}
interface TickPayload {
  attempt: number;
  elapsed_ms: number;
  last_event_age_ms: number;
}
interface RoundStartPayload {
  attempt: number;
  mode: "initial" | "resume";
  started_at_ms: number;
}
interface RoundEndPayload {
  attempt: number;
  return_code: number;
  elapsed_ms: number;
}
interface StagePayload {
  phase: string;
  message: string;
  attempt: number | null;
  at_ms: number;
}
type LiveEvent =
  | { id: number; kind: "codex"; payload: CodexEventPayload }
  | { id: number; kind: "round-start"; payload: RoundStartPayload }
  | { id: number; kind: "round-end"; payload: RoundEndPayload }
  | { id: number; kind: "stage"; payload: StagePayload };

type DialogState =
  | { type: "delete"; runId: string; name: string }
  | { type: "clear" }
  | { type: "start-bypass" }
  | null;

// 事件流接入后，轮询只做兜底：事件丢失/崩溃恢复/最终状态持久化
const POLL_INTERVAL_MS = 10000;
// 实时输出面板最多保留的条数，再多就丢弃最旧的（避免内存膨胀）
const MAX_LIVE_EVENTS = 300;
const EMPTY_STATUS: AutopilotStatus = {
  running: false,
  stopRequested: false,
  status: "idle",
  phase: "idle",
  summary: "",
  message: "",
  startedAt: null,
  finishedAt: null,
  currentRunId: null,
  taskFile: "",
  taskName: "",
  workdir: "",
  codexBin: "",
  logsRootDir: "",
  runDir: "",
  logDir: "",
  stateDir: "",
  mainLogPath: "",
  attempt: 0,
  sessionId: "",
  lastMessagePreview: "",
  lastError: "",
  dryRun: false,
  recentStages: [],
  taskQueue: [],
  currentTaskIndex: 0,
};

function permissionModeFromForm(form: Pick<AutopilotFormState, "bypass" | "fullAuto">): PermissionMode {
  if (form.bypass) return "bypass";
  if (form.fullAuto) return "fullAuto";
  return "approval";
}

function applyPermissionMode(mode: PermissionMode): Pick<AutopilotFormState, "bypass" | "fullAuto"> {
  switch (mode) {
    case "bypass":
      return { bypass: true, fullAuto: false };
    case "fullAuto":
      return { bypass: false, fullAuto: true };
    default:
      return { bypass: false, fullAuto: false };
  }
}

function formatDateTime(value: string | null) {
  if (!value) return "--";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function formatRuntime(startedAt: string | null, finishedAt: string | null, nowTs: number) {
  if (!startedAt) return "--";
  const start = new Date(startedAt).getTime();
  if (Number.isNaN(start)) return "--";

  const end = finishedAt ? new Date(finishedAt).getTime() : nowTs;
  if (Number.isNaN(end)) return "--";

  const totalSeconds = Math.max(0, Math.floor((end - start) / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${String(minutes).padStart(2, "0")}m ${String(seconds).padStart(2, "0")}s`;
  }

  return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
}

function shortenPath(value: string, keep = 42) {
  const trimmed = value.trim();
  if (!trimmed) return "--";
  if (trimmed.length <= keep) return trimmed;
  return `${trimmed.slice(0, 18)}...${trimmed.slice(-18)}`;
}

function leafName(value: string) {
  if (!value.trim()) return "--";
  const parts = value.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || value;
}

function getStatusLabel(status: string, uiText: (zhText: string, enText: string, jaText?: string) => string) {
  switch (status) {
    case "running":
      return uiText("运行中", "Running", "実行中");
    case "stopping":
      return uiText("停止中", "Stopping", "停止中");
    case "stopped":
      return uiText("已停止", "Stopped", "停止済み");
    case "completed":
      return uiText("已完成", "Completed", "完了");
    case "failed":
      return uiText("失败", "Failed", "失敗");
    case "max_attempts":
      return uiText("达到上限", "Max Attempts", "上限到達");
    case "idle_stopped":
      return uiText("空转停止", "Idle Stopped", "空転停止");
    default:
      return uiText("空闲", "Idle", "待機中");
  }
}

function getPhaseLabel(phase: string, uiText: (zhText: string, enText: string, jaText?: string) => string) {
  switch (phase) {
    case "preparing":
      return uiText("准备启动", "Preparing", "準備中");
    case "starting":
      return uiText("已启动", "Started", "起動済み");
    case "fresh_start":
      return uiText("清理旧状态", "Fresh Start", "旧状態を削除");
    case "running":
      return uiText("执行中", "Running", "実行中");
    case "starting_session":
      return uiText("新建会话", "New Session", "新規セッション");
    case "resuming_session":
      return uiText("续跑会话", "Resuming", "再開中");
    case "resume_ready":
      return uiText("检测到旧会话", "Resume Ready", "再開可能");
    case "session_bound":
      return uiText("已绑定会话", "Session Bound", "セッション紐付け");
    case "attempt":
      return uiText("执行轮次", "Attempt", "試行");
    case "assistant_reply":
      return uiText("收到回复", "Reply Received", "応答受信");
    case "waiting_retry":
      return uiText("等待续跑", "Waiting Retry", "再試行待ち");
    case "idle_warning":
      return uiText("疑似空转", "Idle Warning", "空転警告");
    case "idle_stopped":
      return uiText("空转停止", "Idle Stopped", "空転停止");
    case "stopping":
      return uiText("正在停止", "Stopping", "停止中");
    case "stop_warning":
      return uiText("停止异常", "Stop Warning", "停止警告");
    case "stopped":
      return uiText("已停止", "Stopped", "停止済み");
    case "completed":
      return uiText("已完成", "Completed", "完了");
    case "max_attempts":
      return uiText("达到上限", "Max Attempts", "上限到達");
    case "failed":
      return uiText("失败", "Failed", "失敗");
    default:
      return uiText("空闲", "Idle", "待機中");
  }
}

function getStatusTone(status: string) {
  switch (status) {
    case "running":
      return "#2563eb";
    case "completed":
      return "#059669";
    case "failed":
      return "#dc2626";
    case "stopping":
    case "stopped":
    case "max_attempts":
    case "idle_stopped":
      return "#d97706";
    default:
      return "var(--text-secondary)";
  }
}

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
  const logsCount = logs.length;
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
        <div className="section-card">
          <div className="section-card-title">
            <Bot size={16} />
            {uiText("任务入口", "Run Setup", "実行設定")}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
            {uiText(
              "界面只显示阶段和摘要，不直接展示原始 Codex 日志。完整日志会统一保存到专用目录，可单独打开和清理。后端直接调用 Codex。",
              "The UI shows stages and summaries only. Full logs are stored in a dedicated folder. The backend now invokes Codex directly instead of relying on an external Python script.",
              "画面には段階と要約のみを表示し、生ログは専用フォルダに保存します。バックエンドは Python スクリプトではなく Codex を直接呼び出します。",
            )}
          </p>

          <TaskFileList
            files={form.taskFiles}
            label={uiText("任务文件", "Task Files", "タスクファイル")}
            description={uiText(
              "支持选择多个任务文件，按列表顺序依次执行（队列模式）。",
              "Select multiple task files; they execute sequentially in order (queue mode).",
              "複数のタスクファイルを選択でき、リスト順に順次実行されます（キューモード）。",
            )}
            addLabel={uiText("添加任务", "Add Tasks", "タスクを追加")}
            clearLabel={uiText("清空", "Clear", "クリア")}
            emptyLabel={uiText("尚未选择任务文件", "No task file selected", "タスクファイル未選択")}
            onPick={() => void handlePickFiles()}
            onRemove={handleRemoveTaskFile}
            onMove={handleMoveTaskFile}
            onClear={handleClearTaskFiles}
          />

          <div style={{ marginTop: 14 }}>
            <PathField
              label={uiText("工作目录", "Working Directory", "作業ディレクトリ")}
              value={form.workdir}
              placeholder={uiText(
                "留空时自动使用任务文件所在目录",
                "Leave empty to use the task file directory",
                "空欄ならタスクファイルの親ディレクトリを使います",
              )}
              onChange={(value) => updateField("workdir", value)}
              onPick={() => void handlePickFolder()}
              buttonLabel={uiText("选择目录", "Pick Folder", "フォルダを選択")}
            />
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
              gap: 12,
              marginTop: 14,
            }}
          >
            <TextField
              label={uiText("模型", "Model", "モデル")}
              value={form.model}
              onChange={(value) => updateField("model", value)}
              placeholder="gpt-5.4"
            />
            <TextField
              label={uiText("Profile", "Profile", "プロファイル")}
              value={form.profile}
              onChange={(value) => updateField("profile", value)}
              placeholder="default"
            />
            <TextField
              label={uiText("轮询间隔(秒)", "Retry Interval (s)", "間隔(秒)")}
              value={form.interval}
              onChange={(value) => updateField("interval", value)}
              placeholder="3"
            />
            <TextField
              label={uiText("最大轮次", "Max Attempts", "最大試行回数")}
              value={form.maxAttempts}
              onChange={(value) => updateField("maxAttempts", value)}
              placeholder="0"
            />
          </div>

          <div style={{ marginTop: 16 }}>
            <label className="field-label">{uiText("权限模式", "Permission Mode", "権限モード")}</label>
            <select
              className="input"
              value={permissionMode}
              onChange={(event) => updatePermissionMode(event.target.value as PermissionMode)}
            >
              <option value="approval">{uiText("审批", "Approval", "承認")}</option>
              <option value="fullAuto">Full Auto</option>
              <option value="bypass">Bypass</option>
            </select>
            {permissionMode === "bypass" && (
              <div
                style={{
                  marginTop: 8,
                  padding: "9px 11px",
                  borderRadius: 8,
                  border: "1px solid rgba(220, 38, 38, 0.35)",
                  background: "rgba(220, 38, 38, 0.08)",
                  color: "#dc2626",
                  fontSize: 12,
                  fontWeight: 600,
                }}
              >
                {uiText(
                  "Bypass 会跳过审批与沙箱限制，启动前需要再次确认。",
                  "Bypass skips approval and sandbox checks. Starting requires confirmation.",
                  "Bypass は承認とサンドボックス制限をスキップします。開始前に確認が必要です。",
                )}
              </div>
            )}
          </div>

          <details style={{ marginTop: 16 }}>
            <summary
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                cursor: "pointer",
                color: "var(--text-secondary)",
                fontSize: 13,
                fontWeight: 600,
              }}
            >
              <SlidersHorizontal size={14} />
              {uiText("高级选项", "Advanced Options", "詳細オプション")}
            </summary>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
                gap: 10,
                marginTop: 12,
              }}
            >
              <ToggleOption
                label={uiText("从头开始", "Fresh Run", "新規実行")}
                checked={form.fresh}
                onChange={(checked) => updateField("fresh", checked)}
              />
              <ToggleOption
                label={uiText("Dry Run", "Dry Run", "Dry Run")}
                checked={form.dryRun}
                onChange={(checked) => updateField("dryRun", checked)}
              />
              <ToggleOption
                label={uiText("跳过 Git 检查", "Skip Git Check", "Git チェックをスキップ")}
                checked={form.skipGitCheck}
                onChange={(checked) => updateField("skipGitCheck", checked)}
              />
              <ToggleOption
                label={uiText("详细调试日志", "Verbose Debug Logs", "詳細デバッグログ")}
                checked={form.verbose}
                onChange={(checked) => updateField("verbose", checked)}
              />
            </div>
          </details>

          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 12,
              alignItems: "center",
              marginTop: 18,
              flexWrap: "wrap",
            }}
          >
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {uiText(
                "所有运行日志将统一保存到 `~/.cchub/autopilot/runs`。",
                "All runs are stored under `~/.cchub/autopilot/runs`.",
                "すべての実行ログは `~/.cchub/autopilot/runs` に保存されます。",
              )}
            </div>
            <button className="btn btn-secondary btn-sm" onClick={handleResetForm} style={{ gap: 6 }}>
              <RotateCcw size={14} />
              {uiText("重置表单", "Reset Form", "フォームをリセット")}
            </button>
          </div>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: 12 }}>
          <MetricCard
            title={uiText("当前状态", "Current Status", "現在の状態")}
            value={statusLabel}
            tone={statusTone}
            icon={<ShieldCheck size={16} />}
          />
          <MetricCard
            title={uiText("当前阶段", "Current Phase", "現在の段階")}
            value={phaseLabel}
            tone="var(--accent)"
            icon={<Bot size={16} />}
          />
          <MetricCard
            title={uiText("运行时长", "Runtime", "実行時間")}
            value={runtimeLabel}
            tone={runtimeTone}
            icon={<Play size={16} />}
          />
          <MetricCard
            title={uiText("执行轮次", "Attempt", "実行回数")}
            value={status.attempt > 0 ? String(status.attempt) : "--"}
            tone="var(--text-primary)"
            icon={<RefreshCw size={16} />}
          />
          {status.taskQueue && status.taskQueue.length > 1 ? (
            <MetricCard
              title={uiText("队列进度", "Queue Progress", "キュー進行")}
              value={`${status.currentTaskIndex + 1} / ${status.taskQueue.length}`}
              tone="var(--accent)"
              icon={<FileSearch size={16} />}
            />
          ) : (
            <MetricCard
              title={uiText("最近一次运行", "Latest Run", "最新の実行")}
              value={latestRun ? leafName(latestRun.taskFile || latestRun.taskName) : "--"}
              tone="var(--text-primary)"
              icon={<FileSearch size={16} />}
            />
          )}
        </div>

        {status.taskQueue && status.taskQueue.length > 1 && (
          <div className="section-card">
            <div className="section-card-title">
              <Bot size={16} />
              {uiText("任务队列", "Task Queue", "タスクキュー")}
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {status.taskQueue.map((file, idx) => {
                const parts = file.split(/[\\/]/).filter(Boolean);
                const name = parts[parts.length - 1] || file;
                const isCurrent = idx === status.currentTaskIndex;
                const isDone = idx < status.currentTaskIndex;
                return (
                  <div
                    key={`${file}-${idx}`}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 10,
                      padding: "6px 10px",
                      borderRadius: 6,
                      background: isCurrent ? "var(--bg-elevated)" : "transparent",
                      border: isCurrent ? "1px solid var(--accent)" : "1px solid transparent",
                      opacity: isDone ? 0.55 : 1,
                    }}
                  >
                    <span
                      style={{
                        width: 22,
                        height: 22,
                        borderRadius: "50%",
                        background: isCurrent ? "var(--accent)" : isDone ? "var(--success, #22c55e)" : "var(--bg-app)",
                        color: isCurrent || isDone ? "#fff" : "var(--text-secondary)",
                        display: "inline-flex",
                        alignItems: "center",
                        justifyContent: "center",
                        fontSize: 11,
                        fontWeight: 600,
                        flexShrink: 0,
                      }}
                    >
                      {isDone ? "✓" : idx + 1}
                    </span>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div
                        style={{
                          fontSize: 13,
                          fontWeight: isCurrent ? 600 : 400,
                          color: "var(--text-primary)",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {name}
                      </div>
                      <div
                        style={{
                          fontSize: 10,
                          color: "var(--text-muted)",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {file}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {heartbeatLabel && (
          <div
            style={{
              padding: "8px 14px",
              borderRadius: 8,
              background: "var(--bg-elevated)",
              border: "1px solid var(--border-default)",
              display: "flex",
              alignItems: "center",
              gap: 10,
              fontSize: 12,
              color: "var(--text-secondary)",
            }}
          >
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: "50%",
                background: "#2563eb",
                boxShadow: "0 0 6px rgba(37, 99, 235, 0.6)",
                flexShrink: 0,
              }}
            />
            <span>{heartbeatLabel}</span>
          </div>
        )}

        <div className="section-card">
          <div className="section-card-title">
            <RefreshCw size={16} />
            {uiText("运行摘要", "Run Summary", "実行サマリー")}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: 14 }}>
            <SummaryItem label={uiText("摘要", "Summary", "要約")} value={currentSummary} strong />
            <SummaryItem label={uiText("运行 ID", "Run ID", "実行 ID")} value={status.currentRunId || "--"} />
            <SummaryItem
              label={uiText("会话 ID", "Session ID", "セッション ID")}
              value={
                status.sessionId || uiText("等待 Codex 创建会话", "Waiting for Codex session", "Codex セッション待機中")
              }
            />
            <SummaryItem
              label={uiText("任务文档", "Task File", "タスクファイル")}
              value={status.taskFile ? shortenPath(status.taskFile) : uiText("尚未选择", "Not selected", "未選択")}
            />
            <SummaryItem
              label={uiText("工作目录", "Working Directory", "作業ディレクトリ")}
              value={
                status.workdir
                  ? shortenPath(status.workdir)
                  : uiText("按启动时自动解析", "Resolved on start", "開始時に自動解決")
              }
            />
            <SummaryItem
              label={uiText("主日志", "Main Log", "メインログ")}
              value={status.mainLogPath ? shortenPath(status.mainLogPath, 52) : "--"}
            />
            <SummaryItem
              label={uiText("开始时间", "Started At", "開始時刻")}
              value={formatDateTime(status.startedAt)}
            />
            <SummaryItem
              label={uiText("结束时间", "Finished At", "終了時刻")}
              value={formatDateTime(status.finishedAt)}
            />
          </div>
          {status.lastMessagePreview && (
            <div
              style={{
                marginTop: 14,
                padding: 12,
                borderRadius: 10,
                background: "var(--bg-elevated)",
                border: "1px solid var(--border-default)",
              }}
            >
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 600,
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.04em",
                  marginBottom: 6,
                }}
              >
                {uiText("最近回复摘要", "Latest Reply Preview", "最新の応答要約")}
              </div>
              <div style={{ fontSize: 13, lineHeight: 1.6, color: "var(--text-secondary)" }}>
                {status.lastMessagePreview}
              </div>
            </div>
          )}
          {status.lastError && status.status === "failed" && (
            <div
              style={{
                marginTop: 14,
                padding: 12,
                borderRadius: 10,
                background: "var(--danger-subtle)",
                border: "1px solid rgba(220, 38, 38, 0.18)",
                color: "var(--danger)",
              }}
            >
              {status.lastError}
            </div>
          )}
        </div>

        <div className="section-card">
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 12,
              alignItems: "center",
              marginBottom: 12,
              flexWrap: "wrap",
            }}
          >
            <div
              className="section-card-title"
              style={{ marginBottom: 0, display: "flex", alignItems: "center", gap: 8 }}
            >
              <Bot size={16} />
              {uiText("实时输出", "Live Output", "リアルタイム出力")}
              <span style={{ fontSize: 11, color: "var(--text-muted)", fontWeight: 400 }}>
                {liveEvents.length > 0 ? `(${liveEvents.length})` : ""}
              </span>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <button
                className="btn btn-ghost btn-xs"
                onClick={() => setLiveEvents([])}
                disabled={liveEvents.length === 0}
              >
                {uiText("清空", "Clear", "クリア")}
              </button>
              <button className="btn btn-ghost btn-xs" onClick={() => setLiveOutputOpen((v) => !v)}>
                {liveOutputOpen ? uiText("收起", "Collapse", "折りたたむ") : uiText("展开", "Expand", "展開")}
              </button>
            </div>
          </div>
          {liveOutputOpen &&
            (liveEvents.length === 0 ? (
              <div
                style={{
                  padding: "24px 12px",
                  textAlign: "center",
                  fontSize: 12,
                  color: "var(--text-muted)",
                }}
              >
                {uiText(
                  "Autopilot 启动后，这里会实时显示 Codex 输出与轮次进度",
                  "Codex output and round progress will stream here once Autopilot starts",
                  "Autopilot 起動後、Codex の出力とラウンド進捗がここに流れます",
                )}
              </div>
            ) : (
              <div
                ref={liveListRef}
                style={{
                  maxHeight: 320,
                  overflowY: "auto",
                  fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)",
                  fontSize: 11.5,
                  lineHeight: 1.55,
                  background: "var(--bg-app)",
                  border: "1px solid var(--border-subtle)",
                  borderRadius: 8,
                  padding: "10px 12px",
                }}
              >
                {liveEvents.map((evt) => (
                  <LiveEventRow key={evt.id} event={evt} uiText={uiText} />
                ))}
              </div>
            ))}
        </div>

        <div className="section-card">
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 12,
              alignItems: "center",
              marginBottom: 12,
              flexWrap: "wrap",
            }}
          >
            <div className="section-card-title" style={{ marginBottom: 0 }}>
              <Bot size={16} />
              {uiText("阶段进度", "Stage Timeline", "段階タイムライン")}
            </div>
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {uiText(
                "只保留阶段级别信息，不直接显示原始执行日志",
                "Only stage-level updates are shown here, not raw execution logs",
                "ここでは生ログではなく段階レベルの更新のみ表示します",
              )}
            </div>
          </div>

          {stageItems.length === 0 ? (
            <div className="empty-state" style={{ padding: "42px 20px" }}>
              <div className="empty-icon">
                <Bot size={24} style={{ color: "var(--text-muted)" }} />
              </div>
              <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>
                {uiText("还没有阶段记录", "No stage updates yet", "段階更新はまだありません")}
              </p>
            </div>
          ) : (
            <div className="stagger" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {stageItems.map((item) => (
                <div
                  key={`${item.at}-${item.phase}-${item.message}`}
                  style={{
                    display: "grid",
                    gridTemplateColumns: "160px 1fr auto",
                    gap: 10,
                    padding: 12,
                    borderRadius: 10,
                    border: "1px solid var(--border-default)",
                    background: "var(--bg-card)",
                    alignItems: "center",
                  }}
                >
                  <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
                    {getPhaseLabel(item.phase, uiText)}
                  </div>
                  <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>{item.message}</div>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", whiteSpace: "nowrap" }}>
                    {item.attempt
                      ? `${uiText("第", "Run ", "第")} ${item.attempt} ${uiText("轮", "", "回")}`
                      : formatDateTime(item.at)}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="section-card">
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 12,
              alignItems: "center",
              marginBottom: 12,
              flexWrap: "wrap",
            }}
          >
            <div>
              <div className="section-card-title" style={{ marginBottom: 4 }}>
                <FolderOpen size={16} />
                {uiText("日志记录", "Run History", "実行履歴")}
              </div>
              <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                {uiText(`共 ${logsCount} 条记录`, `${logsCount} record(s)`, `${logsCount} 件の記録`)}
              </div>
            </div>
            <button
              className="btn btn-danger-ghost btn-sm"
              onClick={() => setDialog({ type: "clear" })}
              style={{ gap: 6 }}
              disabled={logs.length === 0 || busyRunId === "clear"}
            >
              <Trash2 size={14} />
              {uiText("清理全部", "Clear All", "すべて削除")}
            </button>
          </div>

          {logs.length === 0 ? (
            <div className="empty-state" style={{ padding: "42px 20px" }}>
              <div className="empty-icon">
                <FolderOpen size={24} style={{ color: "var(--text-muted)" }} />
              </div>
              <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>
                {uiText("还没有历史日志", "No run history yet", "履歴はまだありません")}
              </p>
              <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8 }}>
                {uiText(
                  "启动一次任务后，这里会展示每次运行的阶段摘要和管理操作。",
                  "Run a task once to see summaries and log management here.",
                  "一度実行すると、ここに要約と管理操作が表示されます。",
                )}
              </p>
            </div>
          ) : (
            <div className="stagger" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {logs.map((item) => {
                const itemStatusTone = getStatusTone(item.status);
                const isDeleting = busyRunId === item.currentRunId;
                return (
                  <div
                    key={item.currentRunId || item.runDir}
                    style={{
                      padding: 14,
                      borderRadius: 12,
                      border: "1px solid var(--border-default)",
                      background: "var(--bg-card)",
                    }}
                  >
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        gap: 14,
                        alignItems: "flex-start",
                        flexWrap: "wrap",
                      }}
                    >
                      <div style={{ minWidth: 0 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                          <span style={{ fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
                            {item.taskName || leafName(item.taskFile)}
                          </span>
                          <span
                            style={{
                              padding: "3px 8px",
                              borderRadius: 999,
                              background: "var(--bg-elevated)",
                              color: itemStatusTone,
                              fontSize: 11,
                              fontWeight: 700,
                            }}
                          >
                            {getStatusLabel(item.status, uiText)}
                          </span>
                          {item.running && (
                            <span
                              style={{
                                padding: "3px 8px",
                                borderRadius: 999,
                                background: "rgba(37, 99, 235, 0.12)",
                                color: "#2563eb",
                                fontSize: 11,
                                fontWeight: 700,
                              }}
                            >
                              {uiText("运行中", "Running", "実行中")}
                            </span>
                          )}
                        </div>
                        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 6 }}>
                          {formatDateTime(item.startedAt)} {" · "} {uiText("阶段", "Phase", "段階")}:{" "}
                          {getPhaseLabel(item.phase, uiText)} {" · "} {uiText("轮次", "Attempt", "回数")}:{" "}
                          {item.attempt || 0}
                        </div>
                        <div style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: 10, lineHeight: 1.6 }}>
                          {item.summary || item.message || "--"}
                        </div>
                        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 8 }}>
                          {uiText("工作目录", "Working Directory", "作業ディレクトリ")}: {leafName(item.workdir)} ·{" "}
                          {shortenPath(item.workdir, 34)}
                        </div>
                      </div>
                      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => void handleOpenRunDir(item.runDir)}
                          style={{ gap: 6 }}
                        >
                          <FolderOpen size={14} />
                          {uiText("打开目录", "Open Folder", "フォルダを開く")}
                        </button>
                        <button
                          className="btn btn-danger-ghost btn-sm"
                          onClick={() =>
                            setDialog({
                              type: "delete",
                              runId: item.currentRunId || "",
                              name: item.taskName || leafName(item.taskFile),
                            })
                          }
                          style={{ gap: 6 }}
                          disabled={!item.currentRunId || isDeleting || item.running}
                        >
                          <Trash2 size={14} />
                          {uiText("删除", "Delete", "削除")}
                        </button>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
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

function TaskFileList(props: {
  files: string[];
  label: string;
  description: string;
  addLabel: string;
  clearLabel: string;
  emptyLabel: string;
  onPick: () => void;
  onRemove: (idx: number) => void;
  onMove: (idx: number, direction: -1 | 1) => void;
  onClear: () => void;
}) {
  const { files } = props;
  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
        <label className="field-label" style={{ marginBottom: 0 }}>
          {props.label}
          {files.length > 0 && (
            <span style={{ marginLeft: 8, fontSize: 11, color: "var(--text-muted)", fontWeight: 400 }}>
              ({files.length})
            </span>
          )}
        </label>
        <div style={{ display: "flex", gap: 6 }}>
          {files.length > 0 && (
            <button className="btn btn-ghost btn-xs" onClick={props.onClear}>
              {props.clearLabel}
            </button>
          )}
          <button className="btn btn-secondary btn-sm" onClick={props.onPick} style={{ gap: 6 }}>
            <FolderOpen size={13} />
            {props.addLabel}
          </button>
        </div>
      </div>
      <p style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>{props.description}</p>
      {files.length === 0 ? (
        <div
          style={{
            padding: "14px 12px",
            border: "1px dashed var(--border-default)",
            borderRadius: 8,
            fontSize: 12,
            color: "var(--text-muted)",
            textAlign: "center",
          }}
        >
          {props.emptyLabel}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {files.map((f, idx) => {
            const parts = f.split(/[\\/]/).filter(Boolean);
            const name = parts[parts.length - 1] || f;
            return (
              <div
                key={`${f}-${idx}`}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  padding: "6px 10px",
                  borderRadius: 6,
                  background: "var(--bg-elevated)",
                  border: "1px solid var(--border-subtle)",
                }}
              >
                <span
                  style={{
                    width: 22,
                    height: 22,
                    borderRadius: "50%",
                    background: "var(--bg-app)",
                    display: "inline-flex",
                    alignItems: "center",
                    justifyContent: "center",
                    fontSize: 11,
                    fontWeight: 600,
                    color: "var(--text-secondary)",
                    flexShrink: 0,
                  }}
                >
                  {idx + 1}
                </span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div
                    style={{
                      fontSize: 13,
                      fontWeight: 500,
                      color: "var(--text-primary)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {name}
                  </div>
                  <div
                    style={{
                      fontSize: 10,
                      color: "var(--text-muted)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {f}
                  </div>
                </div>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => props.onMove(idx, -1)}
                  disabled={idx === 0}
                  title="Up"
                >
                  ↑
                </button>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => props.onMove(idx, 1)}
                  disabled={idx === files.length - 1}
                  title="Down"
                >
                  ↓
                </button>
                <button className="btn btn-ghost btn-icon-sm" onClick={() => props.onRemove(idx)} title="Remove">
                  <Trash2 size={13} />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function PathField(props: {
  label: string;
  value: string;
  placeholder: string;
  buttonLabel: string;
  onChange: (value: string) => void;
  onPick: () => void;
}) {
  return (
    <div>
      <label className="field-label">{props.label}</label>
      <div style={{ display: "flex", gap: 8 }}>
        <input
          className="input"
          value={props.value}
          onChange={(event) => props.onChange(event.target.value)}
          placeholder={props.placeholder}
          style={{ flex: 1 }}
        />
        <button className="btn btn-secondary btn-sm" onClick={props.onPick} style={{ gap: 6, flexShrink: 0 }}>
          <FolderOpen size={14} />
          {props.buttonLabel}
        </button>
      </div>
    </div>
  );
}

function TextField(props: { label: string; value: string; placeholder: string; onChange: (value: string) => void }) {
  return (
    <div>
      <label className="field-label">{props.label}</label>
      <input
        className="input"
        value={props.value}
        onChange={(event) => props.onChange(event.target.value)}
        placeholder={props.placeholder}
      />
    </div>
  );
}

function ToggleOption(props: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "10px 12px",
        borderRadius: 10,
        border: "1px solid var(--border-default)",
        background: "var(--bg-card)",
        cursor: "pointer",
      }}
    >
      <input type="checkbox" checked={props.checked} onChange={(event) => props.onChange(event.target.checked)} />
      <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>{props.label}</span>
    </label>
  );
}

function MetricCard(props: { title: string; value: string; tone: string; icon: ReactNode }) {
  return (
    <div className="card" style={{ padding: 16, minWidth: 0 }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center" }}>
        <div
          style={{
            fontSize: 12,
            color: "var(--text-muted)",
            minWidth: 0,
            flex: 1,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
          title={props.title}
        >
          {props.title}
        </div>
        <div style={{ color: props.tone }}>{props.icon}</div>
      </div>
      <div
        style={{
          marginTop: 10,
          fontSize: 18,
          fontWeight: 700,
          color: props.tone,
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={props.value}
      >
        {props.value}
      </div>
    </div>
  );
}

function SummaryItem(props: { label: string; value: string; strong?: boolean }) {
  return (
    <div>
      <div className="field-label" style={{ marginBottom: 6 }}>
        {props.label}
      </div>
      <div
        style={{
          fontSize: props.strong ? 15 : 13,
          fontWeight: props.strong ? 600 : 400,
          color: props.strong ? "var(--text-primary)" : "var(--text-secondary)",
          lineHeight: 1.6,
        }}
      >
        {props.value}
      </div>
    </div>
  );
}

// 把 codex --json 的事件对象压成一行友好摘要。不认识的字段就回落到 JSON 单行。
function summarizeCodexJson(value: unknown): { label: string; tone: string; detail: string } {
  const tone = "var(--text-secondary)";
  if (!value || typeof value !== "object") {
    return { label: "event", tone, detail: typeof value === "string" ? value : JSON.stringify(value) };
  }
  const obj = value as Record<string, unknown>;
  // codex v1: { type, msg: {...} } 或 { type, role, content }
  const type = typeof obj.type === "string" ? (obj.type as string) : "";
  const msg = (obj.msg as Record<string, unknown> | undefined) ?? undefined;
  const content = (obj.content as unknown) ?? (msg?.content as unknown);
  if (type.includes("delta") || type.includes("token")) {
    const text = typeof content === "string" ? content : ((msg?.text as string) ?? "");
    return { label: "delta", tone: "var(--text-primary)", detail: text };
  }
  if (type.includes("tool") && type.includes("call")) {
    const name = (msg?.name as string) ?? (obj.name as string) ?? "tool";
    return { label: `tool→${name}`, tone: "#2563eb", detail: JSON.stringify(obj.arguments ?? msg?.arguments ?? "") };
  }
  if (type.includes("tool") && (type.includes("result") || type.includes("output"))) {
    const out = JSON.stringify(obj.output ?? msg?.output ?? obj.result ?? msg?.result ?? "");
    return { label: "tool←", tone: "#059669", detail: out };
  }
  if (type.includes("turn") || type.includes("complete")) {
    return { label: type, tone: "#059669", detail: JSON.stringify(msg ?? obj) };
  }
  if (type.includes("error")) {
    return { label: type, tone: "#dc2626", detail: JSON.stringify(msg ?? obj) };
  }
  if (type) {
    return { label: type, tone, detail: JSON.stringify(msg ?? obj) };
  }
  // 无 type 字段：直接把整对象单行序列化
  return { label: "event", tone, detail: JSON.stringify(obj) };
}

function LiveEventRow({
  event,
  uiText,
}: {
  event: LiveEvent;
  uiText: (zh: string, en: string, ja?: string) => string;
}) {
  const rowBase: React.CSSProperties = {
    display: "flex",
    gap: 8,
    padding: "3px 0",
    borderBottom: "1px dashed var(--border-subtle)",
    whiteSpace: "pre-wrap",
    wordBreak: "break-word",
  };
  const labelStyle: React.CSSProperties = {
    flexShrink: 0,
    minWidth: 72,
    fontSize: 10,
    fontWeight: 700,
    letterSpacing: "0.04em",
    textTransform: "uppercase",
    color: "var(--text-muted)",
    paddingTop: 1,
  };

  if (event.kind === "round-start") {
    const p = event.payload;
    const modeText =
      p.mode === "initial"
        ? uiText("启动新会话", "initial", "新規セッション")
        : uiText("续跑会话", "resume", "継続セッション");
    return (
      <div
        style={{
          ...rowBase,
          background: "var(--accent-subtle)",
          padding: "6px 8px",
          borderRadius: 6,
          borderBottom: "none",
          margin: "6px 0",
        }}
      >
        <span style={{ ...labelStyle, color: "var(--accent)" }}>ROUND {p.attempt}</span>
        <span style={{ color: "var(--text-primary)", fontWeight: 600 }}>
          {uiText(
            `第 ${p.attempt} 轮开始（${modeText}）`,
            `Round ${p.attempt} started (${modeText})`,
            `ラウンド ${p.attempt} 開始（${modeText}）`,
          )}
        </span>
      </div>
    );
  }

  if (event.kind === "round-end") {
    const p = event.payload;
    const sec = Math.floor(p.elapsed_ms / 1000);
    const ok = p.return_code === 0;
    return (
      <div
        style={{
          ...rowBase,
          padding: "6px 8px",
          borderRadius: 6,
          borderBottom: "none",
          margin: "6px 0",
          background: ok ? "rgba(5, 150, 105, 0.08)" : "rgba(220, 38, 38, 0.08)",
        }}
      >
        <span style={{ ...labelStyle, color: ok ? "#059669" : "#dc2626" }}>END {p.attempt}</span>
        <span style={{ color: "var(--text-primary)" }}>
          {uiText(
            `第 ${p.attempt} 轮结束 · 退出码 ${p.return_code} · 耗时 ${sec}s`,
            `Round ${p.attempt} ended · exit ${p.return_code} · ${sec}s`,
            `ラウンド ${p.attempt} 終了 · exit ${p.return_code} · ${sec}s`,
          )}
        </span>
      </div>
    );
  }

  if (event.kind === "stage") {
    return (
      <div style={rowBase}>
        <span style={{ ...labelStyle, color: "var(--accent)" }}>{event.payload.phase}</span>
        <span style={{ color: "var(--text-secondary)" }}>{event.payload.message}</span>
      </div>
    );
  }

  // codex stdout / stderr
  const { kind, raw, json } = event.payload;
  if (kind === "stderr") {
    return (
      <div style={rowBase}>
        <span style={{ ...labelStyle, color: "#dc2626" }}>STDERR</span>
        <span style={{ color: "#dc2626" }}>{raw ?? ""}</span>
      </div>
    );
  }
  if (json !== undefined) {
    const s = summarizeCodexJson(json);
    return (
      <div style={rowBase}>
        <span style={{ ...labelStyle, color: s.tone }}>{s.label}</span>
        <span style={{ color: "var(--text-secondary)" }}>{s.detail}</span>
      </div>
    );
  }
  return (
    <div style={rowBase}>
      <span style={labelStyle}>stdout</span>
      <span style={{ color: "var(--text-secondary)" }}>{raw ?? ""}</span>
    </div>
  );
}
