/* eslint-disable @typescript-eslint/no-explicit-any */
import { type Dispatch, type SetStateAction, type RefObject } from "react";
import {
  Bot,
  FileSearch,
  FolderOpen,
  Play,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  SlidersHorizontal,
  Trash2,
} from "lucide-react";

import type { AutopilotFormState } from "../../stores/autopilotForm";
import {
  formatDateTime,
  getPhaseLabel,
  getStatusLabel,
  getStatusTone,
  leafName,
  shortenPath,
  type AutopilotStatus,
  type DialogState,
  type LiveEvent,
  type PermissionMode,
} from "./helpers";
import {
  LiveEventRow,
  MetricCard,
  PathField,
  SummaryItem,
  TaskFileList,
  TextField,
  ToggleOption,
} from "./subcomponents";

type LocaleText = (zh: string, en: string, ja?: string) => string;

export function RunSetupPanel(props: {
  uiText: LocaleText;
  form: AutopilotFormState;
  permissionMode: PermissionMode;
  updateField: <K extends keyof AutopilotFormState>(key: K, value: AutopilotFormState[K]) => void;
  updatePermissionMode: (mode: PermissionMode) => void;
  handlePickFiles: () => void;
  handleRemoveTaskFile: (idx: number) => void;
  handleMoveTaskFile: (idx: number, direction: -1 | 1) => void;
  handleClearTaskFiles: () => void;
  handlePickFolder: () => void;
  handleResetForm: () => void;
}) {
  const { uiText, form, permissionMode } = props;
  return (
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
        onPick={props.handlePickFiles}
        onRemove={props.handleRemoveTaskFile}
        onMove={props.handleMoveTaskFile}
        onClear={props.handleClearTaskFiles}
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
          onChange={(value) => props.updateField("workdir", value)}
          onPick={props.handlePickFolder}
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
          onChange={(value) => props.updateField("model", value)}
          placeholder="gpt-5.6-sol"
        />
        <TextField
          label={uiText("Profile", "Profile", "プロファイル")}
          value={form.profile}
          onChange={(value) => props.updateField("profile", value)}
          placeholder="default"
        />
        <TextField
          label={uiText("轮询间隔(秒)", "Retry Interval (s)", "間隔(秒)")}
          value={form.interval}
          onChange={(value) => props.updateField("interval", value)}
          placeholder="3"
        />
        <TextField
          label={uiText("最大轮次", "Max Attempts", "最大試行回数")}
          value={form.maxAttempts}
          onChange={(value) => props.updateField("maxAttempts", value)}
          placeholder="0"
        />
      </div>

      <div style={{ marginTop: 16 }}>
        <label className="field-label">{uiText("权限模式", "Permission Mode", "権限モード")}</label>
        <select
          className="input"
          value={permissionMode}
          onChange={(event) => props.updatePermissionMode(event.target.value as PermissionMode)}
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
            onChange={(checked) => props.updateField("fresh", checked)}
          />
          <ToggleOption
            label={uiText("Dry Run", "Dry Run", "Dry Run")}
            checked={form.dryRun}
            onChange={(checked) => props.updateField("dryRun", checked)}
          />
          <ToggleOption
            label={uiText("跳过 Git 检查", "Skip Git Check", "Git チェックをスキップ")}
            checked={form.skipGitCheck}
            onChange={(checked) => props.updateField("skipGitCheck", checked)}
          />
          <ToggleOption
            label={uiText("详细调试日志", "Verbose Debug Logs", "詳細デバッグログ")}
            checked={form.verbose}
            onChange={(checked) => props.updateField("verbose", checked)}
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
        <button className="btn btn-secondary btn-sm" onClick={props.handleResetForm} style={{ gap: 6 }}>
          <RotateCcw size={14} />
          {uiText("重置表单", "Reset Form", "フォームをリセット")}
        </button>
      </div>
    </div>
  );
}

