import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Globe, FolderOpen, Info, Palette, Sun, Moon, Download, RefreshCw, CheckCircle, AlertCircle, Copy, Check, Upload, Archive, Wifi, Link2 } from "lucide-react";
import { t, getLocale, setLocale, type Locale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import WebDavSyncSection from "../components/WebDavSyncSection";
import CopilotAuthSection from "../components/CopilotAuthSection";
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
  getAppLabel,
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

interface CustomPath { tool_id: string; config_dir: string | null; mcp_config_path: string | null; skills_dir: string | null; }
interface DetectedTool { id: string; name: string; config_path: string; skills_dir: string; mcp_config_path: string; installed: boolean; install_command: string; install_url: string; }
interface PendingImportedProjectRoot { project_root: string; file_count: number; }
interface AutoRemapImportedProjectRootsResult { remapped_roots: number; restored_files: number; skipped_roots: number; }
interface LastImportSummary {
  imported_at: string;
  db_rows_restored: number;
  tool_configs_restored: number;
  skills_restored: number;
  full_files_restored: number;
  pending_project_files: number;
  safety_backup_path: string;
}
interface FullRescanResult {
  mcp_servers: number;
  skills: number;
  hooks: number;
  instruction_files: number;
  workflows: number;
  config_roots: number;
  pending_project_roots: number;
  tool_health_issues: number;
  manual_setup_required: number;
}
interface ToolEnvironmentReport {
  tool_id: string;
  tool_name: string;
  cli_available: boolean;
  cli_command: string;
  config_path: string;
  config_exists: boolean;
  mcp_config_path: string;
  mcp_config_exists: boolean;
  skills_dir: string;
  skills_dir_exists: boolean;
  config_dir: string;
  config_dir_exists: boolean;
  has_custom_config_dir: boolean;
  has_custom_mcp_config_path: boolean;
  has_custom_skills_dir: boolean;
  manual_setup_kind: string | null;
  manual_setup_command: string | null;
  manual_setup_path: string | null;
}
interface BootstrapToolEnvironmentResult {
  created_dirs: number;
  created_files: number;
  notes: string[];
}
interface RepairAllResult {
  remapped_roots: number;
  restored_project_files: number;
  skipped_remap_roots: number;
  bootstrapped_tools: number;
  created_dirs: number;
  created_files: number;
  bootstrap_notes: string[];
  rescan: FullRescanResult;
}
interface ManagedBackupFile {
  path: string;
  name: string;
  created_at: string;
  size_bytes: number;
  kind: string;
  can_restore: boolean;
}
interface BackupPreferences {
  auto_backup_enabled: boolean;
  retention_count: number;
}

function hasToolHealthIssue(report: ToolEnvironmentReport) {
  return !report.cli_available
    || !report.config_dir_exists
    || !report.config_exists
    || !report.mcp_config_exists
    || !report.skills_dir_exists;
}

function formatBytes(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

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
  const [pendingProjectRoots, setPendingProjectRoots] = useState<PendingImportedProjectRoot[]>([]);
  const [toolReports, setToolReports] = useState<ToolEnvironmentReport[]>([]);
  const [lastImportSummary, setLastImportSummary] = useState<LastImportSummary | null>(null);
  const [lastRescan, setLastRescan] = useState<FullRescanResult | null>(null);
  const [remapTargets, setRemapTargets] = useState<Record<string, string>>({});
  const [remappingRoot, setRemappingRoot] = useState<string | null>(null);
  const [autoMatchingPending, setAutoMatchingPending] = useState(false);
  const [bootstrappingToolId, setBootstrappingToolId] = useState<string | null>(null);
  const [repairingAll, setRepairingAll] = useState(false);
  const [rescanningAll, setRescanningAll] = useState(false);
  const [refreshingMigrationHealth, setRefreshingMigrationHealth] = useState(false);
  const [exportingBackup, setExportingBackup] = useState(false);
  const [importingBackup, setImportingBackup] = useState(false);
  const [managedBackups, setManagedBackups] = useState<ManagedBackupFile[]>([]);
  const [loadingManagedBackups, setLoadingManagedBackups] = useState(false);
  const [backupPreferences, setBackupPreferencesState] = useState<BackupPreferences>({
    auto_backup_enabled: false,
    retention_count: 20,
  });
  const [savingBackupPreferences, setSavingBackupPreferences] = useState(false);
  const [creatingManagedBackup, setCreatingManagedBackup] = useState(false);
  const [restoringBackupPath, setRestoringBackupPath] = useState<string | null>(null);
  const [deletingBackupPath, setDeletingBackupPath] = useState<string | null>(null);
  const [migrationPanelsOpen, setMigrationPanelsOpen] = useState({
    summary: false,
    pending: false,
    health: false,
    auth: false,
  });
  const migrationPanelsInitialized = useRef(false);
  const migrationPanelRefs = {
    summary: useRef<HTMLDetailsElement | null>(null),
    pending: useRef<HTMLDetailsElement | null>(null),
    health: useRef<HTMLDetailsElement | null>(null),
    auth: useRef<HTMLDetailsElement | null>(null),
  };
  const i = t();
  const loc = getLocale();
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    loc === "zh" ? zhText : loc === "ja" ? (jaText ?? enText) : enText
  );

  useEffect(() => {
    loadToolsAndPaths();
    loadProxy();
    loadSkillSyncMethod();
    loadPendingProjectRoots();
    loadLastImportSummary();
    void loadManagedBackups();
    void loadBackupPreferences();
    void runScheduledBackupCheck();
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

  async function loadPendingProjectRoots() {
    try {
      const roots = await invoke<PendingImportedProjectRoot[]>("get_pending_imported_project_roots");
      applyPendingProjectRoots(roots);
    } catch { /* ignore */ }
  }

  async function loadLastImportSummary() {
    try {
      const summary = await invoke<LastImportSummary | null>("get_last_import_summary");
      setLastImportSummary(summary);
    } catch { /* ignore */ }
  }

  async function loadManagedBackups() {
    setLoadingManagedBackups(true);
    try {
      const backups = await invoke<ManagedBackupFile[]>("list_managed_backups");
      setManagedBackups(backups);
    } catch {
      setManagedBackups([]);
    } finally {
      setLoadingManagedBackups(false);
    }
  }

  async function loadBackupPreferences() {
    try {
      const preferences = await invoke<BackupPreferences>("get_backup_preferences");
      setBackupPreferencesState(preferences);
    } catch { /* ignore */ }
  }

  async function runScheduledBackupCheck() {
    try {
      const createdPath = await invoke<string | null>("run_scheduled_backup_if_needed");
      if (createdPath) {
        await loadManagedBackups();
      }
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

  async function handleSaveLocalProviderProxySettings(next: LocalProviderProxySettings) {
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
  }

  async function refreshEnvironmentConflicts() {
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
  }

  async function updateWindowPreference<K extends keyof WindowPreferences>(key: K, value: WindowPreferences[K]) {
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
  }

  async function handleToggleVisibleApp(appId: ManagedAppId) {
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
  }

  async function handleSelectTerminal(terminalId: string) {
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
  }

  async function handleSaveLogLevel(level: string) {
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
  }

  async function handleSyncMethodChange(method: "symlink" | "copy") {
    try {
      await invoke("set_skill_sync_method", { method });
      setSkillSyncMethod(method);
      showToast("success", loc === "zh" ? "已保存" : "Saved");
    } catch (e) { showToast("error", String(e)); }
  }

  async function loadToolsAndPaths() {
    try {
      const [t, p, reports] = await Promise.all([
        invoke<DetectedTool[]>("detect_tools"),
        invoke<CustomPath[]>("get_custom_paths"),
        invoke<ToolEnvironmentReport[]>("get_tool_environment_report"),
      ]);
      setTools(t);
      setCustomPaths(p);
      setToolReports(reports);
    } catch (e) { console.error(e); }
  }

  async function refreshMigrationState() {
    await Promise.allSettled([
      loadToolsAndPaths(),
      loadPendingProjectRoots(),
      loadLastImportSummary(),
      loadManagedBackups(),
      invoke("sync_config_profiles"),
    ]);
  }

  function applyPendingProjectRoots(roots: PendingImportedProjectRoot[]) {
    setPendingProjectRoots(roots);
    setRemapTargets((current) => {
      const next: Record<string, string> = {};
      for (const item of roots) {
        next[item.project_root] = current[item.project_root] || "";
      }
      return next;
    });
  }

  async function fetchMigrationStatusCounts() {
    const [roots, reports] = await Promise.all([
      invoke<PendingImportedProjectRoot[]>("get_pending_imported_project_roots"),
      invoke<ToolEnvironmentReport[]>("get_tool_environment_report"),
    ]);
    applyPendingProjectRoots(roots);
    setToolReports(reports);
    return {
      pendingRoots: roots.length,
      healthIssues: reports.filter(hasToolHealthIssue).length,
      authGaps: reports.filter((report) => !!report.manual_setup_kind).length,
    };
  }

  function toggleMigrationPanel(panel: keyof typeof migrationPanelsOpen, open: boolean) {
    setMigrationPanelsOpen((current) => ({ ...current, [panel]: open }));
  }

  function focusMigrationPanel(panel: keyof typeof migrationPanelsOpen) {
    setMigrationPanelsOpen((current) => ({ ...current, [panel]: true }));
    window.setTimeout(() => {
      migrationPanelRefs[panel].current?.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 0);
  }

  async function runBootstrapForTool(toolId: string, toolName: string) {
    setBootstrappingToolId(toolId);
    try {
      const result = await invoke<BootstrapToolEnvironmentResult>("bootstrap_tool_environment", {
        toolId,
      });
      await refreshMigrationState();
      const message = [
        i.settings.migrationHealthBootstrapSuccess
          .replace("{dirs}", String(result.created_dirs))
          .replace("{files}", String(result.created_files)),
        ...result.notes,
      ].join("；");
      showToast("success", message || `${toolName} updated`);
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setBootstrappingToolId((current) => current === toolId ? null : current);
    }
  }

  async function openInSystemWithLabel(target: string, label: string) {
    try {
      await invoke("open_in_system", { target });
    } catch (e) {
      showToast(
        "error",
        `${i.settings.openFailed.replace("{label}", label)}: ${String(e)}`
      );
    }
  }

  async function copyText(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      showToast("success", i.settings.copied.replace("{label}", label));
    } catch (e) {
      showToast("error", String(e));
    }
  }

  async function handleExportBackup() {
    setExportingBackup(true);
    try {
      const path = await invoke<string>("save_backup_to_file");
      showToast("success", loc === "zh" ? `备份已保存到: ${path}` : `Backup saved to: ${path}`);
    } catch (e) {
      if (String(e) !== "Cancelled") showToast("error", String(e));
    } finally {
      setExportingBackup(false);
    }
  }

  async function handleImportBackup() {
    setImportingBackup(true);
    try {
      const msg = await invoke<string>("import_backup_from_file");
      await refreshMigrationState();
      showToast("success", msg);
    } catch (e) {
      if (String(e) !== "Cancelled") showToast("error", String(e));
    } finally {
      setImportingBackup(false);
    }
  }

  async function handleCreateManagedBackup() {
    setCreatingManagedBackup(true);
    try {
      const path = await invoke<string>("create_managed_backup", { kind: "manual" });
      await loadManagedBackups();
      showToast("success", loc === "zh" ? `备份已创建: ${path}` : `Backup created: ${path}`);
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setCreatingManagedBackup(false);
    }
  }

  async function handleSaveBackupPreferences(next: BackupPreferences) {
    setSavingBackupPreferences(true);
    try {
      const saved = await invoke<BackupPreferences>("set_backup_preferences", { preferences: next });
      setBackupPreferencesState(saved);
      showToast("success", loc === "zh" ? "备份策略已保存" : "Backup preferences saved");
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setSavingBackupPreferences(false);
    }
  }

  async function handleRenameManagedBackup(backup: ManagedBackupFile) {
    const nextName = window.prompt(
      loc === "zh" ? "输入新的备份名称" : "Enter a new backup name",
      backup.name.replace(/\.sql$/i, ""),
    );
    if (!nextName || nextName.trim() === "" || nextName.trim() === backup.name.replace(/\.sql$/i, "")) {
      return;
    }
    try {
      await invoke("rename_managed_backup", { path: backup.path, newName: nextName.trim() });
      await loadManagedBackups();
      showToast("success", loc === "zh" ? "备份已重命名" : "Backup renamed");
    } catch (e) {
      showToast("error", String(e));
    }
  }

  async function handleDeleteManagedBackup(backup: ManagedBackupFile) {
    if (!window.confirm(loc === "zh" ? `删除备份「${backup.name}」？` : `Delete backup "${backup.name}"?`)) {
      return;
    }
    setDeletingBackupPath(backup.path);
    try {
      await invoke("delete_managed_backup", { path: backup.path });
      await loadManagedBackups();
      showToast("success", loc === "zh" ? "备份已删除" : "Backup deleted");
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setDeletingBackupPath((current) => current === backup.path ? null : current);
    }
  }

  async function handleRestoreManagedBackup(backup: ManagedBackupFile) {
    if (!window.confirm(loc === "zh" ? `恢复备份「${backup.name}」？这会覆盖当前数据库。` : `Restore backup "${backup.name}"? This replaces the current database.`)) {
      return;
    }
    setRestoringBackupPath(backup.path);
    try {
      const message = await invoke<string>("restore_managed_backup", { path: backup.path });
      await refreshMigrationState();
      showToast("success", message);
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setRestoringBackupPath((current) => current === backup.path ? null : current);
    }
  }

  async function handleFullRescan() {
    setRescanningAll(true);
    try {
      const result = await invoke<FullRescanResult>("run_full_rescan");
      setLastRescan(result);
      await refreshMigrationState();
      showToast(
        "success",
        i.settings.fullRescanSuccess
          .replace("{mcp}", String(result.mcp_servers))
          .replace("{skills}", String(result.skills))
          .replace("{hooks}", String(result.hooks))
          .replace("{docs}", String(result.instruction_files))
      );
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setRescanningAll(false);
    }
  }

  async function handleRepairAll() {
    setRepairingAll(true);
    try {
      const result = await invoke<RepairAllResult>("repair_all_migration_issues");
      setLastRescan(result.rescan);
      await refreshMigrationState();
      const status = await fetchMigrationStatusCounts();
      showToast(
        "success",
        i.settings.pendingImportsRepairAllSuccess
          .replace("{roots}", String(result.remapped_roots))
          .replace("{files}", String(result.restored_project_files))
          .replace("{tools}", String(result.bootstrapped_tools))
          .replace("{pending}", String(status.pendingRoots))
          .replace("{issues}", String(status.healthIssues))
          .replace("{auth}", String(status.authGaps))
      );
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setRepairingAll(false);
    }
  }

  async function handleAutoMatchPending() {
    setAutoMatchingPending(true);
    try {
      const result = await invoke<AutoRemapImportedProjectRootsResult>("auto_remap_imported_project_roots");
      await refreshMigrationState();
      const status = await fetchMigrationStatusCounts();
      showToast(
        "success",
        i.settings.pendingImportsAutoMatchSuccess
          .replace("{roots}", String(result.remapped_roots))
          .replace("{files}", String(result.restored_files))
          .replace("{skipped}", String(result.skipped_roots))
          .replace("{pending}", String(status.pendingRoots))
          .replace("{issues}", String(status.healthIssues))
      );
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setAutoMatchingPending(false);
    }
  }

  async function handleRefreshMigrationHealth() {
    setRefreshingMigrationHealth(true);
    try {
      const [detectedTools, savedPaths, reports] = await Promise.all([
        invoke<DetectedTool[]>("detect_tools"),
        invoke<CustomPath[]>("get_custom_paths"),
        invoke<ToolEnvironmentReport[]>("get_tool_environment_report"),
      ]);
      setTools(detectedTools);
      setCustomPaths(savedPaths);
      setToolReports(reports);
      showToast(
        "success",
        i.settings.migrationHealthRefreshSuccess.replace(
          "{count}",
          String(reports.filter(hasToolHealthIssue).length)
        )
      );
    } catch (e) {
      console.error(e);
      showToast("error", String(e));
    } finally {
      setRefreshingMigrationHealth(false);
    }
  }

  function handleLocaleChange(newLocale: Locale) {
    setLocale(newLocale);
    setLoc(newLocale);
    window.location.reload();
  }

  function handleThemeChange(newTheme: Theme) {
    setTheme(newTheme);
    setThm(newTheme);
  }

  async function handleCheckUpdate() {
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
  }

  async function handleInstallUpdate() {
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
  }

  const toolMeta = new Map(tools.map((tool) => [tool.id, tool] as const));
  const visibleToolIds = new Set(visibleApps);
  const visibleTools = tools.filter((tool) => visibleToolIds.has(tool.id as ManagedAppId));
  const toolHealthIssues = toolReports.filter(hasToolHealthIssue);
  const manualSetupReports = toolReports.filter((report) => !!report.manual_setup_kind);
  const pendingProjectFiles = pendingProjectRoots.reduce((sum, item) => sum + item.file_count, 0);
  const migrationReady = pendingProjectRoots.length === 0 && toolHealthIssues.length === 0 && manualSetupReports.length === 0;
  const updaterDisabledByEnv = (updaterEnvironmentState?.disabled_by_env ?? false) || (appUpdate?.disabled_by_env ?? false);
  const updaterEnvValue = updaterEnvironmentState?.env_var_value ?? null;

  useEffect(() => {
    if (migrationPanelsInitialized.current) return;
    if (tools.length === 0 && !lastImportSummary && pendingProjectRoots.length === 0 && toolReports.length === 0) return;
    setMigrationPanelsOpen({
      summary: !!lastImportSummary,
      pending: pendingProjectRoots.length > 0,
      health: toolHealthIssues.length > 0,
      auth: manualSetupReports.length > 0,
    });
    migrationPanelsInitialized.current = true;
  }, [lastImportSummary, manualSetupReports.length, pendingProjectRoots.length, toolHealthIssues.length, toolReports.length, tools.length]);

  const migrationOverviewCards = [
    {
      panel: "pending" as const,
      label: i.settings.pendingImports,
      value: pendingProjectRoots.length,
      tone: pendingProjectRoots.length > 0 ? "warning" : "ready",
      helper: pendingProjectRoots.length > 0
        ? (loc === "zh" ? "需要恢复路径" : "Needs path repair")
        : (loc === "zh" ? "已处理" : "Resolved"),
    },
    {
      panel: "summary" as const,
      label: i.settings.importSummaryPending,
      value: pendingProjectFiles,
      tone: pendingProjectFiles > 0 ? "warning" : "neutral",
      helper: lastImportSummary
        ? (loc === "zh" ? "查看最近导入" : "Review latest import")
        : (loc === "zh" ? "暂无导入记录" : "No recent import"),
    },
    {
      panel: "health" as const,
      label: i.settings.migrationHealth,
      value: toolHealthIssues.length,
      tone: toolHealthIssues.length > 0 ? "danger" : "ready",
      helper: toolHealthIssues.length > 0
        ? (loc === "zh" ? "优先处理环境缺失" : "Fix environment gaps first")
        : (loc === "zh" ? "环境正常" : "Environment ready"),
    },
    {
      panel: "auth" as const,
      label: i.settings.authGuide,
      value: manualSetupReports.length,
      tone: manualSetupReports.length > 0 ? "warning" : "ready",
      helper: manualSetupReports.length > 0
        ? (loc === "zh" ? "仍需手动认证" : "Manual auth still required")
        : (loc === "zh" ? "无需补全" : "No manual auth needed"),
    },
  ];

  return (
    <div className="animate-in">
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.settings.title}</h2>
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
        {/* Appearance */}
        <div className="section-card">
          <div className="section-card-title">
            <Palette size={17} style={{ color: "var(--text-secondary)" }} />
            {i.settings.appearance}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
            {/* Theme */}
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.theme}</p>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                {([["dark", i.settings.dark, Moon], ["light", i.settings.light, Sun]] as [Theme, string, typeof Moon][]).map(([key, label, Icon]) => (
                  <button
                    key={key}
                    className={`btn btn-sm ${theme === key ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => handleThemeChange(key)}
                    style={{ gap: 6 }}
                  >
                    <Icon size={14} />{label}
                  </button>
                ))}
              </div>
            </div>

            <div className="divider" />

            {/* Language */}
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <Globe size={16} style={{ color: "var(--text-secondary)" }} />
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.language}</p>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                {([["zh", "中文"], ["en", "English"], ["ja", "日本語"]] as [Locale, string][]).map(([key, label]) => (
                  <button
                    key={key}
                    className={`btn btn-sm ${locale === key ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => handleLocaleChange(key)}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* General */}
        <div className="section-card">
          <div className="section-card-title">{i.settings.general}</div>

          <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.autoScan}</p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{i.settings.autoScanDesc}</p>
              </div>
              <button className={`toggle ${autoScan ? "on" : "off"}`} onClick={() => setAutoScan(!autoScan)}>
                <div className="toggle-knob" />
              </button>
            </div>

            <div className="divider" />

            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.checkUpdates}</p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{i.settings.checkUpdatesDesc}</p>
              </div>
              <button className={`toggle ${checkUpdates ? "on" : "off"}`} onClick={() => setCheckUpdates(!checkUpdates)}>
                <div className="toggle-knob" />
              </button>
            </div>

            <div className="divider" />

            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <Link2 size={15} style={{ color: "var(--text-secondary)" }} />
                  <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.skillSyncMethod}</p>
                </div>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{i.settings.skillSyncDesc}</p>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                {(["symlink", "copy"] as const).map((m) => (
                  <button
                    key={m}
                    className={`btn btn-sm ${skillSyncMethod === m ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => handleSyncMethodChange(m)}
                  >
                    {m === "symlink" ? i.settings.skillSyncSymlink : i.settings.skillSyncCopy}
                  </button>
                ))}
              </div>
            </div>
            {skillSyncMethod === "symlink" && (
              <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: -8 }}>
                {i.settings.skillSyncSymlinkHint}
              </p>
            )}
          </div>
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <AlertCircle size={17} style={{ color: environmentConflicts.length > 0 ? "var(--warning)" : "var(--text-secondary)" }} />
            {uiText("环境变量冲突检测", "Environment Override Detection", "環境変数上書き検出")}
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start", flexWrap: "wrap", marginBottom: 14 }}>
            <p style={{ fontSize: 12, color: "var(--text-muted)", maxWidth: 720 }}>
              {uiText(
                "检测当前桌面进程继承的 CLI 环境变量。若这些变量已存在，可能覆盖 CCHub 的配置文件、Profile 切换和端点设置。",
                "Detect inherited CLI environment variables that may override CCHub-managed config files, profile switching, and endpoint settings.",
                "デスクトッププロセスが引き継いだ CLI 環境変数を検出します。存在する場合、CCHub 管理の設定ファイル、Profile 切替、エンドポイント設定を上書きする可能性があります。",
              )}
            </p>
            <button
              className="btn btn-secondary btn-sm"
              onClick={refreshEnvironmentConflicts}
              disabled={refreshingEnvConflicts}
              style={{ gap: 6 }}
            >
              <RefreshCw size={14} className={refreshingEnvConflicts ? "spin" : ""} />
              {refreshingEnvConflicts ? i.settings.checking : i.settings.migrationHealthRefresh}
            </button>
          </div>
          {environmentConflicts.length === 0 ? (
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
              <CheckCircle size={16} style={{ color: "var(--success)" }} />
              {uiText("未发现会覆盖配置的环境变量", "No overriding environment variables were detected", "設定を上書きする環境変数は検出されませんでした")}
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {environmentConflicts.map((conflict) => (
                <div key={conflict.id} style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap", marginBottom: 8 }}>
                    <div style={{ fontSize: 13, fontWeight: 600 }}>
                      {conflict.kind === "multi_tool_override"
                        ? uiText("检测到多套 CLI 环境覆盖", "Multiple CLI override groups detected", "複数の CLI 上書きグループを検出しました")
                        : uiText(
                          `${getAppLabel(conflict.affected_apps[0] as ManagedAppId)} 环境覆盖`,
                          `${getAppLabel(conflict.affected_apps[0] as ManagedAppId)} override detected`,
                          `${getAppLabel(conflict.affected_apps[0] as ManagedAppId)} の上書きを検出しました`,
                        )}
                    </div>
                    <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                      {conflict.affected_apps.map((appId) => (
                        <span key={appId} className="badge badge-warning" style={{ fontSize: 10 }}>
                          {getAppLabel(appId as ManagedAppId)}
                        </span>
                      ))}
                    </div>
                  </div>
                  <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 8 }}>
                    {conflict.kind === "multi_tool_override"
                      ? uiText(
                        "多个工具的认证或端点变量同时存在，常见于历史 shell 配置残留。",
                        "Auth or endpoint variables for multiple tools are present at the same time, often from old shell profile exports.",
                        "複数ツールの認証またはエンドポイント変数が同時に存在します。古い shell 設定の残骸であることが多いです。",
                      )
                      : uiText(
                        "这些变量会优先于 CCHub 写入的配置文件生效。",
                        "These variables take precedence over configuration files managed by CCHub.",
                        "これらの変数は CCHub が管理する設定ファイルより優先されます。",
                      )}
                  </div>
                  <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                    {conflict.variables.map((item) => (
                      <code key={item} className="badge badge-accent" style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace" }}>{item}</code>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Info size={17} style={{ color: "var(--text-secondary)" }} />
            {uiText("窗口设置", "Window Behavior", "ウィンドウ動作")}
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 18 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{uiText("开机自启", "Launch at login", "ログイン時に起動")}</p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>
                  {uiText(
                    "写入当前平台的自启动项，随系统登录自动拉起 CCHub。",
                    "Create a platform-specific autostart entry and launch CCHub when you sign in.",
                    "現在のプラットフォーム向け自動起動項目を作成し、サインイン時に CCHub を起動します。",
                  )}
                </p>
              </div>
              <button
                className={`toggle ${windowPreferences.launch_at_login ? "on" : "off"}`}
                onClick={() => updateWindowPreference("launch_at_login", !windowPreferences.launch_at_login)}
                disabled={savingWindowKey === "launch_at_login"}
              >
                <div className="toggle-knob" />
              </button>
            </div>

            <div className="divider" />

            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{uiText("静默启动", "Launch hidden", "非表示で起動")}</p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>
                  {uiText(
                    "启动后先隐藏到托盘，适合配合开机自启使用。",
                    "Hide the main window immediately after launch. Useful together with autostart.",
                    "起動直後にメインウィンドウをトレイへ隠します。自動起動との併用に向いています。",
                  )}
                </p>
              </div>
              <button
                className={`toggle ${windowPreferences.launch_hidden ? "on" : "off"}`}
                onClick={() => updateWindowPreference("launch_hidden", !windowPreferences.launch_hidden)}
                disabled={savingWindowKey === "launch_hidden"}
              >
                <div className="toggle-knob" />
              </button>
            </div>

            <div className="divider" />

            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{uiText("关闭到托盘", "Close to tray", "閉じるとトレイへ")}</p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>
                  {uiText(
                    "关闭主窗口时不退出进程，而是最小化留在系统托盘。",
                    "Hide to the tray instead of quitting when the main window is closed.",
                    "メインウィンドウを閉じたときに終了せず、システムトレイへ隠します。",
                  )}
                </p>
              </div>
              <button
                className={`toggle ${windowPreferences.close_to_tray ? "on" : "off"}`}
                onClick={() => updateWindowPreference("close_to_tray", !windowPreferences.close_to_tray)}
                disabled={savingWindowKey === "close_to_tray"}
              >
                <div className="toggle-knob" />
              </button>
            </div>
          </div>
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Link2 size={17} style={{ color: "var(--text-secondary)" }} />
            {uiText("首选终端", "Preferred Terminal", "優先ターミナル")}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
            {uiText(
              "为后续会话恢复和目录打开动作指定默认终端，现在也可以直接用下方按钮验证。",
              "Choose the terminal CCHub should prefer for future session restore and open-in-terminal actions. You can test it below now.",
              "将来のセッション復元やディレクトリをターミナルで開く操作に使う既定ターミナルを選択します。下のボタンですぐ確認できます。",
            )}
          </p>
          {terminalPreferences ? (
            <>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(190px, 1fr))", gap: 10 }}>
                {terminalPreferences.options.map((option) => {
                  const active = option.id === terminalPreferences.selected_terminal;
                  return (
                    <button
                      key={option.id}
                      type="button"
                      onClick={() => handleSelectTerminal(option.id)}
                      disabled={savingTerminal}
                      style={{
                        padding: "12px 14px",
                        borderRadius: 10,
                        border: `1px solid ${active ? "var(--text-primary)" : "var(--border-default)"}`,
                        background: active ? "var(--bg-elevated)" : "var(--bg-input)",
                        textAlign: "left",
                        cursor: "pointer",
                        opacity: option.installed ? 1 : 0.55,
                      }}
                    >
                      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", marginBottom: 8 }}>
                        <span style={{ fontSize: 13, fontWeight: 600 }}>{option.label}</span>
                        <span className={`badge ${option.installed ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
                          {option.installed ? uiText("已检测", "Detected", "検出済み") : uiText("未检测", "Missing", "未検出")}
                        </span>
                      </div>
                      <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>{option.command}</div>
                    </button>
                  );
                })}
              </div>
              <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap", marginTop: 14 }}>
                <span className="badge badge-accent">
                  {uiText(`当前平台: ${terminalPreferences.platform}`, `Platform: ${terminalPreferences.platform}`, `現在のプラットフォーム: ${terminalPreferences.platform}`)}
                </span>
                <button
                  className="btn btn-secondary btn-sm"
                  style={{ gap: 6 }}
                  onClick={async () => {
                    try {
                      await invoke("open_in_preferred_terminal", { path: null });
                    } catch (e) {
                      showToast("error", String(e));
                    }
                  }}
                >
                  <FolderOpen size={14} />
                  {uiText("在终端中打开主目录", "Open Home In Terminal", "ホームをターミナルで開く")}
                </button>
              </div>
            </>
          ) : (
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{uiText("正在读取终端列表...", "Loading terminal options...", "ターミナル一覧を読み込み中...")}</div>
          )}
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Globe size={17} style={{ color: "var(--text-secondary)" }} />
            {uiText("App 可见性", "App Visibility", "App 表示設定")}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
            {uiText(
              "控制工具页、配置文件、工作流、指令文档等页面中的 App 标签显示。至少保留一个。",
              "Control which app tabs appear across Tools, Config Files, Workflows, Instruction Docs, and related pages. Keep at least one visible.",
              "Tools、設定ファイル、ワークフロー、指示ドキュメントなどで表示する App タブを制御します。少なくとも 1 つは残してください。",
            )}
          </p>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            {MANAGED_APPS.map((appId) => {
              const active = visibleApps.includes(appId);
              return (
                <button
                  key={appId}
                  className={`btn btn-sm ${active ? "btn-primary" : "btn-secondary"}`}
                  onClick={() => handleToggleVisibleApp(appId)}
                  disabled={savingVisibleApps}
                  style={{ gap: 6 }}
                >
                  {getAppLabel(appId)}
                  <span className={`dot ${active ? "dot-active" : "dot-disabled"}`} />
                </button>
              );
            })}
          </div>
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 10 }}>
            {uiText(
              `当前显示 ${visibleApps.length} / ${MANAGED_APPS.length} 个 App`,
              `${visibleApps.length} / ${MANAGED_APPS.length} apps currently visible`,
              `${visibleApps.length} / ${MANAGED_APPS.length} 個の App を表示中`,
            )}
          </p>
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Link2 size={17} style={{ color: "var(--text-secondary)" }} />
            {uiText("本地 Provider 代理", "Local Provider Proxy", "ローカル Provider プロキシ")}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
            {uiText(
              "把 Claude / Codex / Gemini / OpenCode / OpenClaw 的活动 Provider 改写到本机回环地址，由 CCHub 在请求时动态转发到当前选中的 Provider。切换 Provider 时无需重启代理服务。",
              "Rewrite supported app endpoints to a local loopback address so CCHub can forward requests to the currently active provider at request time. Provider switches do not require restarting the proxy service.",
              "対応 App のエンドポイントをローカルループバックへ書き換え、CCHub がリクエスト時点のアクティブ Provider へ動的転送します。Provider 切替でプロキシ再起動は不要です。",
            )}
          </p>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 12, marginBottom: 14 }}>
            <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
                {uiText("监听地址", "Listen Address", "待受アドレス")}
              </div>
              <div style={{ fontSize: 13, fontWeight: 600, fontFamily: "'JetBrains Mono', monospace" }}>
                {localProviderProxyStatus?.base_url || `http://127.0.0.1:${localProviderProxySettings.port}/proxy`}
              </div>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
                {localProviderProxyStatus?.running
                  ? uiText("代理服务运行中", "Proxy server is running", "プロキシサーバーは稼働中です")
                  : uiText("当前未运行，至少启用一个 App 后保存即可启动", "Not running. Enable at least one app and save to start it.", "現在は停止中です。少なくとも 1 つの App を有効化して保存すると起動します。")}
              </div>
            </div>

            <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
                {uiText("监听端口", "Listen Port", "待受ポート")}
              </div>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <input
                  className="input"
                  type="number"
                  min={1024}
                  max={65535}
                  value={localProviderProxySettings.port}
                  onChange={(event) => {
                    const nextPort = Math.max(1024, Math.min(65535, Number(event.target.value) || 34567));
                    setLocalProviderProxySettingsState((current) => ({ ...current, port: nextPort }));
                  }}
                  style={{ maxWidth: 130 }}
                />
                <button
                  className="btn btn-primary btn-sm"
                  onClick={() => void handleSaveLocalProviderProxySettings(localProviderProxySettings)}
                  disabled={savingLocalProviderProxy}
                >
                  {savingLocalProviderProxy
                    ? uiText("保存中...", "Saving...", "保存中...")
                    : uiText("保存代理设置", "Save Proxy Settings", "プロキシ設定を保存")}
                </button>
              </div>
            </div>
          </div>

          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 14 }}>
            {MANAGED_APPS.map((appId) => {
              const active = localProviderProxySettings.enabled_apps.includes(appId);
              return (
                <button
                  key={`local-proxy:${appId}`}
                  className={`btn btn-sm ${active ? "btn-primary" : "btn-secondary"}`}
                  disabled={savingLocalProviderProxy}
                  onClick={() => {
                    setLocalProviderProxySettingsState((current) => {
                      const enabled = current.enabled_apps.includes(appId)
                        ? current.enabled_apps.filter((item) => item !== appId)
                        : [...current.enabled_apps, appId];
                      return { ...current, enabled_apps: enabled };
                    });
                  }}
                  style={{ gap: 6 }}
                >
                  {getAppLabel(appId)}
                  <span className={`dot ${active ? "dot-active" : "dot-disabled"}`} />
                </button>
              );
            })}
          </div>

          <p style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 14 }}>
            {uiText(
              "当前版本已具备本地代理接管、按 App 开关、热切换，以及基于 endpointCandidates 的候选端点自动重试。基础请求统计已接入 Dashboard；格式转换、完整故障转移队列和成本计费仍会继续补齐。",
              "This revision now includes local proxy takeover, per-app toggles, hot switching, and automatic retries across configured endpoint candidates. Basic request statistics are available on the Dashboard; format conversion, full failover queues, and cost accounting are still pending.",
              "この版ではローカルプロキシ接管、App 単位の切替、ホットスイッチ、endpointCandidates に基づく候補エンドポイント自動再試行まで利用できます。基本的なリクエスト統計は Dashboard に表示され、形式変換・完全なフェイルオーバーキュー・コスト計算は引き続き実装予定です。",
            )}
          </p>

          {localProviderProxySettings.enabled_apps.length > 0 && (
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 10 }}>
              {localProviderProxySettings.enabled_apps
                .filter((appId): appId is ManagedAppId => MANAGED_APPS.includes(appId as ManagedAppId))
                .map((appId) => {
                const endpoint = `${localProviderProxyStatus?.base_url || `http://127.0.0.1:${localProviderProxySettings.port}/proxy`}/${appId}`;
                return (
                  <div key={`local-proxy-endpoint:${appId}`} style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", marginBottom: 8 }}>
                      <div style={{ fontSize: 13, fontWeight: 600 }}>{getAppLabel(appId)}</div>
                      <button
                        className="btn btn-ghost btn-icon-sm"
                        onClick={() => void copyText(endpoint, `${getAppLabel(appId)} Proxy URL`)}
                        title={uiText("复制代理地址", "Copy proxy URL", "プロキシ URL をコピー")}
                      >
                        <Copy size={12} />
                      </button>
                    </div>
                    <div style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
                      {endpoint}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Info size={17} style={{ color: "var(--text-secondary)" }} />
            {uiText("日志级别", "Log Level", "ログレベル")}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
            {uiText(
              "控制运行期日志详细程度。设置会立即影响 `~/.cchub/app.log` 的写入阈值，`crash.log` 仍持续记录崩溃信息。",
              "Control runtime log verbosity. Changes take effect immediately for `~/.cchub/app.log`, while `crash.log` continues to capture crash reports.",
              "実行時ログの詳細度を制御します。設定は `~/.cchub/app.log` の出力しきい値へ即時反映され、`crash.log` は引き続きクラッシュ情報を記録します。",
            )}
          </p>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))", gap: 10, marginBottom: 14 }}>
            {[
              ["error", uiText("仅记录错误与崩溃", "Errors and crashes only", "エラーとクラッシュのみ")],
              ["warn", uiText("记录警告、错误与崩溃", "Warnings, errors, and crashes", "警告・エラー・クラッシュ")],
              ["info", uiText("记录常规操作与状态", "Operational events and status", "通常操作と状態")],
              ["debug", uiText("记录详细调试步骤", "Detailed debugging steps", "詳細なデバッグ手順")],
              ["trace", uiText("记录最细粒度诊断信息", "Most verbose diagnostics", "最も詳細な診断情報")],
            ].map(([level, description]) => {
              const active = logPreferences.level === level;
              return (
                <button
                  key={level}
                  type="button"
                  className={`btn btn-sm ${active ? "btn-primary" : "btn-secondary"}`}
                  onClick={() => void handleSaveLogLevel(level)}
                  disabled={savingLogPreferences}
                  style={{ justifyContent: "space-between", alignItems: "flex-start", padding: "12px 14px", height: "auto", textAlign: "left" }}
                >
                  <span>
                    <span style={{ display: "block", fontFamily: "'JetBrains Mono', monospace", fontSize: 12, marginBottom: 6 }}>
                      {level.toUpperCase()}
                    </span>
                    <span style={{ display: "block", fontSize: 12, color: active ? "inherit" : "var(--text-secondary)", lineHeight: 1.5 }}>
                      {description}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
          {logFileTargets && (
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: 10 }}>
              {[
                ["app.log", logFileTargets.runtime_log_path, uiText("运行期操作日志", "Runtime activity log", "実行時アクティビティログ")],
                ["crash.log", logFileTargets.crash_log_path, uiText("崩溃与 panic 记录", "Crash and panic log", "クラッシュと panic のログ")],
              ].map(([label, path, description]) => (
                <div key={label} style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", marginBottom: 8 }}>
                    <div>
                      <div style={{ fontSize: 13, fontWeight: 600 }}>{label}</div>
                      <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{description}</div>
                    </div>
                    <div style={{ display: "flex", gap: 6 }}>
                      <button
                        className="btn btn-ghost btn-icon-sm"
                        onClick={() => copyText(path, label)}
                        title={uiText("复制路径", "Copy path", "パスをコピー")}
                      >
                        <Copy size={12} />
                      </button>
                      <button
                        className="btn btn-ghost btn-icon-sm"
                        onClick={() => openInSystemWithLabel(path, label)}
                        title={uiText("打开日志", "Open log", "ログを開く")}
                      >
                        <FolderOpen size={12} />
                      </button>
                    </div>
                  </div>
                  <div style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
                    {path}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Info size={17} style={{ color: "var(--text-secondary)" }} />
            {uiText("键盘快捷键", "Keyboard Shortcuts", "キーボードショートカット")}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 10 }}>
            {[
              {
                key: "Ctrl/Cmd + ,",
                desc: uiText("全局打开设置页", "Open Settings globally", "設定ページを開く"),
              },
              {
                key: "Esc",
                desc: uiText("关闭技能编辑器、工作流编辑器、指令文档编辑器等面板", "Close editors and drilldown panels such as Skills, Workflows, and Instruction Docs", "スキル、ワークフロー、指示ドキュメントなどの編集・詳細パネルを閉じる"),
              },
              {
                key: "Ctrl/Cmd + S",
                desc: uiText("在支持的编辑页快速保存", "Save on supported editor pages", "対応する編集ページですばやく保存"),
              },
              {
                key: "Ctrl/Cmd + N",
                desc: uiText("在支持的页面新建配置、预设或向导", "Create a new profile, preset, or wizard flow on supported pages", "対応ページで新規プロファイル、プリセット、ウィザードを開始"),
              },
              {
                key: "Ctrl/Cmd + F",
                desc: uiText("聚焦当前页面的主搜索框（Profiles / Skills / Marketplace）", "Focus the primary search field on the current page (Profiles / Skills / Marketplace)", "現在のページの主検索欄へフォーカス（Profiles / Skills / Marketplace）"),
              },
            ].map((item) => (
              <div key={item.key} style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
                <div style={{ marginBottom: 6 }}>
                  <code className="badge badge-accent" style={{ fontFamily: "'JetBrains Mono', monospace" }}>{item.key}</code>
                </div>
                <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.6 }}>{item.desc}</div>
              </div>
            ))}
          </div>
        </div>

        {/* Tool Paths */}
        <div className="section-card">
          <div className="section-card-title">
            <FolderOpen size={17} style={{ color: "var(--text-secondary)" }} />
            {loc === "zh" ? "工具路径配置" : "Tool Path Configuration"}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {visibleTools.map((tool) => {
              const custom = customPaths.find(p => p.tool_id === tool.id);
              return (
                <div key={tool.id} style={{ padding: "12px 16px", borderRadius: 8, background: "var(--bg-input)", opacity: tool.installed ? 1 : 0.6 }}>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <span style={{ fontSize: 13, fontWeight: 600 }}>{tool.name}</span>
                      <span className={`badge ${tool.installed ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
                        {tool.installed ? (loc === "zh" ? "已安装" : "Installed") : (loc === "zh" ? "未安装" : "Not installed")}
                      </span>
                    </div>
                    {pathSaved === tool.id && <Check size={14} style={{ color: "var(--success)" }} />}
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      <span style={{ fontSize: 11, color: "var(--text-muted)", width: 80, flexShrink: 0 }}>MCP</span>
                      <input
                        className="input"
                        style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace", padding: "4px 8px", height: 28, flex: 1 }}
                        defaultValue={custom?.mcp_config_path || tool.mcp_config_path}
                        onBlur={async (e) => {
                          const val = e.target.value.trim();
                          if (val && val !== tool.mcp_config_path) {
                            await invoke("save_custom_path", { toolId: tool.id, configDir: custom?.config_dir || null, mcpConfigPath: val, skillsDir: custom?.skills_dir || null });
                            setPathSaved(tool.id); setTimeout(() => setPathSaved(null), 2000);
                            loadToolsAndPaths();
                          }
                        }}
                      />
                      <button className="btn btn-ghost btn-icon-sm" title={loc === "zh" ? "选择文件" : "Pick file"}
                        onClick={async () => {
                          const picked = await invoke<string | null>("pick_file");
                          if (picked) {
                            await invoke("save_custom_path", { toolId: tool.id, configDir: custom?.config_dir || null, mcpConfigPath: picked, skillsDir: custom?.skills_dir || null });
                            setPathSaved(tool.id); setTimeout(() => setPathSaved(null), 2000);
                            loadToolsAndPaths();
                          }
                        }}>
                        <FolderOpen size={12} />
                      </button>
                    </div>
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      <span style={{ fontSize: 11, color: "var(--text-muted)", width: 80, flexShrink: 0 }}>Skills</span>
                      <input
                        className="input"
                        style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace", padding: "4px 8px", height: 28, flex: 1 }}
                        defaultValue={custom?.skills_dir || tool.skills_dir}
                        onBlur={async (e) => {
                          const val = e.target.value.trim();
                          if (val && val !== tool.skills_dir) {
                            await invoke("save_custom_path", { toolId: tool.id, configDir: custom?.config_dir || null, mcpConfigPath: custom?.mcp_config_path || null, skillsDir: val });
                            setPathSaved(tool.id); setTimeout(() => setPathSaved(null), 2000);
                            loadToolsAndPaths();
                          }
                        }}
                      />
                      <button className="btn btn-ghost btn-icon-sm" title={loc === "zh" ? "选择文件夹" : "Pick folder"}
                        onClick={async () => {
                          const picked = await invoke<string | null>("pick_folder");
                          if (picked) {
                            await invoke("save_custom_path", { toolId: tool.id, configDir: custom?.config_dir || null, mcpConfigPath: custom?.mcp_config_path || null, skillsDir: picked });
                            setPathSaved(tool.id); setTimeout(() => setPathSaved(null), 2000);
                            loadToolsAndPaths();
                          }
                        }}>
                        <FolderOpen size={12} />
                      </button>
                    </div>
                  </div>
                  {!tool.installed && tool.install_command && (
                    <div style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 6 }}>
                      <code style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>{tool.install_command}</code>
                      <button
                        className="btn btn-ghost btn-icon-sm"
                        onClick={() => copyText(tool.install_command, loc === "zh" ? `${tool.name} 安装命令` : `${tool.name} install command`)}
                        title="Copy"
                      >
                        <Copy size={11} />
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Link2 size={17} style={{ color: "var(--text-secondary)" }} />
            {uiText("Auth Center", "Auth Center", "Auth Center")}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
            {uiText(
              "集中管理需要 OAuth 的第三方账号。当前已接入 GitHub Copilot，多账号登录后可在 Profiles 页把某个 Provider 绑定到指定账号。",
              "Manage OAuth-backed third-party accounts in one place. GitHub Copilot is wired in now; after adding accounts you can bind a provider to a specific account from Profiles.",
              "OAuth が必要なサードパーティアカウントをここでまとめて管理します。現在は GitHub Copilot に対応しており、追加後は Profiles で Provider ごとにアカウントを紐付けできます。",
            )}
          </p>
          <CopilotAuthSection />
        </div>

        <WebDavSyncSection />

        <div className="section-card">
          <div className="section-card-title">
            <Archive size={17} style={{ color: "var(--text-secondary)" }} />
            {i.settings.migrationCenter}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
            {i.settings.migrationCenterDesc}
          </p>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))", gap: 10, marginBottom: 16 }}>
            {migrationOverviewCards.map(({ panel, label, value, tone, helper }) => {
              const isActive = migrationPanelsOpen[panel];
              const palette = tone === "danger"
                ? {
                    border: "rgba(239, 68, 68, 0.35)",
                    background: "rgba(239, 68, 68, 0.08)",
                    valueColor: "var(--error)",
                    badgeBg: "rgba(239, 68, 68, 0.14)",
                    badgeColor: "var(--error)",
                  }
                : tone === "warning"
                  ? {
                      border: "rgba(245, 158, 11, 0.35)",
                      background: "rgba(245, 158, 11, 0.08)",
                      valueColor: "var(--warning)",
                      badgeBg: "rgba(245, 158, 11, 0.14)",
                      badgeColor: "var(--warning)",
                    }
                  : tone === "ready"
                    ? {
                        border: "rgba(34, 197, 94, 0.28)",
                        background: "rgba(34, 197, 94, 0.07)",
                        valueColor: "var(--success)",
                        badgeBg: "rgba(34, 197, 94, 0.12)",
                        badgeColor: "var(--success)",
                      }
                    : {
                        border: "var(--border-color)",
                        background: "var(--bg-input)",
                        valueColor: "var(--text-primary)",
                        badgeBg: "var(--bg-card)",
                        badgeColor: "var(--text-secondary)",
                      };

              return (
              <button
                key={label}
                type="button"
                onClick={() => focusMigrationPanel(panel)}
                style={{
                  padding: "12px 14px",
                  borderRadius: 10,
                  background: palette.background,
                  border: `1px solid ${palette.border}`,
                  textAlign: "left",
                  cursor: "pointer",
                  boxShadow: isActive ? "0 0 0 1px var(--accent-primary)" : "none",
                  transform: isActive ? "translateY(-1px)" : "none",
                  transition: "all 160ms ease",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "flex-start", marginBottom: 10 }}>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.4 }}>{label}</div>
                  <span
                    style={{
                      fontSize: 10,
                      padding: "3px 7px",
                      borderRadius: 999,
                      background: palette.badgeBg,
                      color: palette.badgeColor,
                      whiteSpace: "nowrap",
                    }}
                  >
                    {isActive ? (loc === "zh" ? "当前展开" : "Open") : (loc === "zh" ? "查看" : "View")}
                  </span>
                </div>
                <div style={{ fontSize: 22, fontWeight: 700, lineHeight: 1.1, color: palette.valueColor }}>{value}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>{helper}</div>
              </button>
              );
            })}
          </div>

          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
            <button
              className="btn btn-primary btn-sm"
              style={{ gap: 6 }}
              onClick={handleExportBackup}
              disabled={exportingBackup}
            >
              <Download size={14} className={exportingBackup ? "spin" : ""} />
              {exportingBackup ? i.settings.migrationExporting : i.settings.migrationExport}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              style={{ gap: 6 }}
              onClick={handleImportBackup}
              disabled={importingBackup}
            >
              <Upload size={14} className={importingBackup ? "spin" : ""} />
              {importingBackup ? i.settings.migrationImporting : i.settings.migrationImport}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              style={{ gap: 6 }}
              disabled={repairingAll}
              onClick={handleRepairAll}
            >
              <RefreshCw size={14} className={repairingAll ? "spin" : ""} />
              {repairingAll ? i.settings.pendingImportsRepairingAll : i.settings.pendingImportsRepairAll}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              style={{ gap: 6 }}
              disabled={rescanningAll}
              onClick={handleFullRescan}
            >
              <RefreshCw size={14} className={rescanningAll ? "spin" : ""} />
              {rescanningAll ? i.settings.fullRescanning : i.settings.fullRescan}
            </button>
          </div>

          {migrationReady ? (
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
              <CheckCircle size={16} style={{ color: "var(--success)" }} />
              {i.settings.migrationCenterReady}
            </div>
          ) : (
            <div />
          )}
          <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 16 }}>
            <details
              ref={migrationPanelRefs.summary}
              open={migrationPanelsOpen.summary}
              onToggle={(event) => toggleMigrationPanel("summary", event.currentTarget.open)}
              style={{ borderRadius: 10, background: "var(--bg-input)" }}
            >
              <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
                {i.settings.importSummary}
              </summary>
              <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
                {lastImportSummary ? (
                  <>
                    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))", gap: 8 }}>
                      {[
                        [i.settings.importSummaryImportedAt, lastImportSummary.imported_at],
                        [i.settings.importSummaryData, String(lastImportSummary.db_rows_restored)],
                        [i.settings.importSummaryToolConfigs, String(lastImportSummary.tool_configs_restored)],
                        [i.settings.importSummarySkills, String(lastImportSummary.skills_restored)],
                        [i.settings.importSummaryFiles, String(lastImportSummary.full_files_restored)],
                        [i.settings.importSummaryPending, String(lastImportSummary.pending_project_files)],
                      ].map(([label, value]) => (
                        <div key={String(label)} style={{ padding: "8px 10px", borderRadius: 8, background: "var(--bg-card)" }}>
                          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{label}</div>
                          <div style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-word" }}>{value}</div>
                        </div>
                      ))}
                    </div>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
                      <div style={{ minWidth: 0 }}>
                        <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{i.settings.importSummaryBackup}</div>
                        <div style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace", color: "var(--text-secondary)", wordBreak: "break-all" }}>
                          {lastImportSummary.safety_backup_path}
                        </div>
                      </div>
                      <button
                        className="btn btn-secondary btn-sm"
                        type="button"
                        onClick={() => openInSystemWithLabel(
                          lastImportSummary.safety_backup_path,
                          loc === "zh" ? "安全备份路径" : "safety backup path"
                        )}
                        style={{ gap: 6 }}
                      >
                        <FolderOpen size={14} />
                        {i.settings.authGuideOpenPath}
                      </button>
                    </div>
                  </>
                ) : (
                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.importSummaryEmpty}</div>
                )}

                <div style={{ paddingTop: 4 }}>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 6 }}>{i.settings.fullRescan}</div>
                  {lastRescan ? (
                    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(120px, 1fr))", gap: 8 }}>
                      {[
                        [i.settings.rescanMcp, lastRescan.mcp_servers],
                        [i.settings.rescanSkills, lastRescan.skills],
                        [i.settings.rescanHooks, lastRescan.hooks],
                        [i.settings.rescanDocs, lastRescan.instruction_files],
                        [i.settings.rescanWorkflows, lastRescan.workflows],
                        [i.settings.rescanConfigRoots, lastRescan.config_roots],
                      ].map(([label, value]) => (
                        <div key={String(label)} style={{ fontSize: 12 }}>
                          <span style={{ color: "var(--text-muted)" }}>{label}: </span>
                          <span style={{ fontFamily: "'JetBrains Mono', monospace" }}>{value}</span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.migrationCenterLastRescanEmpty}</div>
                  )}
                </div>
              </div>
            </details>

            <details
              ref={migrationPanelRefs.pending}
              open={migrationPanelsOpen.pending}
              onToggle={(event) => toggleMigrationPanel("pending", event.currentTarget.open)}
              style={{ borderRadius: 10, background: "var(--bg-input)" }}
            >
              <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
                {i.settings.pendingImports}
              </summary>
              <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
                <div style={{ display: "flex", gap: 12, justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap" }}>
                  <div>
                    <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.pendingImportsDesc}</p>
                    <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>{i.settings.pendingImportsAutoMatchDesc}</p>
                  </div>
                  <button
                    className="btn btn-secondary btn-sm"
                    type="button"
                    disabled={autoMatchingPending || pendingProjectRoots.length === 0}
                    onClick={handleAutoMatchPending}
                  >
                    <RefreshCw size={14} className={autoMatchingPending ? "spin" : ""} />
                    {autoMatchingPending ? i.settings.pendingImportsAutoMatching : i.settings.pendingImportsAutoMatch}
                  </button>
                </div>
                {pendingProjectRoots.length === 0 ? (
                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.pendingImportsEmpty}</div>
                ) : (
                  pendingProjectRoots.map((item) => (
                    <div
                      key={item.project_root}
                      style={{
                        padding: "12px 14px",
                        borderRadius: 10,
                        background: "var(--bg-card)",
                        display: "flex",
                        flexDirection: "column",
                        gap: 10,
                      }}
                    >
                      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
                        <div style={{ minWidth: 0 }}>
                          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{i.settings.pendingImportsOldPath}</div>
                          <div style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>{item.project_root}</div>
                        </div>
                        <span className="badge badge-muted">{i.settings.pendingImportsFiles.replace("{count}", String(item.file_count))}</span>
                      </div>
                      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                        <input
                          className="input"
                          style={{ flex: 1, minWidth: 220, fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
                          placeholder={i.settings.pendingImportsNewPath}
                          value={remapTargets[item.project_root] || ""}
                          onChange={(e) => setRemapTargets((current) => ({ ...current, [item.project_root]: e.target.value }))}
                        />
                        <button
                          className="btn btn-secondary btn-sm"
                          type="button"
                          onClick={async () => {
                            try {
                              const picked = await invoke<string | null>("pick_folder");
                              if (picked) {
                                setRemapTargets((current) => ({ ...current, [item.project_root]: picked }));
                              }
                            } catch (e) { console.error(e); }
                          }}
                        >
                          <FolderOpen size={14} />
                          {i.settings.pendingImportsPick}
                        </button>
                        <button
                          className="btn btn-primary btn-sm"
                          type="button"
                          disabled={remappingRoot === item.project_root || !(remapTargets[item.project_root] || "").trim()}
                          onClick={async () => {
                            const targetPath = (remapTargets[item.project_root] || "").trim();
                            if (!targetPath) return;
                            setRemappingRoot(item.project_root);
                            try {
                              const restored = await invoke<number>("remap_imported_project_root", {
                                sourcePath: item.project_root,
                                targetPath,
                              });
                              await refreshMigrationState();
                              showToast(
                                "success",
                                i.settings.pendingImportsSuccess
                                  .replace("{count}", String(restored))
                                  .replace("{target}", targetPath)
                              );
                            } catch (e) {
                              showToast("error", String(e));
                            } finally {
                              setRemappingRoot((current) => current === item.project_root ? null : current);
                            }
                          }}
                        >
                          {remappingRoot === item.project_root ? i.settings.pendingImportsApplying : i.settings.pendingImportsApply}
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </details>

            <details
              ref={migrationPanelRefs.health}
              open={migrationPanelsOpen.health}
              onToggle={(event) => toggleMigrationPanel("health", event.currentTarget.open)}
              style={{ borderRadius: 10, background: "var(--bg-input)" }}
            >
              <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
                {i.settings.migrationHealth}
              </summary>
              <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start", flexWrap: "wrap" }}>
                  <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.migrationHealthDesc}</p>
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={handleRefreshMigrationHealth}
                    disabled={refreshingMigrationHealth}
                    style={{ gap: 6 }}
                  >
                    <RefreshCw size={14} className={refreshingMigrationHealth ? "spin" : ""} />
                    {refreshingMigrationHealth ? i.settings.migrationHealthRefreshing : i.settings.migrationHealthRefresh}
                  </button>
                </div>
                {toolHealthIssues.length === 0 ? (
                  <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
                    <CheckCircle size={16} style={{ color: "var(--success)" }} />
                    {i.settings.migrationHealthReady}
                  </div>
                ) : (
                  toolHealthIssues.map((report) => {
                    const tool = toolMeta.get(report.tool_id);
                    const issueBadges = [
                      !report.cli_available ? i.settings.migrationHealthCliMissing : null,
                      !report.config_dir_exists ? i.settings.migrationHealthConfigDirMissing : null,
                      !report.config_exists ? i.settings.migrationHealthConfigMissing : null,
                      !report.mcp_config_exists ? i.settings.migrationHealthMcpMissing : null,
                      !report.skills_dir_exists ? i.settings.migrationHealthSkillsMissing : null,
                    ].filter(Boolean) as string[];

                    return (
                      <div
                        key={report.tool_id}
                        style={{
                          padding: "12px 14px",
                          borderRadius: 10,
                          background: "var(--bg-card)",
                          display: "flex",
                          flexDirection: "column",
                          gap: 10,
                        }}
                      >
                        <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
                          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                            <span style={{ fontSize: 13, fontWeight: 600 }}>{report.tool_name}</span>
                            {issueBadges.map((badge) => (
                              <span key={badge} className="badge badge-muted" style={{ fontSize: 10 }}>{badge}</span>
                            ))}
                          </div>
                          {!report.cli_available && tool?.install_command && (
                            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                              <button
                                className="btn btn-ghost btn-sm"
                                onClick={() => copyText(
                                  tool.install_command,
                                  loc === "zh" ? `${report.tool_name} 安装命令` : `${report.tool_name} install command`
                                )}
                                style={{ gap: 6 }}
                              >
                                <Copy size={12} />
                                {i.settings.migrationHealthInstall}
                              </button>
                              <button
                                className="btn btn-secondary btn-sm"
                                onClick={() => runBootstrapForTool(report.tool_id, report.tool_name)}
                                disabled={bootstrappingToolId === report.tool_id}
                                style={{ gap: 6 }}
                              >
                                <FolderOpen size={12} className={bootstrappingToolId === report.tool_id ? "spin" : ""} />
                                {bootstrappingToolId === report.tool_id ? i.settings.migrationHealthBootstrapping : i.settings.migrationHealthBootstrap}
                              </button>
                            </div>
                          )}
                          {report.cli_available && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => runBootstrapForTool(report.tool_id, report.tool_name)}
                              disabled={bootstrappingToolId === report.tool_id}
                              style={{ gap: 6 }}
                            >
                              <FolderOpen size={12} className={bootstrappingToolId === report.tool_id ? "spin" : ""} />
                              {bootstrappingToolId === report.tool_id ? i.settings.migrationHealthBootstrapping : i.settings.migrationHealthBootstrap}
                            </button>
                          )}
                        </div>
                        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: 8 }}>
                          <div style={{ fontSize: 12 }}>
                            <div style={{ color: "var(--text-muted)", marginBottom: 4 }}>{i.settings.migrationHealthCli}</div>
                            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                              <span className={`badge ${report.cli_available ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
                                {report.cli_available ? i.settings.migrationHealthStatusOk : i.settings.migrationHealthStatusMissing}
                              </span>
                              <span style={{ fontFamily: "'JetBrains Mono', monospace" }}>{report.cli_command}</span>
                            </div>
                          </div>
                          <div style={{ fontSize: 12 }}>
                            <div style={{ color: "var(--text-muted)", marginBottom: 4 }}>{i.settings.migrationHealthPath}</div>
                            <div style={{ fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all", color: "var(--text-secondary)" }}>
                              {report.config_dir}
                            </div>
                            {(report.has_custom_config_dir || report.has_custom_mcp_config_path || report.has_custom_skills_dir) && (
                              <div style={{ marginTop: 6 }}>
                                <span className="badge badge-accent" style={{ fontSize: 10 }}>{i.settings.migrationHealthCustomPath}</span>
                              </div>
                            )}
                          </div>
                        </div>
                        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 8 }}>
                          {[
                            [i.settings.migrationHealthConfigDir, report.config_dir_exists, report.config_dir],
                            [i.settings.migrationHealthConfigFile, report.config_exists, report.config_path],
                            [i.settings.migrationHealthMcpConfig, report.mcp_config_exists, report.mcp_config_path],
                            [i.settings.migrationHealthSkillsDir, report.skills_dir_exists, report.skills_dir],
                          ].map(([label, ok, path]) => (
                            <div key={`${report.tool_id}-${label}`} style={{ padding: "8px 10px", borderRadius: 8, background: "var(--bg-input)" }}>
                              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                                <span className={`badge ${ok ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
                                  {ok ? i.settings.migrationHealthStatusOk : i.settings.migrationHealthStatusMissing}
                                </span>
                                <span style={{ fontSize: 12 }}>{label}</span>
                              </div>
                              <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
                                {path}
                              </div>
                            </div>
                          ))}
                        </div>
                        {!report.cli_available && tool?.install_command && (
                          <code style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                            {tool.install_command}
                          </code>
                        )}
                      </div>
                    );
                  })
                )}
              </div>
            </details>

            <details
              ref={migrationPanelRefs.auth}
              open={migrationPanelsOpen.auth}
              onToggle={(event) => toggleMigrationPanel("auth", event.currentTarget.open)}
              style={{ borderRadius: 10, background: "var(--bg-input)" }}
            >
              <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
                {i.settings.authGuide}
              </summary>
              <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
                <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.authGuideDesc}</p>
                {manualSetupReports.length === 0 ? (
                  <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
                    <CheckCircle size={16} style={{ color: "var(--success)" }} />
                    {i.settings.authGuideReady}
                  </div>
                ) : (
                  manualSetupReports.map((report) => {
                    const tool = toolMeta.get(report.tool_id);
                    const description = report.manual_setup_kind === "codex_login"
                      ? i.settings.authGuideCodexLogin
                      : report.manual_setup_kind === "gemini_api_key"
                        ? i.settings.authGuideGeminiKey
                        : report.manual_setup_kind || "";

                    return (
                      <div
                        key={`${report.tool_id}-auth`}
                        style={{
                          padding: "12px 14px",
                          borderRadius: 10,
                          background: "var(--bg-card)",
                          display: "flex",
                          flexDirection: "column",
                          gap: 10,
                        }}
                      >
                        <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
                          <div>
                            <div style={{ fontSize: 13, fontWeight: 600 }}>{report.tool_name}</div>
                            <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>{description}</div>
                          </div>
                          <span className="badge badge-muted">{report.tool_id}</span>
                        </div>
                        {report.manual_setup_path && (
                          <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
                            {report.manual_setup_path}
                          </div>
                        )}
                        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                          {report.manual_setup_command && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => copyText(
                                report.manual_setup_command || "",
                                loc === "zh" ? `${report.tool_name} 认证命令` : `${report.tool_name} auth command`
                              )}
                              style={{ gap: 6 }}
                            >
                              <Copy size={12} />
                              {i.settings.authGuideCopyCommand}
                            </button>
                          )}
                          {report.manual_setup_path && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => copyText(
                                report.manual_setup_path || "",
                                loc === "zh" ? `${report.tool_name} 路径` : `${report.tool_name} path`
                              )}
                              style={{ gap: 6 }}
                            >
                              <Copy size={12} />
                              {i.settings.authGuideCopyPath}
                            </button>
                          )}
                          {report.manual_setup_path && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => openInSystemWithLabel(
                                report.manual_setup_path || "",
                                loc === "zh" ? `${report.tool_name} 路径` : `${report.tool_name} path`
                              )}
                              style={{ gap: 6 }}
                            >
                              <FolderOpen size={12} />
                              {i.settings.authGuideOpenPath}
                            </button>
                          )}
                          <button
                            className="btn btn-secondary btn-sm"
                            onClick={() => runBootstrapForTool(report.tool_id, report.tool_name)}
                            disabled={bootstrappingToolId === report.tool_id}
                            style={{ gap: 6 }}
                          >
                            <FolderOpen size={12} className={bootstrappingToolId === report.tool_id ? "spin" : ""} />
                            {bootstrappingToolId === report.tool_id ? i.settings.migrationHealthBootstrapping : i.settings.authGuidePrepareFile}
                          </button>
                          {tool?.install_url && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => openInSystemWithLabel(
                                tool.install_url,
                                loc === "zh" ? `${report.tool_name} 说明页` : `${report.tool_name} docs`
                              )}
                              style={{ gap: 6 }}
                            >
                              <Link2 size={12} />
                              {i.settings.authGuideOpenDocs}
                            </button>
                          )}
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </details>
          </div>
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Archive size={17} style={{ color: "var(--text-secondary)" }} />
            {uiText("备份管理", "Backup Management", "バックアップ管理")}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
            {uiText(
              "集中管理 SQL 备份，支持手动创建、每小时自动备份、保留策略、重命名、删除与恢复。",
              "Manage SQL backups in one place, including manual creation, hourly automatic backups, retention, rename, delete, and restore.",
              "SQL バックアップを一元管理します。手動作成、毎時自動バックアップ、保持数、名前変更、削除、復元に対応します。",
            )}
          </p>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 12, marginBottom: 16 }}>
            <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
                {loc === "zh" ? "每小时自动备份" : "Hourly Auto Backup"}
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center" }}>
                <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {backupPreferences.auto_backup_enabled
                    ? (loc === "zh" ? "已启用，启动时会补齐缺失的小时备份" : "Enabled. Missing hourly backups are created on startup.")
                    : (loc === "zh" ? "未启用" : "Disabled")}
                </div>
                <button
                  className={`toggle ${backupPreferences.auto_backup_enabled ? "on" : "off"}`}
                  onClick={() => void handleSaveBackupPreferences({
                    ...backupPreferences,
                    auto_backup_enabled: !backupPreferences.auto_backup_enabled,
                  })}
                  disabled={savingBackupPreferences}
                >
                  <div className="toggle-knob" />
                </button>
              </div>
            </div>

            <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
                {loc === "zh" ? "保留最近备份数" : "Retention Count"}
              </div>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <input
                  className="input"
                  type="number"
                  min={1}
                  max={99}
                  value={backupPreferences.retention_count}
                  onChange={(event) => setBackupPreferencesState((current) => ({
                    ...current,
                    retention_count: Math.max(1, Number(event.target.value) || 1),
                  }))}
                  style={{ maxWidth: 110 }}
                />
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={() => void handleSaveBackupPreferences(backupPreferences)}
                  disabled={savingBackupPreferences}
                >
                  {savingBackupPreferences ? (loc === "zh" ? "保存中..." : "Saving...") : (loc === "zh" ? "保存策略" : "Save")}
                </button>
              </div>
            </div>

            <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
                {loc === "zh" ? "受管备份" : "Managed Backups"}
              </div>
              <div style={{ fontSize: 24, fontWeight: 700, marginBottom: 8 }}>{managedBackups.length}</div>
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                <button
                  className="btn btn-primary btn-sm"
                  onClick={handleCreateManagedBackup}
                  disabled={creatingManagedBackup}
                  style={{ gap: 6 }}
                >
                  <Archive size={14} className={creatingManagedBackup ? "spin" : ""} />
                  {creatingManagedBackup ? (loc === "zh" ? "创建中..." : "Creating...") : (loc === "zh" ? "立即备份" : "Create Backup")}
                </button>
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={() => void loadManagedBackups()}
                  disabled={loadingManagedBackups}
                  style={{ gap: 6 }}
                >
                  <RefreshCw size={14} className={loadingManagedBackups ? "spin" : ""} />
                  {loc === "zh" ? "刷新列表" : "Refresh"}
                </button>
              </div>
            </div>
          </div>

          {loadingManagedBackups ? (
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 12, color: "var(--text-muted)" }}>
              <RefreshCw size={14} className="spin" />
              {loc === "zh" ? "正在读取备份列表..." : "Loading backup list..."}
            </div>
          ) : managedBackups.length === 0 ? (
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {loc === "zh" ? "还没有托管备份。点击“立即备份”创建第一份 SQL 备份。" : "No managed backups yet. Create your first SQL backup from here."}
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {managedBackups.map((backup) => (
                <div
                  key={backup.path}
                  style={{
                    padding: "12px 14px",
                    borderRadius: 10,
                    background: "var(--bg-input)",
                    display: "flex",
                    justifyContent: "space-between",
                    gap: 12,
                    alignItems: "center",
                    flexWrap: "wrap",
                  }}
                >
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginBottom: 6 }}>
                      <span style={{ fontSize: 13, fontWeight: 600 }}>{backup.name}</span>
                      <span className={`badge ${backup.kind === "scheduled" ? "badge-warning" : "badge-accent"}`} style={{ fontSize: 10 }}>
                        {backup.kind === "scheduled"
                          ? (loc === "zh" ? "自动备份" : "Scheduled")
                          : (loc === "zh" ? "手动备份" : "Manual")}
                      </span>
                      <span className="badge badge-muted" style={{ fontSize: 10 }}>{formatBytes(backup.size_bytes)}</span>
                    </div>
                    <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
                      {backup.created_at.replace("T", " ").slice(0, 19)}
                    </div>
                    <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
                      {backup.path}
                    </div>
                  </div>
                  <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                    <button className="btn btn-secondary btn-sm" onClick={() => handleRenameManagedBackup(backup)}>
                      {loc === "zh" ? "重命名" : "Rename"}
                    </button>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={() => openInSystemWithLabel(backup.path, backup.name)}
                      style={{ gap: 6 }}
                    >
                      <FolderOpen size={14} />
                      {loc === "zh" ? "打开" : "Open"}
                    </button>
                    <button
                      className="btn btn-secondary btn-sm"
                      disabled={!backup.can_restore || restoringBackupPath === backup.path}
                      onClick={() => void handleRestoreManagedBackup(backup)}
                    >
                      {restoringBackupPath === backup.path
                        ? (loc === "zh" ? "恢复中..." : "Restoring...")
                        : (loc === "zh" ? "恢复" : "Restore")}
                    </button>
                    <button
                      className="btn btn-danger-ghost btn-sm"
                      disabled={deletingBackupPath === backup.path}
                      onClick={() => void handleDeleteManagedBackup(backup)}
                    >
                      {deletingBackupPath === backup.path
                        ? (loc === "zh" ? "删除中..." : "Deleting...")
                        : (loc === "zh" ? "删除" : "Delete")}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* App Update */}
        <div className="section-card">
          <div className="section-card-title">
            <Download size={17} style={{ color: "var(--text-secondary)" }} />
            {i.settings.appUpdate}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.currentVersion}</p>
                <p style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: 2 }}>{appVersion}</p>
              </div>
              <button
                className="btn btn-sm btn-secondary"
                onClick={handleCheckUpdate}
                disabled={checkingUpdate || updaterDisabledByEnv}
                style={{ gap: 6 }}
              >
                <RefreshCw size={14} className={checkingUpdate ? "spin" : ""} />
                {checkingUpdate ? i.settings.checking : i.settings.checkForUpdate}
              </button>
            </div>

            {updaterDisabledByEnv && (
              <>
                <div className="divider" />
                <div
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: 8,
                    padding: "12px 14px",
                    borderRadius: 10,
                    background: "var(--bg-input)",
                    border: "1px solid color-mix(in srgb, var(--warning) 55%, transparent)",
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <AlertCircle size={16} style={{ color: "var(--warning)" }} />
                    <span style={{ fontSize: 13, fontWeight: 600, color: "var(--warning)" }}>
                      {uiText(
                        "已被 DISABLE_AUTOUPDATER 环境变量禁用",
                        "Disabled by the DISABLE_AUTOUPDATER environment variable",
                        "DISABLE_AUTOUPDATER 環境変数により無効化されています",
                      )}
                    </span>
                  </div>
                  <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                    {uiText(
                      "当前会跳过应用更新检查与在线安装。移除该环境变量后重新启动 CCHub，即可恢复正常更新流程。",
                      "App update checks and in-app install are skipped while this environment variable is set. Remove it and restart CCHub to restore normal updates.",
                      "この環境変数が設定されている間、アプリ更新確認とアプリ内インストールはスキップされます。削除して CCHub を再起動すると通常の更新に戻ります。",
                    )}
                  </p>
                  {updaterEnvValue && (
                    <code
                      className="badge badge-accent"
                      style={{ alignSelf: "flex-start", fontSize: 11, fontFamily: "'JetBrains Mono', monospace" }}
                    >
                      {`DISABLE_AUTOUPDATER=${updaterEnvValue}`}
                    </code>
                  )}
                </div>
              </>
            )}

            {appUpdate && (
              <>
                <div className="divider" />
                {appUpdate.disabled_by_env ? (
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <AlertCircle size={16} style={{ color: "var(--warning)" }} />
                    <span style={{ fontSize: 13, color: "var(--warning)" }}>
                      {uiText(
                        "更新检查已被环境变量短路",
                        "Update checks are short-circuited by environment configuration",
                        "環境設定により更新確認は短絡されています",
                      )}
                    </span>
                  </div>
                ) : appUpdate.update_available ? (
                  <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <AlertCircle size={16} style={{ color: "var(--warning)" }} />
                      <span style={{ fontSize: 14, fontWeight: 500, color: "var(--warning)" }}>
                        {i.settings.updateAvailable}: v{appUpdate.latest_version}
                      </span>
                    </div>
                    {appUpdate.body && (
                      <p style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "pre-wrap" }}>{appUpdate.body}</p>
                    )}
                    {!appUpdate.can_install && (
                      <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
                        {loc === "zh"
                          ? "已通过发布页检测到新版本。当前构建未提供在线安装清单，将打开下载页手动更新。"
                          : "A newer release was found from GitHub. This build does not have an online install manifest, so the download page will be opened."}
                      </p>
                    )}
                    <button
                      className="btn btn-sm btn-primary"
                      onClick={handleInstallUpdate}
                      disabled={installing || updaterDisabledByEnv}
                      style={{ alignSelf: "flex-start", gap: 6 }}
                    >
                      <Download size={14} />
                      {installing
                        ? i.settings.downloading
                        : appUpdate.can_install
                          ? i.settings.installUpdate
                          : (loc === "zh" ? "前往下载页" : "Open Downloads")}
                    </button>
                  </div>
                ) : (
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <CheckCircle size={16} style={{ color: "var(--success)" }} />
                    <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>
                      {appUpdate.not_configured
                        ? i.settings.updateNotConfigured
                        : i.settings.noUpdate}
                    </span>
                  </div>
                )}
              </>
            )}

            {updateError && (
              <>
                <div className="divider" />
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <AlertCircle size={16} style={{ color: "var(--error)" }} />
                  <span style={{ fontSize: 13, color: "var(--error)" }}>{i.settings.updateFailed}: {updateError}</span>
                </div>
              </>
            )}
          </div>
        </div>

        {/* Network Proxy */}
        <div className="section-card">
          <div className="section-card-title">
            <Wifi size={17} style={{ color: "var(--text-secondary)" }} />
            {loc === "zh" ? "网络代理" : "Network Proxy"}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 12 }}>
            {loc === "zh"
              ? "设置 HTTP/HTTPS 代理地址，用于访问 GitHub 等外部服务。留空则使用系统默认网络。"
              : "Set HTTP/HTTPS proxy for accessing GitHub and external services. Leave empty for system default."}
          </p>
          <div style={{ display: "flex", gap: 8 }}>
            <input
              className="input"
              style={{ flex: 1, fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
              placeholder="http://127.0.0.1:7890"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && (async () => {
                try {
                  await invoke("set_proxy", { proxyUrl });
                  setProxySaved(true);
                  setTimeout(() => setProxySaved(false), 2000);
                  showToast("success", loc === "zh" ? "代理已保存" : "Proxy saved");
                } catch (e) { showToast("error", String(e)); }
              })()}
            />
            <button className="btn btn-primary btn-sm" style={{ gap: 5 }}
              onClick={async () => {
                try {
                  await invoke("set_proxy", { proxyUrl });
                  setProxySaved(true);
                  setTimeout(() => setProxySaved(false), 2000);
                  showToast("success", loc === "zh" ? (proxyUrl.trim() ? "代理已设置" : "代理已清除") : (proxyUrl.trim() ? "Proxy set" : "Proxy cleared"));
                } catch (e) { showToast("error", String(e)); }
              }}>
              {proxySaved ? <Check size={13} style={{ color: "var(--success)" }} /> : <Check size={13} />}
              {loc === "zh" ? "保存" : "Save"}
            </button>
          </div>
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
            {loc === "zh"
              ? "提示：也可以在 VPN 软件中开启 TUN 模式让所有流量走代理，无需在此设置。"
              : "Tip: You can also enable TUN mode in your VPN client to proxy all traffic without setting this."}
          </p>
        </div>

        {/* About */}
        <div className="section-card">
          <div className="section-card-title">
            <Info size={17} style={{ color: "var(--text-secondary)" }} />
            {i.settings.about}
          </div>
          <p style={{ fontSize: 13, color: "var(--text-secondary)" }}>{i.settings.aboutDesc}</p>
          <div style={{ display: "flex", gap: 12, marginTop: 12 }}>
            <span className="badge badge-muted">{appVersion}</span>
            <span className="badge badge-muted">{i.settings.license}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
