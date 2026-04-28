import { useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Terminal, Code, Download, RefreshCw, Trash2 } from "lucide-react";
import Hello2ccConfigSection, { type Hello2ccConfigField } from "../components/Hello2ccConfigSection";
import ToolsChoiceCard from "../components/ToolsChoiceCard";
import ToolsCheckboxSection from "../components/ToolsCheckboxSection";
import ToolsCheckboxRow from "../components/ToolsCheckboxRow";
import ToolsChoiceRow from "../components/ToolsChoiceRow";
import { type Hello2ccSelectOption } from "../components/Hello2ccSelectField";
import { getLocale } from "../lib/i18n";
import ToolsEmptyStateCard from "../components/ToolsEmptyStateCard";
import ToolsManagedSectionHeader from "../components/ToolsManagedSectionHeader";
import ToolsPermissionCard from "../components/ToolsPermissionCard";
import ToolsTabButton from "../components/ToolsTabButton";
import ToolsToggleCard from "../components/ToolsToggleCard";
import { showToast } from "../components/Toast";
import { type ManagedAppId } from "../lib/appPreferences";

const ProxyAdvancedPanel = lazy(() => import("./ProxyAdvanced"));
import { useSetClaudeHudConfigMutation, useSetHello2ccConfigMutation } from "../hooks/mutations";
import {
  useDetectTools,
  fetchToolsPageData,
  queryKeys,
  useHello2ccStatus,
  useHudStatus,
  type Hello2ccConfigQueryResult,
  type Hello2ccStatusQueryResult,
  type ToolSettingsQueryResult,
} from "../hooks/queries";

type ToolTab = "claude" | "codex";

interface HudStatus {
  installed: boolean;
  version: string;
  indexJsPath: string;
  statuslineEnabled: boolean;
  hudConfig: HudConfig;
}

interface HudConfig {
  lineLayout?: "compact" | "expanded";
  showSeparators?: boolean;
  pathLevels?: number;
  elementOrder?: string[];
  gitStatus?: {
    enabled?: boolean;
    showDirty?: boolean;
    showAheadBehind?: boolean;
    showFileStats?: boolean;
  };
  display?: {
    showModel?: boolean;
    showProject?: boolean;
    showContextBar?: boolean;
    contextValue?: "percent" | "tokens" | "remaining" | "both";
    showConfigCounts?: boolean;
    showDuration?: boolean;
    showSpeed?: boolean;
    showUsage?: boolean;
    usageBarEnabled?: boolean;
    showTokenBreakdown?: boolean;
    showTools?: boolean;
    showAgents?: boolean;
    showTodos?: boolean;
    showSessionName?: boolean;
    showClaudeCodeVersion?: boolean;
    showMemoryUsage?: boolean;
  };
}

type Hello2ccConfig = Hello2ccConfigQueryResult;
type Hello2ccStatus = Hello2ccStatusQueryResult;
type Hello2ccSelectKey = Exclude<keyof Hello2ccConfig, "mirror_session_model">;
type HudGitStatusKey = keyof NonNullable<HudConfig["gitStatus"]>;
type HudDisplayBooleanKey = Exclude<keyof NonNullable<HudConfig["display"]>, "contextValue">;

interface Hello2ccUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
}

const DEFAULT_HUD_CONFIG: HudConfig = {
  lineLayout: "expanded",
  showSeparators: false,
  pathLevels: 1,
  gitStatus: { enabled: true, showDirty: true, showAheadBehind: false, showFileStats: false },
  display: {
    showModel: true, showProject: true, showContextBar: true, contextValue: "percent",
    showConfigCounts: false, showDuration: false, showSpeed: false,
    showUsage: true, usageBarEnabled: true, showTokenBreakdown: true,
    showTools: false, showAgents: false, showTodos: false,
    showSessionName: false, showClaudeCodeVersion: false, showMemoryUsage: false,
  },
};

const DEFAULT_HELLO2CC_CONFIG: Hello2ccConfig = {
  routing_policy: "native-inject",
  mirror_session_model: true,
  default_agent_model: "",
  primary_model: "",
  subagent_model: "",
  guide_model: "",
  explore_model: "",
  plan_model: "",
  general_model: "",
  team_model: "",
  compatibility_mode: "full",
};

const PERM_LEVELS = [
  { label_zh: "严格", label_en: "Strict", label_ja: "厳格", color: "#ef4444" },
  { label_zh: "标准", label_en: "Standard", label_ja: "標準", color: "#eab308" },
  { label_zh: "宽松", label_en: "Relaxed", label_ja: "緩和", color: "#3b82f6" },
  { label_zh: "全部允许", label_en: "Allow All", label_ja: "すべて許可", color: "#22c55e" },
];

