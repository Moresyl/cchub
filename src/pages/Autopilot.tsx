import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Bot,
  FileSearch,
  FolderOpen,
  Play,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  Square,
  Trash2,
} from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import { showToast } from "../components/Toast";
import { getLocale } from "../lib/i18n";

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
}

interface AutopilotFormState {
  taskFile: string;
  workdir: string;
  model: string;
  profile: string;
  interval: string;
  maxAttempts: string;
  codexBin: string;
  fresh: boolean;
  dryRun: boolean;
  skipGitCheck: boolean;
  bypass: boolean;
  fullAuto: boolean;
  verbose: boolean;
}

interface AutopilotClearResult {
  deletedCount: number;
}

type DialogState =
  | { type: "delete"; runId: string; name: string }
  | { type: "clear" }
  | null;

const STORAGE_KEY = "cchub-autopilot-form";
const POLL_INTERVAL_MS = 2500;
const DEFAULT_FORM: AutopilotFormState = {
  taskFile: "",
  workdir: "",
  model: "",
  profile: "",
  interval: "3",
  maxAttempts: "0",
  codexBin: "",
  fresh: false,
  dryRun: false,
  skipGitCheck: false,
  bypass: true,
  fullAuto: true,
  verbose: false,
};
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
};

function loadStoredForm(): AutopilotFormState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_FORM;
    return { ...DEFAULT_FORM, ...JSON.parse(raw) };
  } catch {
    return DEFAULT_FORM;
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

function getStatusLabel(
  status: string,
  uiText: (zhText: string, enText: string, jaText?: string) => string,
) {
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

function getPhaseLabel(
  phase: string,
  uiText: (zhText: string, enText: string, jaText?: string) => string,
) {
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
  const [form, setForm] = useState<AutopilotFormState>(() => loadStoredForm());
  const [status, setStatus] = useState<AutopilotStatus>(EMPTY_STATUS);
  const [logs, setLogs] = useState<AutopilotStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [dialog, setDialog] = useState<DialogState>(null);
  const [busyRunId, setBusyRunId] = useState<string | null>(null);
  const [nowTs, setNowTs] = useState(() => Date.now());
  const locale = getLocale();
  const uiText = useCallback((zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  ), [locale]);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(form));
  }, [form]);

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

  const loadAll = useCallback(async (silent = false) => {
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
        showToast("error", uiText(`加载 Autopilot 状态失败: ${error}`, `Failed to load Autopilot status: ${error}`, `Autopilot 状態の読み込みに失敗しました: ${error}`));
      }
    } finally {
      if (silent) {
        setRefreshing(false);
      } else {
        setLoading(false);
      }
    }
  }, [loadLogs, loadStatus, uiText]);

  useEffect(() => {
    void loadAll();
  }, [loadAll]);

  useEffect(() => {
    if (!status.running) return undefined;
    const timer = window.setInterval(() => {
      void loadAll(true);
    }, POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [loadAll, status.running]);

  useEffect(() => {
    if (!status.running) return undefined;
    const timer = window.setInterval(() => {
      setNowTs(Date.now());
    }, 1000);
    return () => window.clearInterval(timer);
  }, [status.running]);

  const updateField = useCallback(<K extends keyof AutopilotFormState>(key: K, value: AutopilotFormState[K]) => {
    setForm((current) => ({ ...current, [key]: value }));
  }, []);

  const handlePickFile = useCallback(async () => {
    try {
      const picked = await invoke<string | null>("pick_autopilot_file");
      if (picked) {
        updateField("taskFile", picked);
      }
    } catch (error) {
      showToast("error", `${error}`);
    }
  }, [updateField]);

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

  const handleStart = useCallback(async () => {
    setStarting(true);
    try {
      const nextStatus = await invoke<AutopilotStatus>("start_autopilot", {
        request: {
          taskFile: form.taskFile,
          workdir: form.workdir,
          model: form.model,
          profile: form.profile,
          interval: Number(form.interval || "0"),
          maxAttempts: Number(form.maxAttempts || "0"),
          codexBin: form.codexBin,
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

  const handleStop = useCallback(async () => {
    setStopping(true);
    try {
      const nextStatus = await invoke<AutopilotStatus>("stop_autopilot");
      setStatus(nextStatus);
      showToast("success", uiText("停止请求已发送", "Stop request sent", "停止要求を送信しました"));
      await loadAll(true);
    } catch (error) {
      console.error(error);
      showToast("error", `${error}`);
    } finally {
      setStopping(false);
    }
  }, [loadAll, uiText]);

  const handleRefresh = useCallback(() => {
    void loadAll(true);
  }, [loadAll]);

  const handleOpenLogsRoot = useCallback(async () => {
    const target = status.logsRootDir || logs[0]?.logsRootDir;
    if (!target) {
      showToast("error", uiText("日志目录尚未创建", "Log directory is not ready yet", "ログディレクトリはまだ作成されていません"));
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
    setForm(DEFAULT_FORM);
  }, []);

  const performDelete = useCallback(async (runId: string) => {
    setBusyRunId(runId);
    try {
      await invoke("delete_autopilot_log", { runId });
      showToast("success", uiText("日志记录已删除", "Log record deleted", "ログ記録を削除しました"));
      await loadAll(true);
    } catch (error) {
      showToast("error", `${error}`);
    } finally {
      setBusyRunId(null);
      setDialog(null);
    }
  }, [loadAll, uiText]);

  const performClear = useCallback(async () => {
    setBusyRunId("clear");
    try {
      const result = await invoke<AutopilotClearResult>("clear_autopilot_logs");
      showToast(
        "success",
        uiText(`已清理 ${result.deletedCount} 条日志记录`, `Removed ${result.deletedCount} log record(s)`, `${result.deletedCount} 件のログ記録を削除しました`),
      );
      await loadAll(true);
    } catch (error) {
      showToast("error", `${error}`);
    } finally {
      setBusyRunId(null);
      setDialog(null);
    }
  }, [loadAll, uiText]);

  const canStart = useMemo(
    () => !status.running && !starting && form.taskFile.trim() !== "",
    [form.taskFile, starting, status.running],
  );
  const stageItems = status.recentStages.slice().reverse();
  const currentSummary = status.summary || status.message || uiText("尚未开始执行", "No run in progress", "まだ実行されていません");
  const latestRun = logs[0] ?? null;
  const logsCount = logs.length;
  const statusLabel = getStatusLabel(status.status, uiText);
  const phaseLabel = getPhaseLabel(status.phase, uiText);
  const statusTone = getStatusTone(status.status);
  const runtimeLabel = formatRuntime(status.startedAt, status.finishedAt, nowTs);
  const runtimeTone = status.running ? "#2563eb" : "var(--text-primary)";

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
            {uiText("由 Rust 原生托管 Codex 续跑流程，统一管理阶段、运行状态与独立日志目录", "Native Rust orchestration for Codex resume loops with centralized stages and logs", "Rust ネイティブで Codex 継続実行を管理し、段階とログを一元管理します")}
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
            <button className="btn btn-danger btn-sm" onClick={() => void handleStop()} style={{ gap: 6 }} disabled={stopping}>
              <Square size={14} />
              {stopping ? uiText("停止中...", "Stopping...", "停止中...") : uiText("停止任务", "Stop Run", "停止")}
            </button>
          ) : (
            <button className="btn btn-primary btn-sm" onClick={() => void handleStart()} style={{ gap: 6 }} disabled={!canStart}>
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
            {uiText("界面只显示阶段和摘要，不直接展示原始 Codex 日志。完整日志会统一保存到专用目录，可单独打开和清理。后端直接调用 Codex，不再依赖外部 Python 脚本。", "The UI shows stages and summaries only. Full logs are stored in a dedicated folder. The backend now invokes Codex directly instead of relying on an external Python script.", "画面には段階と要約のみを表示し、生ログは専用フォルダに保存します。バックエンドは Python スクリプトではなく Codex を直接呼び出します。")}
          </p>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: 14 }}>
            <PathField
              label={uiText("任务文件", "Task File", "タスクファイル")}
              value={form.taskFile}
              placeholder={"D:\\work\\tasks\\my-task.md"}
              onChange={(value) => updateField("taskFile", value)}
              onPick={() => void handlePickFile()}
              buttonLabel={uiText("选择任务", "Pick Task", "タスクを選択")}
            />
            <PathField
              label={uiText("工作目录", "Working Directory", "作業ディレクトリ")}
              value={form.workdir}
              placeholder={uiText("留空时自动使用任务文件所在目录", "Leave empty to use the task file directory", "空欄ならタスクファイルの親ディレクトリを使います")}
              onChange={(value) => updateField("workdir", value)}
              onPick={() => void handlePickFolder()}
              buttonLabel={uiText("选择目录", "Pick Folder", "フォルダを選択")}
            />
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 12, marginTop: 14 }}>
            <TextField label={uiText("模型", "Model", "モデル")} value={form.model} onChange={(value) => updateField("model", value)} placeholder="gpt-5.4" />
            <TextField label={uiText("Profile", "Profile", "プロファイル")} value={form.profile} onChange={(value) => updateField("profile", value)} placeholder="default" />
            <TextField label={uiText("轮询间隔(秒)", "Retry Interval (s)", "間隔(秒)")} value={form.interval} onChange={(value) => updateField("interval", value)} placeholder="3" />
            <TextField label={uiText("最大轮次", "Max Attempts", "最大試行回数")} value={form.maxAttempts} onChange={(value) => updateField("maxAttempts", value)} placeholder="0" />
            <TextField label={uiText("Codex 命令", "Codex Command", "Codex コマンド")} value={form.codexBin} onChange={(value) => updateField("codexBin", value)} placeholder="codex" />
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 10, marginTop: 16 }}>
            <ToggleOption label={uiText("从头开始", "Fresh Run", "新規実行")} checked={form.fresh} onChange={(checked) => updateField("fresh", checked)} />
            <ToggleOption label={uiText("仅 Dry Run", "Dry Run", "Dry Run")} checked={form.dryRun} onChange={(checked) => updateField("dryRun", checked)} />
            <ToggleOption label={uiText("跳过 Git 检查", "Skip Git Check", "Git チェックをスキップ")} checked={form.skipGitCheck} onChange={(checked) => updateField("skipGitCheck", checked)} />
            <ToggleOption label={uiText("Bypass 审批", "Bypass Approvals", "承認をバイパス")} checked={form.bypass} onChange={(checked) => updateField("bypass", checked)} />
            <ToggleOption label={uiText("启用 Full Auto", "Full Auto", "Full Auto")} checked={form.fullAuto} onChange={(checked) => updateField("fullAuto", checked)} />
            <ToggleOption label={uiText("记录详细调试", "Verbose", "詳細ログ")} checked={form.verbose} onChange={(checked) => updateField("verbose", checked)} />
          </div>

          <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginTop: 18, flexWrap: "wrap" }}>
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {uiText("所有运行日志将统一保存到 `~/.cchub/autopilot/runs`。", "All runs are stored under `~/.cchub/autopilot/runs`.", "すべての実行ログは `~/.cchub/autopilot/runs` に保存されます。")}
            </div>
            <button className="btn btn-secondary btn-sm" onClick={handleResetForm} style={{ gap: 6 }}>
              <RotateCcw size={14} />
              {uiText("重置表单", "Reset Form", "フォームをリセット")}
            </button>
          </div>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: 12 }}>
          <MetricCard title={uiText("当前状态", "Current Status", "現在の状態")} value={statusLabel} tone={statusTone} icon={<ShieldCheck size={16} />} />
          <MetricCard title={uiText("当前阶段", "Current Phase", "現在の段階")} value={phaseLabel} tone="var(--accent)" icon={<Bot size={16} />} />
          <MetricCard title={uiText("运行时长", "Runtime", "実行時間")} value={runtimeLabel} tone={runtimeTone} icon={<Play size={16} />} />
          <MetricCard title={uiText("执行轮次", "Attempt", "実行回数")} value={status.attempt > 0 ? String(status.attempt) : "--"} tone="var(--text-primary)" icon={<RefreshCw size={16} />} />
          <MetricCard title={uiText("最近一次运行", "Latest Run", "最新の実行")} value={latestRun ? leafName(latestRun.taskFile || latestRun.taskName) : "--"} tone="var(--text-primary)" icon={<FileSearch size={16} />} />
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <RefreshCw size={16} />
            {uiText("运行摘要", "Run Summary", "実行サマリー")}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: 14 }}>
            <SummaryItem label={uiText("摘要", "Summary", "要約")} value={currentSummary} strong />
            <SummaryItem label={uiText("运行 ID", "Run ID", "実行 ID")} value={status.currentRunId || "--"} />
            <SummaryItem label={uiText("会话 ID", "Session ID", "セッション ID")} value={status.sessionId || uiText("等待 Codex 创建会话", "Waiting for Codex session", "Codex セッション待機中")} />
            <SummaryItem label={uiText("任务文档", "Task File", "タスクファイル")} value={status.taskFile ? shortenPath(status.taskFile) : uiText("尚未选择", "Not selected", "未選択")} />
            <SummaryItem label={uiText("工作目录", "Working Directory", "作業ディレクトリ")} value={status.workdir ? shortenPath(status.workdir) : uiText("按启动时自动解析", "Resolved on start", "開始時に自動解決")} />
            <SummaryItem label={uiText("主日志", "Main Log", "メインログ")} value={status.mainLogPath ? shortenPath(status.mainLogPath, 52) : "--"} />
            <SummaryItem label={uiText("开始时间", "Started At", "開始時刻")} value={formatDateTime(status.startedAt)} />
            <SummaryItem label={uiText("结束时间", "Finished At", "終了時刻")} value={formatDateTime(status.finishedAt)} />
          </div>
          {status.lastMessagePreview && (
            <div style={{ marginTop: 14, padding: 12, borderRadius: 10, background: "var(--bg-elevated)", border: "1px solid var(--border-default)" }}>
              <div style={{ fontSize: 11, fontWeight: 600, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.04em", marginBottom: 6 }}>
                {uiText("最近回复摘要", "Latest Reply Preview", "最新の応答要約")}
              </div>
              <div style={{ fontSize: 13, lineHeight: 1.6, color: "var(--text-secondary)" }}>{status.lastMessagePreview}</div>
            </div>
          )}
          {status.lastError && status.status === "failed" && (
            <div style={{ marginTop: 14, padding: 12, borderRadius: 10, background: "var(--danger-subtle)", border: "1px solid rgba(220, 38, 38, 0.18)", color: "var(--danger)" }}>
              {status.lastError}
            </div>
          )}
        </div>

        <div className="section-card">
          <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginBottom: 12, flexWrap: "wrap" }}>
            <div className="section-card-title" style={{ marginBottom: 0 }}>
              <Bot size={16} />
              {uiText("阶段进度", "Stage Timeline", "段階タイムライン")}
            </div>
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {uiText("只保留阶段级别信息，不直接显示原始执行日志", "Only stage-level updates are shown here, not raw execution logs", "ここでは生ログではなく段階レベルの更新のみ表示します")}
            </div>
          </div>

          {stageItems.length === 0 ? (
            <div className="empty-state" style={{ padding: "42px 20px" }}>
              <div className="empty-icon"><Bot size={24} style={{ color: "var(--text-muted)" }} /></div>
              <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{uiText("还没有阶段记录", "No stage updates yet", "段階更新はまだありません")}</p>
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
                  <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>{getPhaseLabel(item.phase, uiText)}</div>
                  <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>{item.message}</div>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", whiteSpace: "nowrap" }}>
                    {item.attempt ? `${uiText("第", "Run ", "第")} ${item.attempt} ${uiText("轮", "", "回")}` : formatDateTime(item.at)}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="section-card">
          <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginBottom: 12, flexWrap: "wrap" }}>
            <div>
              <div className="section-card-title" style={{ marginBottom: 4 }}>
                <FolderOpen size={16} />
                {uiText("日志记录", "Run History", "実行履歴")}
              </div>
              <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                {uiText(`共 ${logsCount} 条记录`, `${logsCount} record(s)`, `${logsCount} 件の記録`)}
              </div>
            </div>
            <button className="btn btn-danger-ghost btn-sm" onClick={() => setDialog({ type: "clear" })} style={{ gap: 6 }} disabled={logs.length === 0 || busyRunId === "clear"}>
              <Trash2 size={14} />
              {uiText("清理全部", "Clear All", "すべて削除")}
            </button>
          </div>

          {logs.length === 0 ? (
            <div className="empty-state" style={{ padding: "42px 20px" }}>
              <div className="empty-icon"><FolderOpen size={24} style={{ color: "var(--text-muted)" }} /></div>
              <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{uiText("还没有历史日志", "No run history yet", "履歴はまだありません")}</p>
              <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8 }}>{uiText("启动一次任务后，这里会展示每次运行的阶段摘要和管理操作。", "Run a task once to see summaries and log management here.", "一度実行すると、ここに要約と管理操作が表示されます。")}</p>
            </div>
          ) : (
            <div className="stagger" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {logs.map((item) => {
                const itemStatusTone = getStatusTone(item.status);
                const isDeleting = busyRunId === item.currentRunId;
                return (
                  <div key={item.currentRunId || item.runDir} style={{ padding: 14, borderRadius: 12, border: "1px solid var(--border-default)", background: "var(--bg-card)" }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 14, alignItems: "flex-start", flexWrap: "wrap" }}>
                      <div style={{ minWidth: 0 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                          <span style={{ fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>{item.taskName || leafName(item.taskFile)}</span>
                          <span style={{ padding: "3px 8px", borderRadius: 999, background: "var(--bg-elevated)", color: itemStatusTone, fontSize: 11, fontWeight: 700 }}>
                            {getStatusLabel(item.status, uiText)}
                          </span>
                          {item.running && (
                            <span style={{ padding: "3px 8px", borderRadius: 999, background: "rgba(37, 99, 235, 0.12)", color: "#2563eb", fontSize: 11, fontWeight: 700 }}>
                              {uiText("运行中", "Running", "実行中")}
                            </span>
                          )}
                        </div>
                        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 6 }}>
                          {formatDateTime(item.startedAt)} {" · "} {uiText("阶段", "Phase", "段階")}: {getPhaseLabel(item.phase, uiText)} {" · "} {uiText("轮次", "Attempt", "回数")}: {item.attempt || 0}
                        </div>
                        <div style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: 10, lineHeight: 1.6 }}>{item.summary || item.message || "--"}</div>
                        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 8 }}>
                          {uiText("工作目录", "Working Directory", "作業ディレクトリ")}: {leafName(item.workdir)} · {shortenPath(item.workdir, 34)}
                        </div>
                      </div>
                      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                        <button className="btn btn-secondary btn-sm" onClick={() => void handleOpenRunDir(item.runDir)} style={{ gap: 6 }}>
                          <FolderOpen size={14} />
                          {uiText("打开目录", "Open Folder", "フォルダを開く")}
                        </button>
                        <button className="btn btn-danger-ghost btn-sm" onClick={() => setDialog({ type: "delete", runId: item.currentRunId || "", name: item.taskName || leafName(item.taskFile) })} style={{ gap: 6 }} disabled={!item.currentRunId || isDeleting || item.running}>
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
        title={dialog?.type === "clear" ? uiText("清理全部日志", "Clear All Logs", "すべてのログを削除") : uiText("删除日志记录", "Delete Log Record", "ログ記録を削除")}
        message={dialog?.type === "clear"
          ? uiText("将删除所有已保存的 Autopilot 运行记录。正在运行的任务会被保留。", "All saved Autopilot records will be removed. Any active run will be preserved.", "保存済みの Autopilot 記録を削除します。実行中のタスクは保持されます。")
          : uiText(`确定删除「${dialog?.name || ""}」这条运行记录吗？`, `Delete the run record "${dialog?.name || ""}"?`, `「${dialog?.name || ""}」の実行記録を削除しますか？`)}
        confirmText={dialog?.type === "clear" ? uiText("全部清理", "Clear All", "すべて削除") : uiText("删除", "Delete", "削除")}
        cancelText={uiText("取消", "Cancel", "キャンセル")}
        onCancel={() => setDialog(null)}
        onConfirm={() => {
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
        <input className="input" value={props.value} onChange={(event) => props.onChange(event.target.value)} placeholder={props.placeholder} style={{ flex: 1 }} />
        <button className="btn btn-secondary btn-sm" onClick={props.onPick} style={{ gap: 6, flexShrink: 0 }}>
          <FolderOpen size={14} />
          {props.buttonLabel}
        </button>
      </div>
    </div>
  );
}

function TextField(props: {
  label: string;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
}) {
  return (
    <div>
      <label className="field-label">{props.label}</label>
      <input className="input" value={props.value} onChange={(event) => props.onChange(event.target.value)} placeholder={props.placeholder} />
    </div>
  );
}

function ToggleOption(props: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label style={{ display: "flex", alignItems: "center", gap: 10, padding: "10px 12px", borderRadius: 10, border: "1px solid var(--border-default)", background: "var(--bg-card)", cursor: "pointer" }}>
      <input type="checkbox" checked={props.checked} onChange={(event) => props.onChange(event.target.checked)} />
      <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>{props.label}</span>
    </label>
  );
}

function MetricCard(props: {
  title: string;
  value: string;
  tone: string;
  icon: ReactNode;
}) {
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

function SummaryItem(props: {
  label: string;
  value: string;
  strong?: boolean;
}) {
  return (
    <div>
      <div className="field-label" style={{ marginBottom: 6 }}>{props.label}</div>
      <div style={{ fontSize: props.strong ? 15 : 13, fontWeight: props.strong ? 600 : 400, color: props.strong ? "var(--text-primary)" : "var(--text-secondary)", lineHeight: 1.6 }}>
        {props.value}
      </div>
    </div>
  );
}
