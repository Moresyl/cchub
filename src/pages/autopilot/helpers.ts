/* eslint-disable @typescript-eslint/no-explicit-any */
import type { AutopilotFormState } from "../../stores/autopilotForm";

export interface AutopilotStageEntry {
  at: string;
  phase: string;
  message: string;
  attempt: number | null;
}

export interface AutopilotStatus {
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

export type PermissionMode = "approval" | "fullAuto" | "bypass";

// 后端实时事件：codex 子进程 stdout/stderr + 心跳 + 轮次/阶段切换
export interface CodexEventPayload {
  kind: "stdout" | "stderr";
  attempt: number;
  json?: unknown;
  raw?: string;
}
export interface TickPayload {
  attempt: number;
  elapsed_ms: number;
  last_event_age_ms: number;
}
export interface RoundStartPayload {
  attempt: number;
  mode: "initial" | "resume";
  started_at_ms: number;
}
export interface RoundEndPayload {
  attempt: number;
  return_code: number;
  elapsed_ms: number;
}
export interface StagePayload {
  phase: string;
  message: string;
  attempt: number | null;
  at_ms: number;
}
export type LiveEvent =
  | { id: number; kind: "codex"; payload: CodexEventPayload }
  | { id: number; kind: "round-start"; payload: RoundStartPayload }
  | { id: number; kind: "round-end"; payload: RoundEndPayload }
  | { id: number; kind: "stage"; payload: StagePayload };

export type DialogState =
  | { type: "delete"; runId: string; name: string }
  | { type: "clear" }
  | { type: "start-bypass" }
  | null;

// 事件流接入后，轮询只做兜底：事件丢失/崩溃恢复/最终状态持久化
export const POLL_INTERVAL_MS = 10000;
// 实时输出面板最多保留的条数，再多就丢弃最旧的（避免内存膨胀）
export const MAX_LIVE_EVENTS = 300;
export const EMPTY_STATUS: AutopilotStatus = {
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

export function permissionModeFromForm(form: Pick<AutopilotFormState, "bypass" | "fullAuto">): PermissionMode {
  if (form.bypass) return "bypass";
  if (form.fullAuto) return "fullAuto";
  return "approval";
}

export function applyPermissionMode(mode: PermissionMode): Pick<AutopilotFormState, "bypass" | "fullAuto"> {
  switch (mode) {
    case "bypass":
      return { bypass: true, fullAuto: false };
    case "fullAuto":
      return { bypass: false, fullAuto: true };
    default:
      return { bypass: false, fullAuto: false };
  }
}

export function formatDateTime(value: string | null) {
  if (!value) return "--";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

export function formatRuntime(startedAt: string | null, finishedAt: string | null, nowTs: number) {
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

export function shortenPath(value: string, keep = 42) {
  const trimmed = value.trim();
  if (!trimmed) return "--";
  if (trimmed.length <= keep) return trimmed;
  return `${trimmed.slice(0, 18)}...${trimmed.slice(-18)}`;
}

export function leafName(value: string) {
  if (!value.trim()) return "--";
  const parts = value.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || value;
}

export function getStatusLabel(status: string, uiText: (zhText: string, enText: string, jaText?: string) => string) {
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

export function getPhaseLabel(phase: string, uiText: (zhText: string, enText: string, jaText?: string) => string) {
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

export function getStatusTone(status: string) {
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