const PERM_DESC_ZH = ["每次操作都确认", "允许读取，写操作确认", "允许读写，仅 Bash 确认", "跳过所有确认"];
const PERM_DESC_EN = ["Confirm every action", "Allow read, confirm write", "Allow read/write, confirm Bash", "Skip all prompts"];
const PERM_DESC_JA = ["毎回すべて確認", "読み取りを許可し、書き込みは確認", "読み書きを許可し、Bash のみ確認", "すべての確認をスキップ"];
export default function Tools() {
  const queryClient = useQueryClient();
  const cachedToolsPageData = queryClient.getQueryData<Awaited<ReturnType<typeof fetchToolsPageData>>>(
    queryKeys.toolsPage,
  );
  const [tab, setTab] = useState<ToolTab>("claude");
  const [permLevel, setPermLevel] = useState(cachedToolsPageData?.permissionsLevel ?? 0);
  const [autoUpdate, setAutoUpdate] = useState(cachedToolsPageData?.autoUpdateChannel ?? "latest");
  const [claudeModel, setClaudeModel] = useState(cachedToolsPageData?.claudeModel ?? "");
  const [toolSearch, setToolSearch] = useState(cachedToolsPageData?.toolSearchEnabled ?? false);
  const [codexApproval, setCodexApproval] = useState(cachedToolsPageData?.codexSettings.approval_mode ?? "suggest");
  const [codexReasoning, setCodexReasoning] = useState(cachedToolsPageData?.codexSettings.reasoning_effort ?? "medium");
  const [codexDisableStorage, setCodexDisableStorage] = useState(cachedToolsPageData?.codexSettings.disable_response_storage ?? false);
  const [codexContextWindow1M, setCodexContextWindow1M] = useState(cachedToolsPageData?.codexSettings.context_window_1m ?? false);
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>(
    cachedToolsPageData?.visibleApps ?? ["claude", "codex", "gemini", "opencode", "openclaw", "hermes"],
  );
  const [loading, setLoading] = useState(!cachedToolsPageData);
  const [hudInstalling, setHudInstalling] = useState(false);
  const [hudUpdateInfo, setHudUpdateInfo] = useState<{ currentVersion: string; latestVersion: string; hasUpdate: boolean } | null>(null);
  const [hudUpdating, setHudUpdating] = useState(false);
  const [hudChecking, setHudChecking] = useState(false);
  const [hello2ccInstalling, setHello2ccInstalling] = useState(false);
  const [hello2ccUninstalling, setHello2ccUninstalling] = useState(false);
  const [hello2ccUpdating, setHello2ccUpdating] = useState(false);
  const [hello2ccChecking, setHello2ccChecking] = useState(false);
  const [hello2ccToggling, setHello2ccToggling] = useState(false);
  const [hello2ccUpdateInfo, setHello2ccUpdateInfo] = useState<Hello2ccUpdateInfo | null>(null);
  const [hello2ccDraft, setHello2ccDraft] = useState<Hello2ccConfig>(DEFAULT_HELLO2CC_CONFIG);
  const pendingPermLevelRef = useRef<number | null>(null);
  const { data: tools = [] } = useDetectTools();
  const { data: rawHudStatus, refetch: refetchHudStatus } = useHudStatus();
  const { data: hello2ccStatus, refetch: refetchHello2ccStatus } = useHello2ccStatus();
  const setHudConfigMutation = useSetClaudeHudConfigMutation();
  const setHello2ccConfigMutation = useSetHello2ccConfigMutation();
  const locale = getLocale();
  const uiText = useCallback((zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  ), [locale]);
  const hudStatus = migrateHudConfig((rawHudStatus as HudStatus | null) ?? null);

  const toolById = useMemo(
    () => new Map(tools.map((tool) => [tool.id, tool])),
    [tools],
  );
  const visibleTabs = useMemo(
    () => (["claude", "codex"] as ToolTab[]).filter((id) => visibleApps.includes(id)),
    [visibleApps],
  );
  const visibleTabItems = useMemo(
    () => visibleTabs.map((id) => ({
      id,
      label: toolById.get(id)?.name || id,
      installed: toolById.get(id)?.installed ?? false,
      Icon: id === "claude" ? Terminal : Code,
    })),
    [toolById, visibleTabs],
  );
  const activeTabInstalled = toolById.get(tab)?.installed ?? false;

  const loadData = useCallback(async (options: { force?: boolean } = {}) => {
    const { force = false } = options;
    if (!queryClient.getQueryData(queryKeys.toolsPage)) {
      setLoading(true);
    }
    try {
      const data = await queryClient.fetchQuery({
        queryKey: queryKeys.toolsPage,
        queryFn: fetchToolsPageData,
        staleTime: force ? 0 : 30_000,
      });
      setPermLevel(data.permissionsLevel);
      setAutoUpdate(data.autoUpdateChannel);
      setClaudeModel(data.claudeModel);
      setToolSearch(data.toolSearchEnabled);
      setCodexApproval(data.codexSettings.approval_mode);
      setCodexReasoning(data.codexSettings.reasoning_effort);
      setCodexDisableStorage(data.codexSettings.disable_response_storage);
      setCodexContextWindow1M(data.codexSettings.context_window_1m);
      setVisibleApps(data.visibleApps);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }, [queryClient]);

  useEffect(() => { void loadData(); }, [loadData]);
  useEffect(() => {
    if (visibleTabs.length === 0) return;
    if (!visibleTabs.includes(tab)) setTab(visibleTabs[0]);
  }, [tab, visibleTabs]);
  useEffect(() => {
    setHello2ccDraft(hello2ccStatus?.config ?? DEFAULT_HELLO2CC_CONFIG);
  }, [hello2ccStatus]);

  const patchToolsPageCache = useCallback(
    (partial: Partial<ToolSettingsQueryResult>) => {
      queryClient.setQueryData<ToolSettingsQueryResult>(queryKeys.toolsPage, (prev) =>
        prev ? { ...prev, ...partial } : prev,
      );
    },
    [queryClient],
  );

  const patchCodexCache = useCallback(
    (partial: Partial<ToolSettingsQueryResult["codexSettings"]>) => {
      queryClient.setQueryData<ToolSettingsQueryResult>(queryKeys.toolsPage, (prev) =>
        prev ? { ...prev, codexSettings: { ...prev.codexSettings, ...partial } } : prev,
      );
    },
    [queryClient],
  );

  const setClaudeSetting = useCallback(async <T,>(fn: string, args: Record<string, unknown>, onSuccess: (value: T) => void, syncCache?: (value: T) => void) => {
    try {
      const value = await invoke<T>(fn, args);
      onSuccess(value);
      syncCache?.(value);
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) { showToast("error", `${e}`); }
  }, [uiText]);

  const setCodex = useCallback(async (key: string, value: string) => {
    try {
      await invoke("set_codex_setting", { key, value });
      // Keep the react-query cache in sync; otherwise remounting the page within
      // staleTime (30s) seeds local state from the stale cache and the select
      // visually reverts to the pre-save value.
      switch (key) {
        case "approval_mode":
          patchCodexCache({ approval_mode: value });
          break;
        case "reasoning_effort":
          patchCodexCache({ reasoning_effort: value });
          break;
        case "disable_response_storage":
          patchCodexCache({ disable_response_storage: value === "true" });
          break;
        case "context_window_1m":
          patchCodexCache({ context_window_1m: value === "true" });
          break;
      }
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) { showToast("error", `${e}`); }
  }, [patchCodexCache, uiText]);

  const handleInstallHud = useCallback(async () => {
    setHudInstalling(true);
    try {
      await invoke("install_claude_hud");
      await refetchHudStatus();
      showToast("success", uiText("claude-hud 安装成功", "claude-hud installed", "claude-hud をインストールしました"));
    } catch (e) {
      showToast("error", uiText(`安装失败: ${e}`, `Install failed: ${e}`, `インストールに失敗しました: ${e}`));
    } finally {
      setHudInstalling(false);
    }
  }, [refetchHudStatus, uiText]);

  const checkHudUpdate = useCallback(async () => {
    setHudChecking(true);
    try {
      const info = await invoke<{ currentVersion: string; latestVersion: string; hasUpdate: boolean }>("check_claude_hud_update");
      setHudUpdateInfo(info);
      if (!info.hasUpdate) {
        showToast("success", uiText("已是最新版本", "Already up to date", "すでに最新です"));
      }
    } catch (e) {
      showToast("error", uiText(`检查更新失败: ${e}`, `Check failed: ${e}`, `更新確認に失敗しました: ${e}`));
    } finally {
      setHudChecking(false);
    }
  }, [uiText]);

  const handleUpdateHud = useCallback(async () => {
    setHudUpdating(true);
    try {
      const result = await invoke<{ version: string; skipped: boolean }>("update_claude_hud");
      await refetchHudStatus();
      setHudUpdateInfo(null);
      if (result.skipped) {
        showToast("success", uiText("已是最新版本", "Already up to date", "すでに最新です"));
      } else {
        showToast("success", uiText(`已更新到 v${result.version}`, `Updated to v${result.version}`, `v${result.version} に更新しました`));
      }
    } catch (e) {
      showToast("error", uiText(`更新失败: ${e}`, `Update failed: ${e}`, `更新に失敗しました: ${e}`));
    } finally {
      setHudUpdating(false);
    }
  }, [refetchHudStatus, uiText]);

  const updateHello2ccDraft = useCallback(<K extends keyof Hello2ccConfig,>(key: K, value: Hello2ccConfig[K]) => {
    setHello2ccDraft(prev => ({ ...prev, [key]: value }));
  }, []);

  const handleInstallHello2cc = useCallback(async () => {
    setHello2ccInstalling(true);
    try {
      await invoke("install_hello2cc");
      setHello2ccUpdateInfo(null);
      await refetchHello2ccStatus();
      showToast("success", uiText("hello2cc 安装成功", "hello2cc installed", "hello2cc をインストールしました"));
    } catch (e) {
      showToast("error", uiText(`安装失败: ${e}`, `Install failed: ${e}`, `インストールに失敗しました: ${e}`));
    } finally {
      setHello2ccInstalling(false);
    }
  }, [refetchHello2ccStatus, uiText]);

  const handleUninstallHello2cc = useCallback(async () => {
    setHello2ccUninstalling(true);
    try {
      await invoke("uninstall_hello2cc");
      setHello2ccUpdateInfo(null);
      await refetchHello2ccStatus();
      showToast("success", uiText("hello2cc 已卸载", "hello2cc uninstalled", "hello2cc をアンインストールしました"));
    } catch (e) {
      showToast("error", uiText(`卸载失败: ${e}`, `Uninstall failed: ${e}`, `アンインストールに失敗しました: ${e}`));
    } finally {
      setHello2ccUninstalling(false);
    }
  }, [refetchHello2ccStatus, uiText]);

  const toggleHello2ccEnabled = useCallback(async (enabled: boolean) => {
    setHello2ccToggling(true);
    try {
      await invoke("set_hello2cc_enabled", { enabled });
      await refetchHello2ccStatus();
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) {
      showToast("error", `${e}`);
    } finally {
      setHello2ccToggling(false);
    }
  }, [refetchHello2ccStatus, uiText]);

  const checkHello2ccUpdate = useCallback(async () => {
    setHello2ccChecking(true);
    try {
      const info = await invoke<Hello2ccUpdateInfo>("check_hello2cc_update");
      setHello2ccUpdateInfo(info);
      if (!info.hasUpdate) {
        showToast("success", uiText("已是最新版本", "Already up to date", "すでに最新です"));
      }
    } catch (e) {
      showToast("error", uiText(`检查更新失败: ${e}`, `Check failed: ${e}`, `更新確認に失敗しました: ${e}`));
    } finally {
      setHello2ccChecking(false);
    }
  }, [uiText]);

  const handleUpdateHello2cc = useCallback(async () => {
    setHello2ccUpdating(true);
    try {
      const status = await invoke<Hello2ccStatus>("update_hello2cc");
      setHello2ccUpdateInfo(null);
      await refetchHello2ccStatus();
      showToast("success", uiText(`已更新到 v${status.version}`, `Updated to v${status.version}`, `v${status.version} に更新しました`));
    } catch (e) {
      showToast("error", uiText(`更新失败: ${e}`, `Update failed: ${e}`, `更新に失敗しました: ${e}`));
    } finally {
      setHello2ccUpdating(false);
    }
  }, [refetchHello2ccStatus, uiText]);

  const handleSaveHello2ccConfig = useCallback(async () => {
    try {
      const status = await setHello2ccConfigMutation.mutateAsync(hello2ccDraft);
      setHello2ccDraft(status.config);
      showToast("success", uiText("hello2cc 配置已保存", "hello2cc config saved", "hello2cc 設定を保存しました"));
    } catch (e) {
      showToast("error", `${e}`);
    }
  }, [hello2ccDraft, setHello2ccConfigMutation, uiText]);

  /** Migrate legacy `layout` field to `lineLayout` + `showSeparators` */
  function migrateHudConfig(status: HudStatus | null): HudStatus | null {
    if (!status?.hudConfig) return status;
    const cfg = status.hudConfig as HudConfig & { layout?: string };
    if ("layout" in cfg && !cfg.lineLayout) {
      if (cfg.layout === "separators") {
        cfg.lineLayout = "compact";
        cfg.showSeparators = true;
      } else {
        cfg.lineLayout = "compact";
        cfg.showSeparators = false;
      }
      delete cfg.layout;
    }
    return { ...status, hudConfig: cfg };
  }

  const toggleStatusLine = useCallback(async (enabled: boolean) => {
    try {
      await invoke("set_claude_statusline", { enabled });
      await refetchHudStatus();
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) { showToast("error", `${e}`); }
  }, [refetchHudStatus, uiText]);

  const updateHudConfig = useCallback(async (patch: Partial<HudConfig>) => {
    if (!hudStatus) return;
    const current = hudStatus.hudConfig || DEFAULT_HUD_CONFIG;
    const updated: HudConfig = {
      ...current,
      ...patch,
      gitStatus: { ...current.gitStatus, ...patch.gitStatus },
      display: { ...current.display, ...patch.display },
    };
    try {
      await setHudConfigMutation.mutateAsync(updated);
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) { showToast("error", `${e}`); }
  }, [hudStatus, setHudConfigMutation, uiText]);

  const commitPermLevel = useCallback(async (nextLevel = pendingPermLevelRef.current ?? permLevel) => {
    pendingPermLevelRef.current = null;
    await setClaudeSetting<number>(
      "set_claude_permissions_level",
      { level: nextLevel },
      setPermLevel,
      (v) => patchToolsPageCache({ permissionsLevel: v }),
    );
  }, [patchToolsPageCache, permLevel, setClaudeSetting]);

  const unavailableLabel = useMemo(() => uiText("未安装", "N/A", "未インストール"), [uiText]);
  const permLevelLabels = useMemo(
    () => PERM_LEVELS.map((level) => uiText(level.label_zh, level.label_en, level.label_ja)),
    [uiText],
  );
  const autoUpdateOptions = useMemo(
    () => [
      { value: "latest", label: uiText("最新", "Latest", "最新") },
      { value: "stable", label: uiText("稳定", "Stable", "安定版") },
      { value: "disabled", label: uiText("关闭", "Off", "オフ") },
    ],
    [uiText],
  );
  const claudeModelOptions = useMemo(
    () => [
      { value: "opus", label: "Opus" },
      { value: "sonnet", label: "Sonnet" },
      { value: "haiku", label: "Haiku" },
    ],
    [],
  );
  const hudLayoutOptions = useMemo(
    () => [
      { value: "expanded", label: uiText("多行展开", "Expanded", "展開表示"), style: { fontSize: 11 } },
      { value: "compact", label: uiText("单行紧凑", "Compact", "コンパクト"), style: { fontSize: 11 } },
    ],
    [uiText],
  );
  const hudPathLevelOptions = useMemo(
    () => [1, 2, 3].map((value) => ({ value, label: String(value), style: { fontSize: 11, minWidth: 24 } })),
    [],
  );
  const hudContextValueOptions = useMemo(
    () => [
      { value: "percent", label: "45%", style: { fontSize: 11 } },
      { value: "tokens", label: "45k/200k", style: { fontSize: 11 } },
      { value: "remaining", label: uiText("剩余", "Remain", "残り"), style: { fontSize: 11 } },
      { value: "both", label: uiText("全部", "Both", "両方"), style: { fontSize: 11 } },
    ],
    [uiText],
  );
  const codexApprovalOptions = useMemo(
    () => [
      { value: "suggest", label: uiText("建议", "Suggest", "提案") },
      { value: "auto-edit", label: uiText("自动编辑", "Auto Edit", "自動編集") },
      { value: "full-auto", label: uiText("全自动", "Full Auto", "フルオート") },
    ],
    [uiText],
  );
  const codexReasoningOptions = useMemo(
    () => [
      { value: "low", label: uiText("低", "Low", "低") },
      { value: "medium", label: uiText("中", "Medium", "中") },
      { value: "high", label: uiText("高", "High", "高") },
      { value: "xhigh", label: uiText("极高", "XHigh", "最高") },
    ],
    [uiText],
  );
  const permLevelOptions = useMemo(
    () => PERM_LEVELS.map((level, index) => ({ value: index, label: permLevelLabels[index], color: level.color })),
    [permLevelLabels],
  );
  const hudGitStatusOptions = useMemo(
    () => [
      { key: "enabled" as HudGitStatusKey, label: uiText("显示分支", "Branch", "ブランチ表示"), defaultValue: true },
      { key: "showDirty" as HudGitStatusKey, label: uiText("未提交标记", "Dirty Mark", "変更あり表示"), defaultValue: true },
      { key: "showAheadBehind" as HudGitStatusKey, label: uiText("领先/落后", "Ahead/Behind", "先行/遅延"), defaultValue: false },
      { key: "showFileStats" as HudGitStatusKey, label: uiText("文件统计", "File Stats", "ファイル統計"), defaultValue: false },
    ],
    [uiText],
  );
  const hudDisplayOptions = useMemo(
    () => [
      { key: "showModel" as HudDisplayBooleanKey, label: uiText("模型名", "Model", "モデル名"), defaultValue: true },
      { key: "showProject" as HudDisplayBooleanKey, label: uiText("项目路径", "Project Path", "プロジェクトパス"), defaultValue: true },
      { key: "showContextBar" as HudDisplayBooleanKey, label: uiText("上下文进度条", "Context Bar", "コンテキストバー"), defaultValue: true },
      { key: "showConfigCounts" as HudDisplayBooleanKey, label: uiText("配置计数", "Config Counts", "設定数"), defaultValue: false },
      { key: "showDuration" as HudDisplayBooleanKey, label: uiText("会话时长", "Duration", "継続時間"), defaultValue: false },
      { key: "showSpeed" as HudDisplayBooleanKey, label: uiText("输出速度", "Output Speed", "出力速度"), defaultValue: false },
      { key: "showUsage" as HudDisplayBooleanKey, label: uiText("用量限制", "Usage", "使用量"), defaultValue: true },
      { key: "usageBarEnabled" as HudDisplayBooleanKey, label: uiText("用量进度条", "Usage Bar", "使用量バー"), defaultValue: true },
      { key: "showTokenBreakdown" as HudDisplayBooleanKey, label: uiText("Token 明细", "Token Detail", "トークン詳細"), defaultValue: true },
      { key: "showTools" as HudDisplayBooleanKey, label: uiText("工具活动", "Tools", "ツール活動"), defaultValue: false },
      { key: "showAgents" as HudDisplayBooleanKey, label: uiText("Agent 活动", "Agents", "Agent 活動"), defaultValue: false },
      { key: "showTodos" as HudDisplayBooleanKey, label: uiText("Todo 进度", "Todos", "Todo 進捗"), defaultValue: false },
      { key: "showSessionName" as HudDisplayBooleanKey, label: uiText("会话名称", "Session Name", "セッション名"), defaultValue: false },
      { key: "showClaudeCodeVersion" as HudDisplayBooleanKey, label: uiText("CC 版本号", "CC Version", "CC バージョン"), defaultValue: false },
      { key: "showMemoryUsage" as HudDisplayBooleanKey, label: uiText("内存占用", "Memory Usage", "メモリ使用量"), defaultValue: false },
    ],
    [uiText],
  );
  const hello2ccRoutingOptions = useMemo<Hello2ccSelectOption[]>(
    () => [
      { value: "native-inject", label: "native-inject" },
      { value: "prompt-only", label: "prompt-only" },
    ],
    [],
  );
  const hello2ccCompatibilityOptions = useMemo<Hello2ccSelectOption[]>(
    () => [
      { value: "full", label: "full" },
      { value: "sanitize-only", label: "sanitize-only" },
    ],
    [],
  );
  const hello2ccModelOptions = useMemo<Hello2ccSelectOption[]>(
    () => [
      { value: "", label: uiText("留空", "Blank", "空欄") },
      { value: "inherit", label: "inherit" },
      { value: "opus", label: "opus" },
      { value: "sonnet", label: "sonnet" },
      { value: "haiku", label: "haiku" },
    ],
    [uiText],
  );
  const hello2ccModelFields = useMemo(
    () => [
      { fieldKey: "default_agent_model" as Hello2ccSelectKey, label: uiText("默认 Agent 槽位", "Default Agent Slot", "既定 Agent スロット"), description: uiText("统一默认值", "Global default", "全体デフォルト") },
      { fieldKey: "primary_model" as Hello2ccSelectKey, label: uiText("Primary Model", "Primary Model", "Primary Model"), description: uiText("高能力 Agent", "High-capability agents", "高能力 Agent") },
      { fieldKey: "subagent_model" as Hello2ccSelectKey, label: uiText("Subagent Model", "Subagent Model", "Subagent Model"), description: uiText("未指定模型的 Agent", "Agents without explicit model", "未指定モデルの Agent") },
      { fieldKey: "guide_model" as Hello2ccSelectKey, label: uiText("Guide Model", "Guide Model", "Guide Model"), description: "Claude Code Guide" },
      { fieldKey: "explore_model" as Hello2ccSelectKey, label: uiText("Explore Model", "Explore Model", "Explore Model"), description: "Explore" },
      { fieldKey: "plan_model" as Hello2ccSelectKey, label: uiText("Plan Model", "Plan Model", "Plan Model"), description: "Plan" },
      { fieldKey: "general_model" as Hello2ccSelectKey, label: uiText("General Model", "General Model", "General Model"), description: "General-Purpose" },
      { fieldKey: "team_model" as Hello2ccSelectKey, label: uiText("Team Model", "Team Model", "Team Model"), description: uiText("团队 teammate", "Team teammates", "チーム teammate") },
    ],
    [uiText],
  );
  const noVisibleTabsTitle = useMemo(
    () => uiText("当前已隐藏所有工具页签", "All tool tabs are currently hidden", "すべてのツールタブは現在非表示です"),
    [uiText],
  );
  const noVisibleTabsDescription = useMemo(
    () => uiText("可在设置页的 App 可见性中重新开启", "Re-enable them from Settings > App Visibility", "Settings > App Visibility から再表示できます"),
    [uiText],
  );
  const notInstalledTitle = useMemo(
    () => uiText(
      `${tab === "claude" ? "Claude Code" : "Codex CLI"} 未安装`,
      `${tab === "claude" ? "Claude Code" : "Codex CLI"} not installed`,
      `${tab === "claude" ? "Claude Code" : "Codex CLI"} は未インストールです`,
    ),
    [tab, uiText],
  );
  const notInstalledDescription = useMemo(
    () => uiText("安装后即可在此管理工具设置", "Install it to manage settings here", "インストール後にここで设置を管理できます"),
    [uiText],
  );
  const handleSelectTab = useCallback((value: string) => {
    setTab(value as ToolTab);
  }, []);
  const handleSelectPermLevel = useCallback((value: string | number) => {
    const nextLevel = Number(value);
    setPermLevel(nextLevel);
    pendingPermLevelRef.current = nextLevel;
    void commitPermLevel(nextLevel);
  }, [commitPermLevel]);
  const handleSelectAutoUpdate = useCallback((value: string | number) => {
    const nextValue = String(value);
    setAutoUpdate(nextValue);
    void setClaudeSetting<string>(
      "set_claude_auto_update",
      { channel: nextValue },
      setAutoUpdate,
      (v) => patchToolsPageCache({ autoUpdateChannel: v }),
    );
  }, [patchToolsPageCache, setClaudeSetting]);
  const handleSelectClaudeModel = useCallback((value: string | number) => {
    const nextValue = String(value);
    setClaudeModel(nextValue);
    void setClaudeSetting<string>(
      "set_claude_model",
      { model: nextValue },
      setClaudeModel,
      (v) => patchToolsPageCache({ claudeModel: v }),
    );
  }, [patchToolsPageCache, setClaudeSetting]);
  const handleSelectHudLayout = useCallback((value: string | number) => {
    void updateHudConfig({ lineLayout: value as HudConfig["lineLayout"] });
  }, [updateHudConfig]);
  const handleSelectHudPathLevel = useCallback((value: string | number) => {
    void updateHudConfig({ pathLevels: Number(value) });
  }, [updateHudConfig]);
  const handleSelectHudContextValue = useCallback((value: string | number) => {
    void updateHudConfig({ display: { contextValue: value as NonNullable<HudConfig["display"]>["contextValue"] } });
  }, [updateHudConfig]);
  const handleToggleHudGitStatus = useCallback((key: string, checked: boolean) => {
    void updateHudConfig({ gitStatus: { [key]: checked } as Partial<NonNullable<HudConfig["gitStatus"]>> });
  }, [updateHudConfig]);
  const handleToggleHudDisplay = useCallback((key: string, checked: boolean) => {
    void updateHudConfig({ display: { [key]: checked } as Partial<NonNullable<HudConfig["display"]>> });
  }, [updateHudConfig]);
  const handleSelectCodexApproval = useCallback((value: string | number) => {
    const nextValue = String(value);
    setCodexApproval(nextValue);
    void setCodex("approval_mode", nextValue);
  }, [setCodex]);
  const handleSelectCodexReasoning = useCallback((value: string | number) => {
    const nextValue = String(value);
    setCodexReasoning(nextValue);
    void setCodex("reasoning_effort", nextValue);
  }, [setCodex]);
  const handleToggleBypassPermissions = useCallback((enabled: boolean) => {
    const nextLevel = enabled ? 3 : 0;
    setPermLevel(nextLevel);
    pendingPermLevelRef.current = nextLevel;
    void commitPermLevel(nextLevel);
  }, [commitPermLevel]);
  const handleToggleToolSearch = useCallback((enabled: boolean) => {
    setToolSearch(enabled);
    void setClaudeSetting<boolean>(
      "set_claude_tool_search",
      { enabled },
      setToolSearch,
      (v) => patchToolsPageCache({ toolSearchEnabled: v }),
    );
  }, [patchToolsPageCache, setClaudeSetting]);
  const handleChangePermLevelRange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    const nextLevel = Number(event.target.value);
    pendingPermLevelRef.current = nextLevel;
    setPermLevel(nextLevel);
  }, []);
  const handleCommitPermLevelPointerUp = useCallback(() => {
    void commitPermLevel();
  }, [commitPermLevel]);
  const handleCommitPermLevelBlur = useCallback(() => {
    void commitPermLevel();
  }, [commitPermLevel]);
  const handleCommitPermLevelKeyUp = useCallback((event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key.startsWith("Arrow") || event.key === "Home" || event.key === "End") {
      void commitPermLevel();
    }
  }, [commitPermLevel]);
  const handleInstallHudClick = useCallback(() => {
    void handleInstallHud();
  }, [handleInstallHud]);
  const handleUpdateHudClick = useCallback(() => {
    void handleUpdateHud();
  }, [handleUpdateHud]);
  const handleCheckHudUpdateClick = useCallback(() => {
    void checkHudUpdate();
  }, [checkHudUpdate]);
  const handleToggleHudSeparators = useCallback((checked: boolean) => {
    void updateHudConfig({ showSeparators: checked });
  }, [updateHudConfig]);
  const handleChangeHello2ccSelect = useCallback((fieldKey: string, value: string) => {
    updateHello2ccDraft(fieldKey as Hello2ccSelectKey, value as Hello2ccConfig[Hello2ccSelectKey]);
  }, [updateHello2ccDraft]);
  const handleToggleHello2ccMirrorSessionModel = useCallback((enabled: boolean) => {
    updateHello2ccDraft("mirror_session_model", enabled);
  }, [updateHello2ccDraft]);
  const handleToggleCodexDisableStorage = useCallback((enabled: boolean) => {
    setCodexDisableStorage(enabled);
    void setCodex("disable_response_storage", String(enabled));
  }, [setCodex]);
  const handleToggleCodexContextWindow1M = useCallback((enabled: boolean) => {
    setCodexContextWindow1M(enabled);
    void setCodex("context_window_1m", String(enabled));
  }, [setCodex]);
  const hello2ccConfigSource = hello2ccStatus?.config ?? DEFAULT_HELLO2CC_CONFIG;
  const handleInstallHello2ccClick = useCallback(() => {
    void handleInstallHello2cc();
  }, [handleInstallHello2cc]);
  const handleUpdateHello2ccClick = useCallback(() => {
    void handleUpdateHello2cc();
  }, [handleUpdateHello2cc]);
  const handleCheckHello2ccUpdateClick = useCallback(() => {
    void checkHello2ccUpdate();
  }, [checkHello2ccUpdate]);
  const handleUninstallHello2ccClick = useCallback(() => {
    void handleUninstallHello2cc();
  }, [handleUninstallHello2cc]);
  const handleResetHello2ccDraft = useCallback(() => {
    setHello2ccDraft(hello2ccConfigSource);
  }, [hello2ccConfigSource]);
  const handleSaveHello2ccConfigClick = useCallback(() => {
    void handleSaveHello2ccConfig();
  }, [handleSaveHello2ccConfig]);
  const perm = PERM_LEVELS[permLevel] || PERM_LEVELS[0];
  const hc = hudStatus?.hudConfig || DEFAULT_HUD_CONFIG;
  const hudResolvedGitStatusOptions = useMemo(
    () => hudGitStatusOptions.map((option) => ({
      key: option.key,
      label: option.label,
      checked: hc.gitStatus?.[option.key] ?? option.defaultValue,
    })),
    [hc.gitStatus, hudGitStatusOptions],
  );
  const hudResolvedDisplayOptions = useMemo(
    () => hudDisplayOptions.map((option) => ({
      key: option.key,
      label: option.label,
      checked: hc.display?.[option.key] ?? option.defaultValue,
    })),
    [hc.display, hudDisplayOptions],
  );
  const hudInstallAction = useMemo(
    () => ({
      label: uiText("安装", "Install", "インストール"),
      icon: Download,
      pending: hudInstalling,
      onClick: handleInstallHudClick,
      disabled: hudInstalling,
      variant: "primary" as const,
      gap: 5,
    }),
    [handleInstallHudClick, hudInstalling, uiText],
  );
  const hudPrimaryAction = useMemo(
    () => (hudUpdateInfo?.hasUpdate
      ? {
        label: uiText(`更新到 v${hudUpdateInfo.latestVersion}`, `Update to v${hudUpdateInfo.latestVersion}`, `v${hudUpdateInfo.latestVersion} に更新`),
        icon: Download,
        pending: hudUpdating,
        onClick: handleUpdateHudClick,
        disabled: hudUpdating,
        variant: "primary" as const,
        gap: 5,
      }
      : {
        label: uiText("检查更新", "Check Update", "更新を確認"),
        icon: RefreshCw,
        pending: hudChecking,
        onClick: handleCheckHudUpdateClick,
        disabled: hudChecking,
        title: uiText("检查更新", "Check for updates", "更新を確認"),
      }),
    [handleCheckHudUpdateClick, handleUpdateHudClick, hudChecking, hudUpdateInfo, hudUpdating, uiText],
  );
  const hudToggle = useMemo(
    () => ({
      value: hudStatus?.statuslineEnabled ?? false,
      onChange: toggleStatusLine,
      labelOn: uiText("已启用", "Enabled", "有効"),
      labelOff: uiText("已关闭", "Disabled", "無効"),
    }),
    [hudStatus?.statuslineEnabled, toggleStatusLine, uiText],
  );
  const hello2ccInstallAction = useMemo(
    () => ({
      label: uiText("安装", "Install", "インストール"),
      icon: Download,
      pending: hello2ccInstalling,
      onClick: handleInstallHello2ccClick,
      disabled: hello2ccInstalling,
      variant: "primary" as const,
      gap: 5,
    }),
    [handleInstallHello2ccClick, hello2ccInstalling, uiText],
  );
  const hello2ccPrimaryAction = useMemo(
    () => (hello2ccUpdateInfo?.hasUpdate
      ? {
        label: uiText(`更新到 v${hello2ccUpdateInfo.latestVersion}`, `Update to v${hello2ccUpdateInfo.latestVersion}`, `v${hello2ccUpdateInfo.latestVersion} に更新`),
        icon: Download,
        pending: hello2ccUpdating,
        onClick: handleUpdateHello2ccClick,
        disabled: hello2ccUpdating,
        variant: "primary" as const,
        gap: 5,
      }
      : {
        label: uiText("检查更新", "Check Update", "更新を確認"),
        icon: RefreshCw,
        pending: hello2ccChecking,
        onClick: handleCheckHello2ccUpdateClick,
        disabled: hello2ccChecking,
      }),
    [handleCheckHello2ccUpdateClick, handleUpdateHello2ccClick, hello2ccChecking, hello2ccUpdateInfo, hello2ccUpdating, uiText],
  );
  const hello2ccSecondaryAction = useMemo(
    () => ({
      label: uiText("卸载", "Uninstall", "アンインストール"),
      icon: Trash2,
      pending: hello2ccUninstalling,
      onClick: handleUninstallHello2ccClick,
      disabled: hello2ccUninstalling,
    }),
    [handleUninstallHello2ccClick, hello2ccUninstalling, uiText],
  );
  const hello2ccToggle = useMemo(
    () => ({
      value: hello2ccStatus?.enabled ?? false,
      onChange: toggleHello2ccEnabled,
      labelOn: hello2ccToggling ? uiText("处理中", "Updating", "更新中") : uiText("已启用", "Enabled", "有効"),
      labelOff: hello2ccToggling ? uiText("处理中", "Updating", "更新中") : uiText("已关闭", "Disabled", "無効"),
    }),
    [hello2ccStatus?.enabled, hello2ccToggling, toggleHello2ccEnabled, uiText],
  );
  const hello2ccHasChanges = JSON.stringify(hello2ccDraft) !== JSON.stringify(hello2ccConfigSource);
  const permDescription = uiText(PERM_DESC_ZH[permLevel], PERM_DESC_EN[permLevel], PERM_DESC_JA[permLevel]);
  const hello2ccSelectFields = useMemo<Hello2ccConfigField[]>(
    () => [
      {
        fieldKey: "routing_policy",
        label: uiText("路由策略", "Routing Policy", "ルーティングポリシー"),
        description: uiText("决定是否在原生 Agent 调用前注入模型槽位", "Choose whether native agent calls receive silent model injection", "ネイティブ Agent 呼び出し前にモデル注入するかを選びます"),
        value: hello2ccDraft.routing_policy,
        options: hello2ccRoutingOptions,
      },
      {
        fieldKey: "compatibility_mode",
        label: uiText("兼容模式", "Compatibility Mode", "互換モード"),
        description: uiText("与其他插件共存时可切到仅净化模式", "Use sanitize-only when coexisting with other orchestration plugins", "他プラグインと共存する場合は sanitize-only を使います"),
        value: hello2ccDraft.compatibility_mode,
        options: hello2ccCompatibilityOptions,
      },
      ...hello2ccModelFields.map((field) => ({
        fieldKey: field.fieldKey,
        label: field.label,
        description: field.description,
        value: hello2ccDraft[field.fieldKey],
        options: hello2ccModelOptions,
      })),
    ],
    [hello2ccCompatibilityOptions, hello2ccDraft, hello2ccModelFields, hello2ccModelOptions, hello2ccRoutingOptions, uiText],
  );

  if (loading) {
    return <div className="loading-center"><div className="spinner" /><span style={{ fontSize: 13, color: "var(--text-muted)" }}>{uiText("加载中...", "Loading...", "読み込み中...")}</span></div>;
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{uiText("工具", "Tools", "ツール")}</h2>
          <p className="page-subtitle">{uiText("管理 AI 编程工具的配置和权限", "Manage AI coding tool settings", "AI コーディングツールの設定と権限を管理")}</p>
        </div>
      </div>

        {/* Tabs */}
      <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
        {visibleTabItems.map(({ id, label, installed, Icon }) => {
          return (
            <ToolsTabButton
              key={id}
              tabId={id}
              label={label}
              icon={<Icon size={14} />}
              active={tab === id}
              installed={installed}
              unavailableLabel={unavailableLabel}
              onSelect={handleSelectTab}
            />
          );
        })}
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {visibleTabs.length === 0 && (
          <ToolsEmptyStateCard
            title={noVisibleTabsTitle}
            description={noVisibleTabsDescription}
            marginBottom={12}
          />
        )}

        {/* Not installed hint */}
        {visibleTabs.length > 0 && !activeTabInstalled && (
          <ToolsEmptyStateCard
            title={notInstalledTitle}
            description={notInstalledDescription}
          />
        )}

        {tab === "claude" && toolById.get("claude")?.installed && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {/* Permission Slider */}
            <ToolsPermissionCard
              title={uiText("权限模式", "Permission Mode", "権限モード")}
              currentLabel={uiText(perm.label_zh, perm.label_en, perm.label_ja)}
              currentDescription={permDescription}
              currentColor={perm.color}
              value={permLevel}
              options={permLevelOptions}
              onSelect={handleSelectPermLevel}
              onRangeChange={handleChangePermLevelRange}
              onRangePointerUp={handleCommitPermLevelPointerUp}
              onRangeKeyUp={handleCommitPermLevelKeyUp}
              onRangeBlur={handleCommitPermLevelBlur}
            />

            {/* Bypass Permissions */}
            <ToolsToggleCard
              title={uiText("绕过权限确认", "Bypass Permissions", "権限確認をバイパス")}
              description={uiText("跳过所有权限确认，全自动执行", "Skip all permission prompts, fully autonomous", "すべての権限確認をスキップして完全自動で実行します")}
              value={permLevel === 3}
              onChange={handleToggleBypassPermissions}
              labelOn="ON"
              labelOff="OFF"
            />

            {/* Auto Update */}
            <ToolsChoiceCard
              title={uiText("自动更新", "Auto Update", "自動更新")}
              description={uiText("Claude Code 更新频道", "Update channel", "Claude Code の更新チャンネル")}
              value={autoUpdate}
              onSelect={handleSelectAutoUpdate}
              options={autoUpdateOptions}
            />

            {/* Model Selection */}
            <ToolsChoiceCard
              title={uiText("模型选择", "Model", "モデル")}
              description={uiText("切换默认使用的模型", "Switch default model", "既定モデルを切り替えます")}
              value={claudeModelOptions.find((option) => claudeModel.includes(String(option.value)))?.value ?? ""}
              onSelect={handleSelectClaudeModel}
              options={claudeModelOptions}
            />

            {/* Tool Search */}
            <ToolsToggleCard
              title="Tool Search"
              description={uiText("启用工具搜索功能（实验性）", "Enable tool search (experimental)", "ツール検索機能を有効化します（実験的）")}
              value={toolSearch}
              onChange={handleToggleToolSearch}
              labelOn={uiText("已启用", "Enabled", "有効")}
              labelOff={uiText("已关闭", "Disabled", "無効")}
            />

            {/* StatusLine (claude-hud) */}
            <div className="card" style={{ padding: "16px 18px" }}>
              <ToolsManagedSectionHeader
                title="StatusLine (claude-hud)"
                description={uiText("终端底部实时状态栏", "Real-time status bar at terminal bottom", "ターミナル下部のリアルタイムステータスバー")}
                version={hudStatus?.version}
                installed={hudStatus?.installed ?? false}
                installAction={hudInstallAction}
                primaryAction={hudPrimaryAction}
                toggle={hudToggle}
              />

              {hudStatus?.installed && (
                <div style={{ borderTop: "1px solid var(--border)", paddingTop: 12, display: "flex", flexDirection: "column", gap: 14 }}>
                  {/* Layout */}
                  <ToolsChoiceRow
                    title={uiText("布局模式", "Layout Mode", "レイアウトモード")}
                    value={hc.lineLayout || "expanded"}
                    onSelect={handleSelectHudLayout}
                    options={hudLayoutOptions}
                  />

                  {/* Separators */}
                  <ToolsCheckboxRow
                    title={uiText("分隔线", "Separators", "区切り線")}
                    label={uiText("活动区域前显示分隔线", "Show separator before activity", "アクティビティの前に区切り線を表示")}
                    checked={hc.showSeparators === true}
                    onChange={handleToggleHudSeparators}
                  />

                  {/* Path Levels */}
                  <ToolsChoiceRow
                    title={uiText("路径层级", "Path Levels", "パス階層")}
                    value={hc.pathLevels || 1}
                    onSelect={handleSelectHudPathLevel}
                    options={hudPathLevelOptions}
                  />

                  {/* Context Value Format */}
                  <ToolsChoiceRow
                    title={uiText("上下文格式", "Context Format", "コンテキスト形式")}
                    value={hc.display?.contextValue || "percent"}
                    onSelect={handleSelectHudContextValue}
                    options={hudContextValueOptions}
                  />

                  {/* Git Status */}
                  <ToolsCheckboxSection
                    title="Git Status"
                    options={hudResolvedGitStatusOptions}
                    onToggle={handleToggleHudGitStatus}
                  />

                  {/* Display Options */}
                  <ToolsCheckboxSection
                    title={uiText("显示选项", "Display", "表示項目")}
                    options={hudResolvedDisplayOptions}
                    onToggle={handleToggleHudDisplay}
                  />
                </div>
              )}
            </div>

            {/* hello2cc */}
            <div className="card" style={{ padding: "16px 18px" }}>
              <ToolsManagedSectionHeader
                title="hello2cc"
                description={uiText("让第三方模型更接近 Claude Code 原生工作流", "Make third-party models behave more like native Claude Code", "サードパーティーモデルを Claude Code ネイティブに近づけます")}
                version={hello2ccStatus?.version}
                installed={hello2ccStatus?.installed ?? false}
                installAction={hello2ccInstallAction}
                primaryAction={hello2ccPrimaryAction}
                secondaryAction={hello2ccSecondaryAction}
                toggle={hello2ccToggle}
                actionsWrap
              />

              {hello2ccStatus?.installed && (
                <Hello2ccConfigSection
                  pathLabel={uiText("插件缓存目录", "Plugin cache path", "プラグインキャッシュパス")}
                  installPath={hello2ccStatus.installPath}
                  fields={hello2ccSelectFields}
                  onSelectChange={handleChangeHello2ccSelect}
                  mirrorTitle={uiText("镜像当前会话模型", "Mirror Session Model", "現在のセッションモデルをミラー")}
                  mirrorDescription={uiText("缺少显式模型时优先跟随当前会话模型槽位", "Prefer the current session model when no explicit slot is set", "明示的なモデルがない場合は現在のセッションモデルを優先します")}
                  mirrorValue={hello2ccDraft.mirror_session_model}
                  onMirrorChange={handleToggleHello2ccMirrorSessionModel}
                  mirrorLabelOn={uiText("已启用", "Enabled", "有効")}
                  mirrorLabelOff={uiText("已关闭", "Disabled", "無効")}
                  resetLabel={uiText("重置", "Reset", "リセット")}
                  saveLabel={uiText("保存配置", "Save Config", "設定を保存")}
                  hasChanges={hello2ccHasChanges}
                  isSaving={setHello2ccConfigMutation.isPending}
                  onReset={handleResetHello2ccDraft}
                  onSave={handleSaveHello2ccConfigClick}
                />
              )}
            </div>

            {/* Proxy Advanced — 代理增强（仅 Claude） */}
            <div className="card" style={{ padding: "16px 18px" }}>
              <Suspense fallback={<div style={{ minHeight: 120, display: "flex", alignItems: "center", justifyContent: "center" }}><div className="spinner" /></div>}>
                <ProxyAdvancedPanel embedded mode="claude" />
              </Suspense>
            </div>
          </div>
        )}

        {tab === "codex" && toolById.get("codex")?.installed && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {/* Approval Mode */}
            <ToolsChoiceCard
              title={uiText("审批模式", "Approval Mode", "承認モード")}
              description={uiText("操作确认级别", "Action confirmation level", "操作確認レベル")}
              value={codexApproval}
              onSelect={handleSelectCodexApproval}
              options={codexApprovalOptions}
            />

            {/* Reasoning Effort */}
            <ToolsChoiceCard
              title={uiText("推理强度", "Reasoning Effort", "推論強度")}
              description={uiText("模型推理计算量", "Model reasoning compute", "モデルの推論計算量")}
              value={codexReasoning}
              onSelect={handleSelectCodexReasoning}
              options={codexReasoningOptions}
            />

            {/* Disable Response Storage */}
            <ToolsToggleCard
              title={uiText("禁用响应存储", "Disable Response Storage", "応答保存を無効化")}
              description={uiText("不保存 API 响应到本地", "Don't save API responses locally", "API 応答をローカルに保存しません")}
              value={codexDisableStorage}
              onChange={handleToggleCodexDisableStorage}
              labelOn={uiText("已禁用", "Disabled", "無効")}
              labelOff={uiText("已启用", "Enabled", "有効")}
            />

            <ToolsToggleCard
              title={uiText("1M 上下文窗口", "1M Context Window", "1M コンテキストウィンドウ")}
              description={uiText("一键写入 `model_context_window = 1000000`", "Write `model_context_window = 1000000` with one toggle", "`model_context_window = 1000000` をワンタップで書き込みます")}
              value={codexContextWindow1M}
              onChange={handleToggleCodexContextWindow1M}
              labelOn={uiText("已开启", "Enabled", "有効")}
              labelOff={uiText("默认", "Default", "既定")}
            />

            {/* Proxy Advanced — Codex OAuth 字段剥离 */}
            <div className="card" style={{ padding: "16px 18px" }}>
              <Suspense fallback={<div style={{ minHeight: 80, display: "flex", alignItems: "center", justifyContent: "center" }}><div className="spinner" /></div>}>
                <ProxyAdvancedPanel embedded mode="codex" />
              </Suspense>
            </div>
          </div>
        )}

      </div>
    </div>
  );
}