export function MetricsRow(props: {
  uiText: LocaleText;
  status: AutopilotStatus;
  statusLabel: string;
  statusTone: string;
  phaseLabel: string;
  runtimeLabel: string;
  runtimeTone: string;
  latestRun: AutopilotStatus | null;
}) {
  const { uiText, status } = props;
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: 12 }}>
      <MetricCard
        title={uiText("当前状态", "Current Status", "現在の状態")}
        value={props.statusLabel}
        tone={props.statusTone}
        icon={<ShieldCheck size={16} />}
      />
      <MetricCard
        title={uiText("当前阶段", "Current Phase", "現在の段階")}
        value={props.phaseLabel}
        tone="var(--accent)"
        icon={<Bot size={16} />}
      />
      <MetricCard
        title={uiText("运行时长", "Runtime", "実行時間")}
        value={props.runtimeLabel}
        tone={props.runtimeTone}
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
          value={props.latestRun ? leafName(props.latestRun.taskFile || props.latestRun.taskName) : "--"}
          tone="var(--text-primary)"
          icon={<FileSearch size={16} />}
        />
      )}
    </div>
  );
}

export function TaskQueuePanel(props: { uiText: LocaleText; status: AutopilotStatus }) {
  const { uiText, status } = props;
  if (!status.taskQueue || status.taskQueue.length <= 1) return null;
  return (
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
  );
}

export function RunSummaryPanel(props: { uiText: LocaleText; status: AutopilotStatus; currentSummary: string }) {
  const { uiText, status, currentSummary } = props;
  return (
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
        <SummaryItem label={uiText("开始时间", "Started At", "開始時刻")} value={formatDateTime(status.startedAt)} />
        <SummaryItem label={uiText("结束时间", "Finished At", "終了時刻")} value={formatDateTime(status.finishedAt)} />
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
  );
}

export function LiveOutputPanel(props: {
  uiText: LocaleText;
  liveEvents: LiveEvent[];
  liveOutputOpen: boolean;
  setLiveOutputOpen: Dispatch<SetStateAction<boolean>>;
  setLiveEvents: Dispatch<SetStateAction<LiveEvent[]>>;
  liveListRef: RefObject<HTMLDivElement | null>;
}) {
  const { uiText, liveEvents, liveOutputOpen, setLiveOutputOpen, setLiveEvents, liveListRef } = props;
  return (
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
        <div className="section-card-title" style={{ marginBottom: 0, display: "flex", alignItems: "center", gap: 8 }}>
          <Bot size={16} />
          {uiText("实时输出", "Live Output", "リアルタイム出力")}
          <span style={{ fontSize: 11, color: "var(--text-muted)", fontWeight: 400 }}>
            {liveEvents.length > 0 ? `(${liveEvents.length})` : ""}
          </span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <button className="btn btn-ghost btn-xs" onClick={() => setLiveEvents([])} disabled={liveEvents.length === 0}>
            {uiText("清空", "Clear", "クリア")}
          </button>
          <button className="btn btn-ghost btn-xs" onClick={() => setLiveOutputOpen((v) => !v)}>
            {liveOutputOpen ? uiText("收起", "Collapse", "折りたたむ") : uiText("展开", "Expand", "展開")}
          </button>
        </div>
      </div>
      {liveOutputOpen &&
        (liveEvents.length === 0 ? (
          <div style={{ padding: "24px 12px", textAlign: "center", fontSize: 12, color: "var(--text-muted)" }}>
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
  );
}

export function StageTimelinePanel(props: { uiText: LocaleText; stageItems: AutopilotStatus["recentStages"] }) {
  const { uiText, stageItems } = props;
  return (
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
  );
}

export function RunHistoryPanel(props: {
  uiText: LocaleText;
  logs: AutopilotStatus[];
  busyRunId: string | null;
  setDialog: Dispatch<SetStateAction<DialogState>>;
  handleOpenRunDir: (target: string) => void;
}) {
  const { uiText, logs, busyRunId, setDialog, handleOpenRunDir } = props;
  const logsCount = logs.length;
  return (
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
  );
}

export function HeartbeatBadge(props: { label: string | null }) {
  if (!props.label) return null;
  return (
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
      <span>{props.label}</span>
    </div>
  );
}
