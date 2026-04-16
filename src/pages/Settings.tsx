import { useCallback, useEffect, useMemo, useState, type ChangeEvent, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t, getLocale, setLocale, type Locale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import SettingsAboutSection from "../components/SettingsAboutSection";
import SettingsAppVisibilitySection from "../components/SettingsAppVisibilitySection";
import SettingsAuthCenterSection from "../components/SettingsAuthCenterSection";
import WebDavSyncSection from "../components/WebDavSyncSection";
import SettingsAppearanceSection from "../components/SettingsAppearanceSection";
import SettingsBackupManagementSection from "../components/SettingsBackupManagementSection";
import SettingsAppUpdateSection from "../components/SettingsAppUpdateSection";
import SettingsEnvironmentConflictSection from "../components/SettingsEnvironmentConflictSection";
import SettingsGeneralSection from "../components/SettingsGeneralSection";
import SettingsKeyboardShortcutsSection from "../components/SettingsKeyboardShortcutsSection";
import SettingsLocalProviderProxySection from "../components/SettingsLocalProviderProxySection";
import SettingsLoggingSection from "../components/SettingsLoggingSection";
import SettingsMigrationCenterSection from "../components/SettingsMigrationCenterSection";
import SettingsNetworkProxySection from "../components/SettingsNetworkProxySection";
import SettingsPreferredTerminalSection from "../components/SettingsPreferredTerminalSection";
import SettingsToolPathSection from "../components/SettingsToolPathSection";
import SettingsWindowBehaviorSection from "../components/SettingsWindowBehaviorSection";
import { useSettingsMigrationState } from "../hooks/useSettingsMigrationState";
import { getTheme, setTheme, type Theme } from "../lib/theme";
import {
  checkAppUpdate,
  getUpdaterEnvironmentState,
  installAppUpdate,
  type AppUpdateHandle,
  type AppUpdateResult,
  type UpdaterEnvironmentState,
} from "../lib/appUpdater";
import { getVersion } from "@tauri-apps/api/app";
import {
  fetchVisibleApps,
  MANAGED_APPS,
  type LocalProviderProxySettings,
  type LocalProviderProxyStatus,
  normalizeVisibleApps,
  type EnvironmentConflict,
  type LogFileTargets,
  type LogPreferences,
  type ManagedAppId,
  type TerminalPreferences,
  type WindowPreferences,
} from "../lib/appPreferences";
import { type CustomPath, type DetectedTool } from "../types/settings";

export default function Settings() {
  const [locale, setLoc] = useState<Locale>(getLocale());
  const [theme, setThm] = useState<Theme>(getTheme());
  const [autoScan, setAutoScan] = useState(true);
  const [checkUpdates, setCheckUpdates] = useState(true);
  const [appUpdate, setAppUpdate] = useState<AppUpdateResult | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [updateHandle, setUpdateHandle] = useState<AppUpdateHandle | null>(null);
  const [updaterEnvironmentState, setUpdaterEnvironmentState] = useState<UpdaterEnvironmentState | null>(null);
  const [appVersion, setAppVersion] = useState("");
  const [tools, setTools] = useState<DetectedTool[]>([]);
  const [customPaths, setCustomPaths] = useState<CustomPath[]>([]);
  const [pathSaved, setPathSaved] = useState<string | null>(null);
  const [proxyUrl, setProxyUrl] = useState("");
  const [proxySaved, setProxySaved] = useState(false);
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>([...MANAGED_APPS]);
  const [localProviderProxySettings, setLocalProviderProxySettingsState] = useState<LocalProviderProxySettings>({
    port: 34567,
    enabled_apps: [],
  });
  const [localProviderProxyStatus, setLocalProviderProxyStatus] = useState<LocalProviderProxyStatus | null>(null);
  const [windowPreferences, setWindowPreferencesState] = useState<WindowPreferences>({
    launch_at_login: false,
    launch_hidden: false,
    close_to_tray: true,
    lightweight_mode: false,
  });
  const [terminalPreferences, setTerminalPreferences] = useState<TerminalPreferences | null>(null);
  const [environmentConflicts, setEnvironmentConflicts] = useState<EnvironmentConflict[]>([]);
  const [logPreferences, setLogPreferencesState] = useState<LogPreferences>({ level: "error" });
  const [logFileTargets, setLogFileTargets] = useState<LogFileTargets | null>(null);
  const [savingWindowKey, setSavingWindowKey] = useState<keyof WindowPreferences | null>(null);
  const [savingVisibleApps, setSavingVisibleApps] = useState(false);
  const [savingLocalProviderProxy, setSavingLocalProviderProxy] = useState(false);
  const [savingTerminal, setSavingTerminal] = useState(false);
  const [refreshingEnvConflicts, setRefreshingEnvConflicts] = useState(false);
  const [savingLogPreferences, setSavingLogPreferences] = useState(false);
  const [skillSyncMethod, setSkillSyncMethod] = useState<"symlink" | "copy">("copy");
  const i = t();
  const loc = getLocale();
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    loc === "zh" ? zhText : loc === "ja" ? (jaText ?? enText) : enText
  );

  useEffect(() => {
    loadToolsAndPaths();
    loadProxy();
    loadSkillSyncMethod();
    loadAdvancedPreferences();
    getVersion().then(v => setAppVersion("v" + v)).catch(() => {});
  }, []);

  async function loadProxy() {
    try {
      const proxy = await invoke<string>("get_proxy");
      setProxyUrl(proxy);
    } catch { /* ignore */ }
  }

  async function loadSkillSyncMethod() {
    try {
      const method = await invoke<string>("get_skill_sync_method");
      if (method === "symlink" || method === "copy") setSkillSyncMethod(method);
    } catch { /* ignore */ }
  }

  async function loadAdvancedPreferences() {
    try {
      const [
        nextVisibleApps,
        nextLocalProviderProxySettings,
        nextLocalProviderProxyStatus,
        nextWindowPreferences,
        nextTerminalPreferences,
        nextEnvironmentConflicts,
        nextLogPreferences,
        nextLogFileTargets,
        nextUpdaterEnvironmentState,
      ] = await Promise.all([
        fetchVisibleApps(),
        invoke<LocalProviderProxySettings>("get_local_provider_proxy_settings"),
        invoke<LocalProviderProxyStatus>("get_local_provider_proxy_status"),
        invoke<WindowPreferences>("get_window_preferences"),
        invoke<TerminalPreferences>("get_terminal_preferences"),
        invoke<EnvironmentConflict[]>("get_environment_conflicts"),
        invoke<LogPreferences>("get_log_preferences"),
        invoke<LogFileTargets>("get_log_file_targets"),
        getUpdaterEnvironmentState(),
      ]);
      setVisibleApps(nextVisibleApps);
      setLocalProviderProxySettingsState(nextLocalProviderProxySettings);
      setLocalProviderProxyStatus(nextLocalProviderProxyStatus);
      setWindowPreferencesState(nextWindowPreferences);
      setTerminalPreferences(nextTerminalPreferences);
      setEnvironmentConflicts(nextEnvironmentConflicts);
      setLogPreferencesState(nextLogPreferences);
      setLogFileTargets(nextLogFileTargets);
      setUpdaterEnvironmentState(nextUpdaterEnvironmentState);
    } catch (e) {
      console.error(e);
    }
  }

  const handleSaveLocalProviderProxySettings = useCallback(async (next: LocalProviderProxySettings) => {
    setSavingLocalProviderProxy(true);
    try {
      const saved = await invoke<LocalProviderProxyStatus>("set_local_provider_proxy_settings", {
        settings: next,
      });
      setLocalProviderProxySettingsState({
        port: saved.port,
        enabled_apps: saved.enabled_apps,
      });
      setLocalProviderProxyStatus(saved);
      showToast(
        "success",
        uiText("本地 Provider 代理设置已保存", "Local provider proxy settings saved", "ローカル Provider プロキシ設定を保存しました"),
      );
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setSavingLocalProviderProxy(false);
    }
  }, [loc]);

  const refreshEnvironmentConflicts = useCallback(async () => {
    setRefreshingEnvConflicts(true);
    try {
      const nextVisibleApps = normalizeVisibleApps(await invoke<string[]>("get_visible_apps"));
      const conflicts = await invoke<EnvironmentConflict[]>("get_environment_conflicts");
      setVisibleApps(nextVisibleApps);
      setEnvironmentConflicts(conflicts);
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setRefreshingEnvConflicts(false);
    }
  }, []);

  const updateWindowPreference = useCallback(async <K extends keyof WindowPreferences>(key: K, value: WindowPreferences[K]) => {
    const nextPreferences = { ...windowPreferences, [key]: value };
    setSavingWindowKey(key);
    try {
      const saved = await invoke<WindowPreferences>("set_window_preferences", { preferences: nextPreferences });
      setWindowPreferencesState(saved);
      showToast("success", loc === "zh" ? "窗口设置已保存" : "Window preferences saved");
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setSavingWindowKey((current) => current === key ? null : current);
    }
  }, [loc, windowPreferences]);

  const handleToggleVisibleApp = useCallback(async (appId: ManagedAppId) => {
    const nextVisibleApps = visibleApps.includes(appId)
      ? visibleApps.filter((item) => item !== appId)
      : [...visibleApps, appId];

    if (nextVisibleApps.length === 0) {
      showToast("error", loc === "zh" ? "至少保留一个可见 App" : "Keep at least one visible app");
      return;
    }

    setSavingVisibleApps(true);
    try {
      const saved = normalizeVisibleApps(await invoke<string[]>("set_visible_apps", { visibleApps: nextVisibleApps }));
      setVisibleApps(saved);
      showToast("success", loc === "zh" ? "App 可见性已保存" : "App visibility saved");
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setSavingVisibleApps(false);
    }
  }, [loc, visibleApps]);

  const handleSelectTerminal = useCallback(async (terminalId: string) => {
    setSavingTerminal(true);
    try {
      await invoke("set_preferred_terminal", { terminalId });
      const nextPreferences = await invoke<TerminalPreferences>("get_terminal_preferences");
      setTerminalPreferences(nextPreferences);
      showToast("success", loc === "zh" ? "终端偏好已保存" : "Terminal preference saved");
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setSavingTerminal(false);
    }
  }, [loc]);

  const handleSaveLogLevel = useCallback(async (level: string) => {
    setSavingLogPreferences(true);
    try {
      const saved = await invoke<LogPreferences>("set_log_preferences", {
        preferences: { level },
      });
      setLogPreferencesState(saved);
      showToast("success", uiText("日志级别已保存", "Log level saved", "ログレベルを保存しました"));
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setSavingLogPreferences(false);
    }
  }, [loc]);

  const handleSyncMethodChange = useCallback(async (method: "symlink" | "copy") => {
    try {
      await invoke("set_skill_sync_method", { method });
      setSkillSyncMethod(method);
      showToast("success", loc === "zh" ? "已保存" : "Saved");
    } catch (e) { showToast("error", String(e)); }
  }, [loc]);

  const loadToolsAndPaths = useCallback(async () => {
    try {
      const [t, p] = await Promise.all([
        invoke<DetectedTool[]>("detect_tools"),
        invoke<CustomPath[]>("get_custom_paths"),
      ]);
      setTools(t);
      setCustomPaths(p);
    } catch (e) { console.error(e); }
  }, []);

  const openInSystemWithLabel = useCallback(async (target: string, label: string) => {
    try {
      await invoke("open_in_system", { target });
    } catch (e) {
      showToast(
        "error",
        `${i.settings.openFailed.replace("{label}", label)}: ${String(e)}`
      );
    }
  }, [i.settings.openFailed]);

  const copyText = useCallback(async (value: string, label: string) => {
    try {
      await navigator.clipboard.writeText(value);
      showToast("success", i.settings.copied.replace("{label}", label));
    } catch (e) {
      showToast("error", String(e));
    }
  }, [i.settings.copied]);

  const migrationStateText = useMemo(() => ({
    migrationHealthBootstrapSuccess: i.settings.migrationHealthBootstrapSuccess,
    fullRescanSuccess: i.settings.fullRescanSuccess,
    pendingImportsRepairAllSuccess: i.settings.pendingImportsRepairAllSuccess,
    pendingImportsAutoMatchSuccess: i.settings.pendingImportsAutoMatchSuccess,
    migrationHealthRefreshSuccess: i.settings.migrationHealthRefreshSuccess,
    pendingImportsSuccess: i.settings.pendingImportsSuccess,
  }), [
    i.settings.fullRescanSuccess,
    i.settings.migrationHealthBootstrapSuccess,
    i.settings.migrationHealthRefreshSuccess,
    i.settings.pendingImportsAutoMatchSuccess,
    i.settings.pendingImportsRepairAllSuccess,
    i.settings.pendingImportsSuccess,
  ]);

  const {
    pendingProjectRoots,
    toolReports,
    lastImportSummary,
    lastRescan,
    remapTargets,
    remappingRoot,
    autoMatchingPending,
    bootstrappingToolId,
    repairingAll,
    rescanningAll,
    refreshingMigrationHealth,
    exportingBackup,
    importingBackup,
    managedBackups,
    loadingManagedBackups,
    backupPreferences,
    savingBackupPreferences,
    creatingManagedBackup,
    restoringBackupPath,
    deletingBackupPath,
    migrationPanelsOpen,
    migrationPanelRefs,
    handleFocusMigrationPanel,
    handleSummaryPanelToggle,
    handlePendingPanelToggle,
    handleHealthPanelToggle,
    handleAuthPanelToggle,
    handlePendingTargetChange,
    handlePickPendingTarget,
    handleApplyPendingTarget,
    handleExportBackup,
    handleImportBackup,
    handleRepairAll,
    handleFullRescan,
    handleOpenSafetyBackupPath,
    handleAutoMatchPending,
    handleRefreshMigrationHealth,
    runBootstrapForTool,
    handleToggleAutoBackup,
    handleBackupRetentionChange,
    handleSaveCurrentBackupPreferences,
    handleCreateManagedBackup,
    handleRefreshManagedBackups,
    handleRenameManagedBackup,
    handleRestoreManagedBackup,
    handleDeleteManagedBackup,
  } = useSettingsMigrationState({
    locale: loc,
    settingsText: migrationStateText,
    toolsLength: tools.length,
    loadToolsAndPaths,
    setTools,
    setCustomPaths,
    openInSystemWithLabel,
  });

  const handleOpenHomeInTerminal = useCallback(async () => {
    try {
      await invoke("open_in_preferred_terminal", { path: null });
    } catch (e) {
      showToast("error", String(e));
    }
  }, []);

  const handleLocalProviderProxyPortChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    const nextPort = Math.max(1024, Math.min(65535, Number(event.target.value) || 34567));
    setLocalProviderProxySettingsState((current) => ({ ...current, port: nextPort }));
  }, []);

  const handleSaveCurrentLocalProviderProxySettings = useCallback(() => {
    void handleSaveLocalProviderProxySettings(localProviderProxySettings);
  }, [handleSaveLocalProviderProxySettings, localProviderProxySettings]);

  const handleToggleLocalProviderProxyApp = useCallback((appId: ManagedAppId) => {
    setLocalProviderProxySettingsState((current) => {
      const enabled = current.enabled_apps.includes(appId)
        ? current.enabled_apps.filter((item) => item !== appId)
        : [...current.enabled_apps, appId];
      return { ...current, enabled_apps: enabled };
    });
  }, []);

  const markToolPathSaved = useCallback((toolId: string) => {
    setPathSaved(toolId);
    window.setTimeout(() => {
      setPathSaved((current) => current === toolId ? null : current);
    }, 2000);
  }, []);

  const saveToolCustomPath = useCallback(async (
    toolId: string,
    configDir: string | null,
    mcpConfigPath: string | null,
    skillsDir: string | null,
  ) => {
    await invoke("save_custom_path", { toolId, configDir, mcpConfigPath, skillsDir });
    markToolPathSaved(toolId);
    await loadToolsAndPaths();
  }, [loadToolsAndPaths, markToolPathSaved]);

  const handleToolMcpPathBlur = useCallback(async (
    toolId: string,
    value: string,
    defaultValue: string,
    customPath?: CustomPath,
  ) => {
    const nextValue = value.trim();
    if (!nextValue || nextValue === defaultValue) return;
    await saveToolCustomPath(
      toolId,
      customPath?.config_dir || null,
      nextValue,
      customPath?.skills_dir || null,
    );
  }, [saveToolCustomPath]);

  const handlePickToolMcpPath = useCallback(async (toolId: string, customPath?: CustomPath) => {
    const picked = await invoke<string | null>("pick_file");
    if (!picked) return;
    await saveToolCustomPath(
      toolId,
      customPath?.config_dir || null,
      picked,
      customPath?.skills_dir || null,
    );
  }, [saveToolCustomPath]);

  const handleToolSkillsDirBlur = useCallback(async (
    toolId: string,
    value: string,
    defaultValue: string,
    customPath?: CustomPath,
  ) => {
    const nextValue = value.trim();
    if (!nextValue || nextValue === defaultValue) return;
    await saveToolCustomPath(
      toolId,
      customPath?.config_dir || null,
      customPath?.mcp_config_path || null,
      nextValue,
    );
  }, [saveToolCustomPath]);

  const handlePickToolSkillsDir = useCallback(async (toolId: string, customPath?: CustomPath) => {
    const picked = await invoke<string | null>("pick_folder");
    if (!picked) return;
    await saveToolCustomPath(
      toolId,
      customPath?.config_dir || null,
      customPath?.mcp_config_path || null,
      picked,
    );
  }, [saveToolCustomPath]);

  const handleCopyToolInstallCommand = useCallback((command: string, toolName: string) => {
    void copyText(command, loc === "zh" ? `${toolName} 安装命令` : `${toolName} install command`);
  }, [copyText, loc]);

  const markProxySaved = useCallback(() => {
    setProxySaved(true);
    window.setTimeout(() => setProxySaved(false), 2000);
  }, []);

  const persistProxy = useCallback(async (successMessage: string) => {
    try {
      await invoke("set_proxy", { proxyUrl });
      markProxySaved();
      showToast("success", successMessage);
    } catch (e) {
      showToast("error", String(e));
    }
  }, [markProxySaved, proxyUrl]);

  const handleProxyInputChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setProxyUrl(event.target.value);
  }, []);

  const handleProxyInputKeyDown = useCallback((event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "Enter") return;
    void persistProxy(loc === "zh" ? "代理已保存" : "Proxy saved");
  }, [loc, persistProxy]);

  const handleSaveProxy = useCallback(() => {
    void persistProxy(
      loc === "zh"
        ? (proxyUrl.trim() ? "代理已设置" : "代理已清除")
        : (proxyUrl.trim() ? "Proxy set" : "Proxy cleared")
    );
  }, [loc, persistProxy, proxyUrl]);

  const handleLocaleChange = useCallback((newLocale: Locale) => {
    setLocale(newLocale);
    setLoc(newLocale);
    window.location.reload();
  }, []);

  const handleThemeChange = useCallback((newTheme: Theme) => {
    setTheme(newTheme);
    setThm(newTheme);
  }, []);

  const handleToggleAutoScan = useCallback(() => {
    setAutoScan((current) => !current);
  }, []);

  const handleToggleCheckUpdates = useCallback(() => {
    setCheckUpdates((current) => !current);
  }, []);

  const handleToggleLaunchAtLogin = useCallback(() => {
    void updateWindowPreference("launch_at_login", !windowPreferences.launch_at_login);
  }, [windowPreferences.launch_at_login]);

  const handleToggleLaunchHidden = useCallback(() => {
    void updateWindowPreference("launch_hidden", !windowPreferences.launch_hidden);
  }, [windowPreferences.launch_hidden]);

  const handleToggleCloseToTray = useCallback(() => {
    void updateWindowPreference("close_to_tray", !windowPreferences.close_to_tray);
  }, [windowPreferences.close_to_tray]);

  const handleToggleLightweightMode = useCallback(() => {
    void updateWindowPreference("lightweight_mode", !windowPreferences.lightweight_mode);
  }, [windowPreferences.lightweight_mode]);

  const handleCheckUpdate = useCallback(async () => {
    if (updaterEnvironmentState?.disabled_by_env) {
      let currentVersion: string | null = null;
      try {
        currentVersion = await getVersion();
      } catch {
        currentVersion = appVersion ? appVersion.replace(/^v/i, "") : null;
      }
      setAppUpdate({
        update_available: false,
        latest_version: null,
        current_version: currentVersion,
        body: null,
        not_configured: false,
        can_install: false,
        release_url: null,
        source: null,
        disabled_by_env: true,
      });
      setUpdateHandle(null);
      setUpdateError(null);
      return;
    }

    setCheckingUpdate(true);
    setUpdateError(null);
    try {
      const { result, handle } = await checkAppUpdate();
      setAppUpdate(result);
      setUpdateHandle(handle);
    } catch (e) {
      setUpdateError(String(e));
      setUpdateHandle(null);
    } finally {
      setCheckingUpdate(false);
    }
  }, [appVersion, updaterEnvironmentState?.disabled_by_env]);

  const handleInstallUpdate = useCallback(async () => {
    if (!updateHandle) return;
    setInstalling(true);
    setUpdateError(null);
    try {
      await installAppUpdate(updateHandle);
    } catch (e) {
      setUpdateError(String(e));
    } finally {
      setInstalling(false);
    }
  }, [updateHandle]);

  const visibleToolIds = new Set(visibleApps);
  const visibleTools = tools.filter((tool) => visibleToolIds.has(tool.id as ManagedAppId));
  const updaterDisabledByEnv = (updaterEnvironmentState?.disabled_by_env ?? false) || (appUpdate?.disabled_by_env ?? false);
  const updaterEnvValue = updaterEnvironmentState?.env_var_value ?? null;

  const migrationCenterLabels = useMemo(() => ({
    title: i.settings.migrationCenter,
    description: i.settings.migrationCenterDesc,
    readyLabel: i.settings.migrationCenterReady,
    activeLabel: loc === "zh" ? "当前展开" : "Open",
    viewLabel: loc === "zh" ? "查看" : "View",
    importSummary: i.settings.importSummary,
    importSummaryPending: i.settings.importSummaryPending,
    pendingImports: i.settings.pendingImports,
    migrationHealth: i.settings.migrationHealth,
    authGuide: i.settings.authGuide,
    migrationExport: i.settings.migrationExport,
    migrationExporting: i.settings.migrationExporting,
    migrationImport: i.settings.migrationImport,
    migrationImporting: i.settings.migrationImporting,
    pendingImportsRepairAll: i.settings.pendingImportsRepairAll,
    pendingImportsRepairingAll: i.settings.pendingImportsRepairingAll,
    fullRescan: i.settings.fullRescan,
    fullRescanning: i.settings.fullRescanning,
    importSummaryEmpty: i.settings.importSummaryEmpty,
    importSummaryBackup: i.settings.importSummaryBackup,
    openPathLabel: i.settings.authGuideOpenPath,
    migrationCenterLastRescanEmpty: i.settings.migrationCenterLastRescanEmpty,
    importSummaryImportedAt: i.settings.importSummaryImportedAt,
    importSummaryData: i.settings.importSummaryData,
    importSummaryToolConfigs: i.settings.importSummaryToolConfigs,
    importSummarySkills: i.settings.importSummarySkills,
    importSummaryFiles: i.settings.importSummaryFiles,
    rescanMcp: i.settings.rescanMcp,
    rescanSkills: i.settings.rescanSkills,
    rescanHooks: i.settings.rescanHooks,
    rescanDocs: i.settings.rescanDocs,
    rescanWorkflows: i.settings.rescanWorkflows,
    rescanConfigRoots: i.settings.rescanConfigRoots,
    pendingImportsDesc: i.settings.pendingImportsDesc,
    pendingImportsAutoMatchDesc: i.settings.pendingImportsAutoMatchDesc,
    pendingImportsAutoMatching: i.settings.pendingImportsAutoMatching,
    pendingImportsAutoMatch: i.settings.pendingImportsAutoMatch,
    pendingImportsEmpty: i.settings.pendingImportsEmpty,
    pendingImportsOldPath: i.settings.pendingImportsOldPath,
    pendingImportsNewPath: i.settings.pendingImportsNewPath,
    pendingImportsPick: i.settings.pendingImportsPick,
    pendingImportsApply: i.settings.pendingImportsApply,
    pendingImportsApplying: i.settings.pendingImportsApplying,
    pendingImportsFiles: i.settings.pendingImportsFiles,
    migrationHealthDesc: i.settings.migrationHealthDesc,
    migrationHealthRefreshing: i.settings.migrationHealthRefreshing,
    migrationHealthRefresh: i.settings.migrationHealthRefresh,
    migrationHealthReady: i.settings.migrationHealthReady,
    migrationHealthCliMissing: i.settings.migrationHealthCliMissing,
    migrationHealthConfigDirMissing: i.settings.migrationHealthConfigDirMissing,
    migrationHealthConfigMissing: i.settings.migrationHealthConfigMissing,
    migrationHealthMcpMissing: i.settings.migrationHealthMcpMissing,
    migrationHealthSkillsMissing: i.settings.migrationHealthSkillsMissing,
    migrationHealthInstall: i.settings.migrationHealthInstall,
    migrationHealthBootstrap: i.settings.migrationHealthBootstrap,
    migrationHealthBootstrapping: i.settings.migrationHealthBootstrapping,
    migrationHealthCli: i.settings.migrationHealthCli,
    migrationHealthPath: i.settings.migrationHealthPath,
    migrationHealthStatusOk: i.settings.migrationHealthStatusOk,
    migrationHealthStatusMissing: i.settings.migrationHealthStatusMissing,
    migrationHealthConfigDir: i.settings.migrationHealthConfigDir,
    migrationHealthConfigFile: i.settings.migrationHealthConfigFile,
    migrationHealthMcpConfig: i.settings.migrationHealthMcpConfig,
    migrationHealthSkillsDir: i.settings.migrationHealthSkillsDir,
    migrationHealthCustomPath: i.settings.migrationHealthCustomPath,
    authGuideDesc: i.settings.authGuideDesc,
    authGuideReady: i.settings.authGuideReady,
    authGuideCodexLogin: i.settings.authGuideCodexLogin,
    authGuideGeminiKey: i.settings.authGuideGeminiKey,
    authGuideCopyCommand: i.settings.authGuideCopyCommand,
    authGuideCopyPath: i.settings.authGuideCopyPath,
    authGuidePrepareFile: i.settings.authGuidePrepareFile,
    authGuideOpenDocs: i.settings.authGuideOpenDocs,
  }), [i.settings, loc]);

  const appUpdateLabels = useMemo(() => ({
    appUpdate: i.settings.appUpdate,
    currentVersion: i.settings.currentVersion,
    checkForUpdate: i.settings.checkForUpdate,
    checking: i.settings.checking,
    updateAvailable: i.settings.updateAvailable,
    downloading: i.settings.downloading,
    installUpdate: i.settings.installUpdate,
    updateNotConfigured: i.settings.updateNotConfigured,
    noUpdate: i.settings.noUpdate,
    updateFailed: i.settings.updateFailed,
    disabledTitle: uiText(
      "已被 DISABLE_AUTOUPDATER 环境变量禁用",
      "Disabled by the DISABLE_AUTOUPDATER environment variable",
      "DISABLE_AUTOUPDATER 環境変数により無効化されています",
    ),
    disabledDescription: uiText(
      "当前会跳过应用更新检查与在线安装。移除该环境变量后重新启动 CCHub，即可恢复正常更新流程。",
      "App update checks and in-app install are skipped while this environment variable is set. Remove it and restart CCHub to restore normal updates.",
      "この環境変数が設定されている間、アプリ更新確認とアプリ内インストールはスキップされます。削除して CCHub を再起動すると通常の更新に戻ります。",
    ),
    disabledCheckDescription: uiText(
      "更新检查已被环境变量短路",
      "Update checks are short-circuited by environment configuration",
      "環境設定により更新確認は短絡されています",
    ),
    openDownloadsLabel: loc === "zh" ? "前往下载页" : "Open Downloads",
    releaseOnlyDescription: loc === "zh"
      ? "已通过发布页检测到新版本。当前构建未提供在线安装清单，将打开下载页手动更新。"
      : "A newer release was found from GitHub. This build does not have an online install manifest, so the download page will be opened.",
  }), [i.settings, loc]);

  return (
    <div className="animate-in">
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.settings.title}</h2>
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
        <SettingsAppearanceSection
          title={i.settings.appearance}
          themeLabel={i.settings.theme}
          languageLabel={i.settings.language}
          darkLabel={i.settings.dark}
          lightLabel={i.settings.light}
          theme={theme}
          locale={locale}
          onThemeChange={handleThemeChange}
          onLocaleChange={handleLocaleChange}
        />

        <SettingsGeneralSection
          title={i.settings.general}
          autoScanTitle={i.settings.autoScan}
          autoScanDescription={i.settings.autoScanDesc}
          autoScanEnabled={autoScan}
          checkUpdatesTitle={i.settings.checkUpdates}
          checkUpdatesDescription={i.settings.checkUpdatesDesc}
          checkUpdatesEnabled={checkUpdates}
          skillSyncTitle={i.settings.skillSyncMethod}
          skillSyncDescription={i.settings.skillSyncDesc}
          skillSyncMethod={skillSyncMethod}
          skillSyncSymlinkLabel={i.settings.skillSyncSymlink}
          skillSyncCopyLabel={i.settings.skillSyncCopy}
          skillSyncSymlinkHint={i.settings.skillSyncSymlinkHint}
          onToggleAutoScan={handleToggleAutoScan}
          onToggleCheckUpdates={handleToggleCheckUpdates}
          onSkillSyncMethodChange={handleSyncMethodChange}
        />

        <SettingsEnvironmentConflictSection
          locale={loc}
          conflicts={environmentConflicts}
          refreshing={refreshingEnvConflicts}
          checkingLabel={i.settings.checking}
          refreshLabel={i.settings.migrationHealthRefresh}
          onRefresh={refreshEnvironmentConflicts}
        />

        <SettingsWindowBehaviorSection
          title={uiText("窗口设置", "Window Behavior", "ウィンドウ動作")}
          launchAtLoginTitle={uiText("开机自启", "Launch at login", "ログイン時に起動")}
          launchAtLoginDescription={uiText(
            "写入当前平台的自启动项，随系统登录自动拉起 CCHub。",
            "Create a platform-specific autostart entry and launch CCHub when you sign in.",
            "現在のプラットフォーム向け自動起動項目を作成し、サインイン時に CCHub を起動します。",
          )}
          launchAtLoginEnabled={windowPreferences.launch_at_login}
          launchHiddenTitle={uiText("静默启动", "Launch hidden", "非表示で起動")}
          launchHiddenDescription={uiText(
            "启动后先隐藏到托盘，适合配合开机自启使用。",
            "Hide the main window immediately after launch. Useful together with autostart.",
            "起動直後にメインウィンドウをトレイへ隠します。自動起動との併用に向いています。",
          )}
          launchHiddenEnabled={windowPreferences.launch_hidden}
          closeToTrayTitle={uiText("关闭到托盘", "Close to tray", "閉じるとトレイへ")}
          closeToTrayDescription={uiText(
            "关闭主窗口时不退出进程，而是最小化留在系统托盘。",
            "Hide to the tray instead of quitting when the main window is closed.",
            "メインウィンドウを閉じたときに終了せず、システムトレイへ隠します。",
          )}
          closeToTrayEnabled={windowPreferences.close_to_tray}
          lightweightModeTitle={uiText("轻量模式", "Lightweight mode", "軽量モード")}
          lightweightModeDescription={uiText(
            "关闭主窗口时直接销毁窗口，仅保留托盘常驻；下次从托盘打开时会重建主窗口，以换取更低后台内存占用。",
            "Destroy the main window when it is closed and keep only the tray resident. Opening from the tray will recreate the window to reduce background memory usage.",
            "メインウィンドウを閉じると破棄し、トレイのみ常駐させます。トレイから再度開く際にウィンドウを再生成し、バックグラウンドのメモリ使用量を抑えます。",
          )}
          lightweightModeEnabled={windowPreferences.lightweight_mode}
          savingWindowKey={savingWindowKey}
          onToggleLaunchAtLogin={handleToggleLaunchAtLogin}
          onToggleLaunchHidden={handleToggleLaunchHidden}
          onToggleCloseToTray={handleToggleCloseToTray}
          onToggleLightweightMode={handleToggleLightweightMode}
        />

        <SettingsPreferredTerminalSection
          locale={loc}
          terminalPreferences={terminalPreferences}
          savingTerminal={savingTerminal}
          onSelectTerminal={handleSelectTerminal}
          onOpenHomeInTerminal={handleOpenHomeInTerminal}
        />

        <SettingsAppVisibilitySection
          locale={loc}
          visibleApps={visibleApps}
          savingVisibleApps={savingVisibleApps}
          onToggleVisibleApp={handleToggleVisibleApp}
        />

        <SettingsLocalProviderProxySection
          locale={loc}
          settings={localProviderProxySettings}
          status={localProviderProxyStatus}
          saving={savingLocalProviderProxy}
          onPortChange={handleLocalProviderProxyPortChange}
          onSave={handleSaveCurrentLocalProviderProxySettings}
          onToggleApp={handleToggleLocalProviderProxyApp}
          onCopyEndpoint={copyText}
        />

        <SettingsLoggingSection
          locale={loc}
          level={logPreferences.level}
          saving={savingLogPreferences}
          logFileTargets={logFileTargets}
          onSaveLogLevel={handleSaveLogLevel}
          onCopy={copyText}
          onOpen={openInSystemWithLabel}
        />

        <SettingsKeyboardShortcutsSection locale={loc} />

        <SettingsToolPathSection
          locale={loc}
          visibleTools={visibleTools}
          customPaths={customPaths}
          pathSaved={pathSaved}
          onSaveMcpPath={handleToolMcpPathBlur}
          onPickMcpPath={handlePickToolMcpPath}
          onSaveSkillsDir={handleToolSkillsDirBlur}
          onPickSkillsDir={handlePickToolSkillsDir}
          onCopyInstallCommand={handleCopyToolInstallCommand}
        />

        <SettingsAuthCenterSection locale={loc} />

        <WebDavSyncSection />

        <SettingsMigrationCenterSection
          labels={migrationCenterLabels}
          locale={loc}
          tools={tools}
          pendingProjectRoots={pendingProjectRoots}
          toolReports={toolReports}
          lastImportSummary={lastImportSummary}
          lastRescan={lastRescan}
          remapTargets={remapTargets}
          remappingRoot={remappingRoot}
          autoMatchingPending={autoMatchingPending}
          bootstrappingToolId={bootstrappingToolId}
          repairingAll={repairingAll}
          rescanningAll={rescanningAll}
          refreshingMigrationHealth={refreshingMigrationHealth}
          exportingBackup={exportingBackup}
          importingBackup={importingBackup}
          migrationPanelsOpen={migrationPanelsOpen}
          migrationPanelRefs={migrationPanelRefs}
          onSummaryToggle={handleSummaryPanelToggle}
          onPendingToggle={handlePendingPanelToggle}
          onHealthToggle={handleHealthPanelToggle}
          onAuthToggle={handleAuthPanelToggle}
          onFocusPanel={handleFocusMigrationPanel}
          onExportBackup={handleExportBackup}
          onImportBackup={handleImportBackup}
          onRepairAll={handleRepairAll}
          onFullRescan={handleFullRescan}
          onOpenBackupPath={handleOpenSafetyBackupPath}
          onAutoMatchPending={handleAutoMatchPending}
          onPendingTargetChange={handlePendingTargetChange}
          onPickPendingTarget={handlePickPendingTarget}
          onApplyPendingTarget={handleApplyPendingTarget}
          onRefreshMigrationHealth={handleRefreshMigrationHealth}
          onCopy={copyText}
          onOpen={openInSystemWithLabel}
          onBootstrapTool={runBootstrapForTool}
        />

        <SettingsBackupManagementSection
          locale={loc}
          autoBackupEnabled={backupPreferences.auto_backup_enabled}
          retentionCount={backupPreferences.retention_count}
          savingBackupPreferences={savingBackupPreferences}
          creatingManagedBackup={creatingManagedBackup}
          loadingManagedBackups={loadingManagedBackups}
          managedBackups={managedBackups}
          restoringBackupPath={restoringBackupPath}
          deletingBackupPath={deletingBackupPath}
          onToggleAutoBackup={handleToggleAutoBackup}
          onRetentionChange={handleBackupRetentionChange}
          onSavePreferences={handleSaveCurrentBackupPreferences}
          onCreateManagedBackup={handleCreateManagedBackup}
          onRefreshManagedBackups={handleRefreshManagedBackups}
          onRenameManagedBackup={handleRenameManagedBackup}
          onOpenBackup={openInSystemWithLabel}
          onRestoreManagedBackup={handleRestoreManagedBackup}
          onDeleteManagedBackup={handleDeleteManagedBackup}
        />

        <SettingsAppUpdateSection
          labels={appUpdateLabels}
          appVersion={appVersion}
          checkingUpdate={checkingUpdate}
          installing={installing}
          updaterDisabledByEnv={updaterDisabledByEnv}
          updaterEnvValue={updaterEnvValue}
          appUpdate={appUpdate}
          updateError={updateError}
          onCheckUpdate={handleCheckUpdate}
          onInstallUpdate={handleInstallUpdate}
        />

        <SettingsNetworkProxySection
          title={loc === "zh" ? "网络代理" : "Network Proxy"}
          description={loc === "zh"
            ? "设置 HTTP/HTTPS 代理地址，用于访问 GitHub 等外部服务。留空则使用系统默认网络。"
            : "Set HTTP/HTTPS proxy for accessing GitHub and external services. Leave empty for system default."}
          hint={loc === "zh"
            ? "提示：也可以在 VPN 软件中开启 TUN 模式让所有流量走代理，无需在此设置。"
            : "Tip: You can also enable TUN mode in your VPN client to proxy all traffic without setting this."}
          proxyUrl={proxyUrl}
          proxySaved={proxySaved}
          saveLabel={loc === "zh" ? "保存" : "Save"}
          placeholder="http://127.0.0.1:7890"
          onProxyChange={handleProxyInputChange}
          onProxyKeyDown={handleProxyInputKeyDown}
          onSave={handleSaveProxy}
        />

        <SettingsAboutSection
          title={i.settings.about}
          description={i.settings.aboutDesc}
          appVersion={appVersion}
          license={i.settings.license}
        />
      </div>
    </div>
  );
}
