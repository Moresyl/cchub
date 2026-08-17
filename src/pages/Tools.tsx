import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Terminal, Code, Download, RefreshCw, Trash2 } from "lucide-react";
import { type Hello2ccConfigField } from "../components/Hello2ccConfigSection";
import { getLocale } from "../lib/i18n";
import ToolsEmptyStateCard from "../components/ToolsEmptyStateCard";
import ToolsTabButton from "../components/ToolsTabButton";
import { showToast } from "../components/Toast";
import LoadingState from "../components/states/LoadingState";
import { type ManagedAppId } from "../lib/appPreferences";

import {
  useSetClaudeHudConfigMutation,
  useSetClaudeSettingMutation,
  useSetClaudeStatuslineMutation,
  useSetCodexSettingMutation,
  useSetHello2ccConfigMutation,
  useSetHello2ccEnabledMutation,
  useUpdateClaudeHudMutation,
  useUpdateHello2ccMutation,
} from "../hooks/mutations";
import {
  useDetectTools,
  fetchToolsPageData,
  queryKeys,
  useHello2ccStatus,
  useHudStatus,
  type ToolSettingsQueryResult,
} from "../hooks/queries";

import {
  DEFAULT_HELLO2CC_CONFIG,
  DEFAULT_HUD_CONFIG,
  PERM_DESC_EN,
  PERM_DESC_JA,
  PERM_DESC_ZH,
  PERM_LEVELS,
  type Hello2ccConfig,
  type Hello2ccSelectKey,
  type Hello2ccUpdateInfo,
  type HudConfig,
  type HudStatus,
  type ToolTab,
} from "./tools/helpers";
import { useToolsOptions, buildHello2ccSelectFields } from "./tools/useToolsOptions";
import ClaudeTab from "./tools/ClaudeTab";
import CodexTab from "./tools/CodexTab";

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
  const [codexDisableStorage, setCodexDisableStorage] = useState(
    cachedToolsPageData?.codexSettings.disable_response_storage ?? false,
  );
  const [codexContextWindow1M, setCodexContextWindow1M] = useState(
    cachedToolsPageData?.codexSettings.context_window_1m ?? false,
  );
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>(
    cachedToolsPageData?.visibleApps ?? [
      "claude",
      "codex",
      "gemini",
      "grokbuild",
      "opencode",
      "openclaw",
      "hermes",
      "pi",
    ],
  );
  const [loading, setLoading] = useState(!cachedToolsPageData);
  const [hudInstalling, setHudInstalling] = useState(false);
  const [hudUpdateInfo, setHudUpdateInfo] = useState<{
    currentVersion: string;
    latestVersion: string;
    hasUpdate: boolean;
  } | null>(null);
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
  const setClaudeSettingMutation = useSetClaudeSettingMutation<unknown>();
  const setCodexSettingMutation = useSetCodexSettingMutation();
  const setClaudeStatuslineMutation = useSetClaudeStatuslineMutation();
  const setHello2ccEnabledMutation = useSetHello2ccEnabledMutation();
  const updateClaudeHudMutation = useUpdateClaudeHudMutation();
  const updateHello2ccMutation = useUpdateHello2ccMutation();
  const locale = getLocale();
  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );
  const hudStatus = migrateHudConfig((rawHudStatus as HudStatus | null) ?? null);

  const toolById = useMemo(() => new Map(tools.map((tool) => [tool.id, tool])), [tools]);
  const visibleTabs = useMemo(
    () => (["claude", "codex"] as ToolTab[]).filter((id) => visibleApps.includes(id)),
    [visibleApps],
  );
  const visibleTabItems = useMemo(
    () =>
      visibleTabs.map((id) => ({
        id,
        label: toolById.get(id)?.name || id,
        installed: toolById.get(id)?.installed ?? false,
        Icon: id === "claude" ? Terminal : Code,
      })),
    [toolById, visibleTabs],
  );
  const activeTabInstalled = toolById.get(tab)?.installed ?? false;

  const loadData = useCallback(
    async (options: { force?: boolean } = {}) => {
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
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    },
    [queryClient],
  );

  useEffect(() => {
    void loadData();
  }, [loadData]);
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

  const setClaudeSetting = useCallback(
    async <T,>(
      fn: string,
      args: Record<string, unknown>,
      onSuccess: (value: T) => void,
      syncCache?: (value: T) => void,
    ) => {
      try {
        const value = (await setClaudeSettingMutation.mutateAsync({ command: fn, args })) as T;
        onSuccess(value);
        syncCache?.(value);
        showToast("success", uiText("已更新", "Updated", "更新しました"));
      } catch (e) {
        showToast("error", `${e}`);
      }
    },
    [setClaudeSettingMutation, uiText],
  );

  const setCodex = useCallback(
    async (key: string, value: string) => {
      try {
        await setCodexSettingMutation.mutateAsync({ key, value });
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
      } catch (e) {
        showToast("error", `${e}`);
      }
    },
    [patchCodexCache, setCodexSettingMutation, uiText],
  );

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
      const info = await invoke<{ currentVersion: string; latestVersion: string; hasUpdate: boolean }>(
        "check_claude_hud_update",
      );
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
      const result = await updateClaudeHudMutation.mutateAsync();
      await refetchHudStatus();
      setHudUpdateInfo(null);
      if (result.skipped) {
        showToast("success", uiText("已是最新版本", "Already up to date", "すでに最新です"));
      } else {
        showToast(
          "success",
          uiText(`已更新到 v${result.version}`, `Updated to v${result.version}`, `v${result.version} に更新しました`),
        );
      }
    } catch (e) {
      showToast("error", uiText(`更新失败: ${e}`, `Update failed: ${e}`, `更新に失敗しました: ${e}`));
    } finally {
      setHudUpdating(false);
    }
  }, [refetchHudStatus, uiText, updateClaudeHudMutation]);

  const updateHello2ccDraft = useCallback(<K extends keyof Hello2ccConfig>(key: K, value: Hello2ccConfig[K]) => {
    setHello2ccDraft((prev) => ({ ...prev, [key]: value }));
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

  const toggleHello2ccEnabled = useCallback(
    async (enabled: boolean) => {
      setHello2ccToggling(true);
      try {
        await setHello2ccEnabledMutation.mutateAsync({ enabled });
        await refetchHello2ccStatus();
        showToast("success", uiText("已更新", "Updated", "更新しました"));
      } catch (e) {
        showToast("error", `${e}`);
      } finally {
        setHello2ccToggling(false);
      }
    },
    [refetchHello2ccStatus, setHello2ccEnabledMutation, uiText],
  );

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
      const status = await updateHello2ccMutation.mutateAsync();
      setHello2ccUpdateInfo(null);
      await refetchHello2ccStatus();
      showToast(
        "success",
        uiText(`已更新到 v${status.version}`, `Updated to v${status.version}`, `v${status.version} に更新しました`),
      );
    } catch (e) {
      showToast("error", uiText(`更新失败: ${e}`, `Update failed: ${e}`, `更新に失敗しました: ${e}`));
    } finally {
      setHello2ccUpdating(false);
    }
  }, [refetchHello2ccStatus, uiText, updateHello2ccMutation]);

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

  const toggleStatusLine = useCallback(
    async (enabled: boolean) => {
      try {
        await setClaudeStatuslineMutation.mutateAsync({ enabled });
        await refetchHudStatus();
        showToast("success", uiText("已更新", "Updated", "更新しました"));
      } catch (e) {
        showToast("error", `${e}`);
      }
    },
    [refetchHudStatus, setClaudeStatuslineMutation, uiText],
  );

  const updateHudConfig = useCallback(
    async (patch: Partial<HudConfig>) => {
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
      } catch (e) {
        showToast("error", `${e}`);
      }
    },
    [hudStatus, setHudConfigMutation, uiText],
  );

  const commitPermLevel = useCallback(
    async (nextLevel = pendingPermLevelRef.current ?? permLevel) => {
      pendingPermLevelRef.current = null;
      await setClaudeSetting<number>("set_claude_permissions_level", { level: nextLevel }, setPermLevel, (v) =>
        patchToolsPageCache({ permissionsLevel: v }),
      );
    },
    [patchToolsPageCache, permLevel, setClaudeSetting],
  );

  const {
    unavailableLabel,
    autoUpdateOptions,
    claudeModelOptions,
    hudLayoutOptions,
    hudPathLevelOptions,
    hudContextValueOptions,
    codexApprovalOptions,
    codexReasoningOptions,
    permLevelOptions,
    hudGitStatusOptions,
    hudDisplayOptions,
    hello2ccRoutingOptions,
    hello2ccCompatibilityOptions,
    hello2ccModelOptions,
    hello2ccModelFields,
    noVisibleTabsTitle,
    noVisibleTabsDescription,
    notInstalledTitle,
    notInstalledDescription,
  } = useToolsOptions(uiText, tab);
  const handleSelectTab = useCallback((value: string) => {
    setTab(value as ToolTab);
  }, []);
  const handleSelectPermLevel = useCallback(
    (value: string | number) => {
      const nextLevel = Number(value);
      setPermLevel(nextLevel);
      pendingPermLevelRef.current = nextLevel;
      void commitPermLevel(nextLevel);
    },
    [commitPermLevel],
  );
  const handleSelectAutoUpdate = useCallback(
    (value: string | number) => {
      const nextValue = String(value);
      setAutoUpdate(nextValue);
      void setClaudeSetting<string>("set_claude_auto_update", { channel: nextValue }, setAutoUpdate, (v) =>
        patchToolsPageCache({ autoUpdateChannel: v }),
      );
    },
    [patchToolsPageCache, setClaudeSetting],
  );
  const handleSelectClaudeModel = useCallback(
    (value: string | number) => {
      const nextValue = String(value);
      setClaudeModel(nextValue);
      void setClaudeSetting<string>("set_claude_model", { model: nextValue }, setClaudeModel, (v) =>
        patchToolsPageCache({ claudeModel: v }),
      );
    },
    [patchToolsPageCache, setClaudeSetting],
  );
  const handleSelectHudLayout = useCallback(
    (value: string | number) => {
      void updateHudConfig({ lineLayout: value as HudConfig["lineLayout"] });
    },
    [updateHudConfig],
  );
  const handleSelectHudPathLevel = useCallback(
    (value: string | number) => {
      void updateHudConfig({ pathLevels: Number(value) });
    },
    [updateHudConfig],
  );
  const handleSelectHudContextValue = useCallback(
    (value: string | number) => {
      void updateHudConfig({ display: { contextValue: value as NonNullable<HudConfig["display"]>["contextValue"] } });
    },
    [updateHudConfig],
  );
  const handleToggleHudGitStatus = useCallback(
    (key: string, checked: boolean) => {
      void updateHudConfig({ gitStatus: { [key]: checked } as Partial<NonNullable<HudConfig["gitStatus"]>> });
    },
    [updateHudConfig],
  );
  const handleToggleHudDisplay = useCallback(
    (key: string, checked: boolean) => {
      void updateHudConfig({ display: { [key]: checked } as Partial<NonNullable<HudConfig["display"]>> });
    },
    [updateHudConfig],
  );
  const handleSelectCodexApproval = useCallback(
    (value: string | number) => {
      const nextValue = String(value);
      setCodexApproval(nextValue);
      void setCodex("approval_mode", nextValue);
    },
    [setCodex],
  );
  const handleSelectCodexReasoning = useCallback(
    (value: string | number) => {
      const nextValue = String(value);
      setCodexReasoning(nextValue);
      void setCodex("reasoning_effort", nextValue);
    },
    [setCodex],
  );
  const handleToggleBypassPermissions = useCallback(
    (enabled: boolean) => {
      const nextLevel = enabled ? 3 : 0;
      setPermLevel(nextLevel);
      pendingPermLevelRef.current = nextLevel;
      void commitPermLevel(nextLevel);
    },
    [commitPermLevel],
  );
  const handleToggleToolSearch = useCallback(
    (enabled: boolean) => {
      setToolSearch(enabled);
      void setClaudeSetting<boolean>("set_claude_tool_search", { enabled }, setToolSearch, (v) =>
        patchToolsPageCache({ toolSearchEnabled: v }),
      );
    },
    [patchToolsPageCache, setClaudeSetting],
  );
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
  const handleCommitPermLevelKeyUp = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key.startsWith("Arrow") || event.key === "Home" || event.key === "End") {
        void commitPermLevel();
      }
    },
    [commitPermLevel],
  );
  const handleInstallHudClick = useCallback(() => {
    void handleInstallHud();
  }, [handleInstallHud]);
  const handleUpdateHudClick = useCallback(() => {
    void handleUpdateHud();
  }, [handleUpdateHud]);
  const handleCheckHudUpdateClick = useCallback(() => {
    void checkHudUpdate();
  }, [checkHudUpdate]);
  const handleToggleHudSeparators = useCallback(
    (checked: boolean) => {
      void updateHudConfig({ showSeparators: checked });
    },
    [updateHudConfig],
  );
  const handleChangeHello2ccSelect = useCallback(
    (fieldKey: string, value: string) => {
      updateHello2ccDraft(fieldKey as Hello2ccSelectKey, value as Hello2ccConfig[Hello2ccSelectKey]);
    },
    [updateHello2ccDraft],
  );
  const handleToggleHello2ccMirrorSessionModel = useCallback(
    (enabled: boolean) => {
      updateHello2ccDraft("mirror_session_model", enabled);
    },
    [updateHello2ccDraft],
  );
  const handleToggleCodexDisableStorage = useCallback(
    (enabled: boolean) => {
      setCodexDisableStorage(enabled);
      void setCodex("disable_response_storage", String(enabled));
    },
    [setCodex],
  );
  const handleToggleCodexContextWindow1M = useCallback(
    (enabled: boolean) => {
      setCodexContextWindow1M(enabled);
      void setCodex("context_window_1m", String(enabled));
    },
    [setCodex],
  );
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
    () =>
      hudGitStatusOptions.map((option) => ({
        key: option.key,
        label: option.label,
        checked: hc.gitStatus?.[option.key] ?? option.defaultValue,
      })),
    [hc.gitStatus, hudGitStatusOptions],
  );
  const hudResolvedDisplayOptions = useMemo(
    () =>
      hudDisplayOptions.map((option) => ({
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
    () =>
      hudUpdateInfo?.hasUpdate
        ? {
            label: uiText(
              `更新到 v${hudUpdateInfo.latestVersion}`,
              `Update to v${hudUpdateInfo.latestVersion}`,
              `v${hudUpdateInfo.latestVersion} に更新`,
            ),
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
          },
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
    () =>
      hello2ccUpdateInfo?.hasUpdate
        ? {
            label: uiText(
              `更新到 v${hello2ccUpdateInfo.latestVersion}`,
              `Update to v${hello2ccUpdateInfo.latestVersion}`,
              `v${hello2ccUpdateInfo.latestVersion} に更新`,
            ),
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
          },
    [
      handleCheckHello2ccUpdateClick,
      handleUpdateHello2ccClick,
      hello2ccChecking,
      hello2ccUpdateInfo,
      hello2ccUpdating,
      uiText,
    ],
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
    () =>
      buildHello2ccSelectFields({
        uiText,
        hello2ccDraft,
        hello2ccRoutingOptions,
        hello2ccCompatibilityOptions,
        hello2ccModelFields,
        hello2ccModelOptions,
      }),
    [
      hello2ccCompatibilityOptions,
      hello2ccDraft,
      hello2ccModelFields,
      hello2ccModelOptions,
      hello2ccRoutingOptions,
      uiText,
    ],
  );

  if (loading) {
    return <LoadingState label={uiText("加载中...", "Loading...", "読み込み中...")} />;
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{uiText("工具", "Tools", "ツール")}</h2>
          <p className="page-subtitle">
            {uiText(
              "管理 AI 编程工具的配置和权限",
              "Manage AI coding tool settings",
              "AI コーディングツールの設定と権限を管理",
            )}
          </p>
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
          <ToolsEmptyStateCard title={noVisibleTabsTitle} description={noVisibleTabsDescription} marginBottom={12} />
        )}

        {/* Not installed hint */}
        {visibleTabs.length > 0 && !activeTabInstalled && (
          <ToolsEmptyStateCard title={notInstalledTitle} description={notInstalledDescription} />
        )}

        {tab === "claude" && toolById.get("claude")?.installed && (
          <ClaudeTab
            uiText={uiText}
            perm={perm}
            permLevel={permLevel}
            permDescription={permDescription}
            permLevelOptions={permLevelOptions}
            handleSelectPermLevel={handleSelectPermLevel}
            handleChangePermLevelRange={handleChangePermLevelRange}
            handleCommitPermLevelPointerUp={handleCommitPermLevelPointerUp}
            handleCommitPermLevelKeyUp={handleCommitPermLevelKeyUp}
            handleCommitPermLevelBlur={handleCommitPermLevelBlur}
            handleToggleBypassPermissions={handleToggleBypassPermissions}
            autoUpdate={autoUpdate}
            autoUpdateOptions={autoUpdateOptions}
            handleSelectAutoUpdate={handleSelectAutoUpdate}
            claudeModel={claudeModel}
            claudeModelOptions={claudeModelOptions}
            handleSelectClaudeModel={handleSelectClaudeModel}
            toolSearch={toolSearch}
            handleToggleToolSearch={handleToggleToolSearch}
            hudStatus={hudStatus}
            hudInstallAction={hudInstallAction}
            hudPrimaryAction={hudPrimaryAction}
            hudToggle={hudToggle}
            hc={hc}
            hudLayoutOptions={hudLayoutOptions}
            handleSelectHudLayout={handleSelectHudLayout}
            handleToggleHudSeparators={handleToggleHudSeparators}
            hudPathLevelOptions={hudPathLevelOptions}
            handleSelectHudPathLevel={handleSelectHudPathLevel}
            hudContextValueOptions={hudContextValueOptions}
            handleSelectHudContextValue={handleSelectHudContextValue}
            hudResolvedGitStatusOptions={hudResolvedGitStatusOptions}
            handleToggleHudGitStatus={handleToggleHudGitStatus}
            hudResolvedDisplayOptions={hudResolvedDisplayOptions}
            handleToggleHudDisplay={handleToggleHudDisplay}
            hello2ccStatus={hello2ccStatus}
            hello2ccInstallAction={hello2ccInstallAction}
            hello2ccPrimaryAction={hello2ccPrimaryAction}
            hello2ccSecondaryAction={hello2ccSecondaryAction}
            hello2ccToggle={hello2ccToggle}
            hello2ccSelectFields={hello2ccSelectFields}
            handleChangeHello2ccSelect={handleChangeHello2ccSelect}
            hello2ccDraft={hello2ccDraft}
            handleToggleHello2ccMirrorSessionModel={handleToggleHello2ccMirrorSessionModel}
            hello2ccHasChanges={hello2ccHasChanges}
            setHello2ccConfigMutation={setHello2ccConfigMutation}
            handleResetHello2ccDraft={handleResetHello2ccDraft}
            handleSaveHello2ccConfigClick={handleSaveHello2ccConfigClick}
          />
        )}

        {tab === "codex" && toolById.get("codex")?.installed && (
          <CodexTab
            uiText={uiText}
            codexApproval={codexApproval}
            codexApprovalOptions={codexApprovalOptions}
            handleSelectCodexApproval={handleSelectCodexApproval}
            codexReasoning={codexReasoning}
            codexReasoningOptions={codexReasoningOptions}
            handleSelectCodexReasoning={handleSelectCodexReasoning}
            codexDisableStorage={codexDisableStorage}
            handleToggleCodexDisableStorage={handleToggleCodexDisableStorage}
            codexContextWindow1M={codexContextWindow1M}
            handleToggleCodexContextWindow1M={handleToggleCodexContextWindow1M}
          />
        )}
      </div>
    </div>
  );
}
