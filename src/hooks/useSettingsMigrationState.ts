import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type Dispatch,
  type SetStateAction,
  type SyntheticEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../components/Toast";
import type { Locale } from "../lib/i18n";
import {
  useDeleteManagedBackupMutation,
  useSaveBackupToFileMutation,
  useSetBackupPreferencesMutation,
} from "./mutations";
import {
  hasToolHealthIssue,
  type AutoRemapImportedProjectRootsResult,
  type BackupPreferences,
  type BootstrapToolEnvironmentResult,
  type CustomPath,
  type DetectedTool,
  type FullRescanResult,
  type LastImportSummary,
  type ManagedBackupFile,
  type MigrationPanelRefs,
  type MigrationPanelState,
  type PendingImportedProjectRoot,
  type RepairAllResult,
  type ToolEnvironmentReport,
} from "../types/settings";

interface UseSettingsMigrationStateOptions {
  locale: Locale;
  settingsText: Record<string, string>;
  toolsLength: number;
  loadToolsAndPaths: () => Promise<void>;
  setTools: Dispatch<SetStateAction<DetectedTool[]>>;
  setCustomPaths: Dispatch<SetStateAction<CustomPath[]>>;
  openInSystemWithLabel: (target: string, label: string) => void | Promise<void>;
}

export function useSettingsMigrationState({
  locale,
  settingsText,
  toolsLength,
  loadToolsAndPaths,
  setTools,
  setCustomPaths,
  openInSystemWithLabel,
}: UseSettingsMigrationStateOptions) {
  const saveBackupToFileMutation = useSaveBackupToFileMutation();
  const setBackupPreferencesMutation = useSetBackupPreferencesMutation();
  const deleteManagedBackupMutation = useDeleteManagedBackupMutation();
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
  const [migrationPanelsOpen, setMigrationPanelsOpen] = useState<MigrationPanelState>({
    summary: false,
    pending: false,
    health: false,
    auth: false,
  });

  const migrationPanelsInitialized = useRef(false);
  const migrationSummaryPanelRef = useRef<HTMLDetailsElement | null>(null);
  const migrationPendingPanelRef = useRef<HTMLDetailsElement | null>(null);
  const migrationHealthPanelRef = useRef<HTMLDetailsElement | null>(null);
  const migrationAuthPanelRef = useRef<HTMLDetailsElement | null>(null);
  const migrationPanelRefs = useMemo<MigrationPanelRefs>(
    () => ({
      summary: migrationSummaryPanelRef,
      pending: migrationPendingPanelRef,
      health: migrationHealthPanelRef,
      auth: migrationAuthPanelRef,
    }),
    [],
  );

  const applyPendingProjectRoots = useCallback((roots: PendingImportedProjectRoot[]) => {
    setPendingProjectRoots(roots);
    setRemapTargets((current) => {
      const next: Record<string, string> = {};
      for (const item of roots) {
        next[item.project_root] = current[item.project_root] || "";
      }
      return next;
    });
  }, []);

  const loadPendingProjectRoots = useCallback(async () => {
    try {
      const roots = await invoke<PendingImportedProjectRoot[]>("get_pending_imported_project_roots");
      applyPendingProjectRoots(roots);
    } catch (error) {
      console.warn("Failed to load pending imported project roots", error);
    }
  }, [applyPendingProjectRoots]);

  const loadLastImportSummary = useCallback(async () => {
    try {
      const summary = await invoke<LastImportSummary | null>("get_last_import_summary");
      setLastImportSummary(summary);
    } catch (error) {
      console.warn("Failed to load last import summary", error);
    }
  }, []);

  const loadManagedBackups = useCallback(async () => {
    setLoadingManagedBackups(true);
    try {
      const backups = await invoke<ManagedBackupFile[]>("list_managed_backups");
      setManagedBackups(backups);
    } catch (error) {
      console.warn("Failed to load managed backups", error);
      setManagedBackups([]);
    } finally {
      setLoadingManagedBackups(false);
    }
  }, []);

  const loadToolReports = useCallback(async () => {
    try {
      const reports = await invoke<ToolEnvironmentReport[]>("get_tool_environment_report");
      setToolReports(reports);
    } catch (error) {
      console.warn("Failed to load tool environment reports", error);
      setToolReports([]);
    }
  }, []);

  const loadBackupPreferences = useCallback(async () => {
    try {
      const preferences = await invoke<BackupPreferences>("get_backup_preferences");
      setBackupPreferencesState(preferences);
    } catch (error) {
      console.warn("Failed to load backup preferences", error);
    }
  }, []);

  const runScheduledBackupCheck = useCallback(async () => {
    try {
      const createdPath = await invoke<string | null>("run_scheduled_backup_if_needed");
      if (createdPath) {
        await loadManagedBackups();
      }
    } catch (error) {
      console.warn("Scheduled backup check failed", error);
    }
  }, [loadManagedBackups]);

  const refreshMigrationState = useCallback(async () => {
    await Promise.allSettled([
      loadToolsAndPaths(),
      loadToolReports(),
      loadPendingProjectRoots(),
      loadLastImportSummary(),
      loadManagedBackups(),
      invoke("sync_config_profiles"),
    ]);
  }, [loadLastImportSummary, loadManagedBackups, loadPendingProjectRoots, loadToolReports, loadToolsAndPaths]);

  const fetchMigrationStatusCounts = useCallback(async () => {
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
  }, [applyPendingProjectRoots]);

  const runBootstrapForTool = useCallback(
    async (toolId: string, toolName: string) => {
      setBootstrappingToolId(toolId);
      try {
        const result = await invoke<BootstrapToolEnvironmentResult>("bootstrap_tool_environment", {
          toolId,
        });
        await refreshMigrationState();
        const message = [
          settingsText.migrationHealthBootstrapSuccess
            .replace("{dirs}", String(result.created_dirs))
            .replace("{files}", String(result.created_files)),
          ...result.notes,
        ].join("；");
        showToast("success", message || `${toolName} updated`);
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setBootstrappingToolId((current) => (current === toolId ? null : current));
      }
    },
    [refreshMigrationState, settingsText],
  );

  const handleExportBackup = useCallback(async () => {
    setExportingBackup(true);
    try {
      const path = await saveBackupToFileMutation.mutateAsync();
      showToast("success", locale === "zh" ? `备份已保存到: ${path}` : `Backup saved to: ${path}`);
    } catch (error) {
      if (String(error) !== "Cancelled") showToast("error", String(error));
    } finally {
      setExportingBackup(false);
    }
  }, [locale, saveBackupToFileMutation]);

  const handleImportBackup = useCallback(async () => {
    setImportingBackup(true);
    try {
      const message = await invoke<string>("import_backup_from_file");
      await refreshMigrationState();
      showToast("success", message);
    } catch (error) {
      if (String(error) !== "Cancelled") showToast("error", String(error));
    } finally {
      setImportingBackup(false);
    }
  }, [refreshMigrationState]);

  const handleCreateManagedBackup = useCallback(async () => {
    setCreatingManagedBackup(true);
    try {
      const path = await invoke<string>("create_managed_backup", { kind: "manual" });
      await loadManagedBackups();
      showToast("success", locale === "zh" ? `备份已创建: ${path}` : `Backup created: ${path}`);
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setCreatingManagedBackup(false);
    }
  }, [loadManagedBackups, locale]);

  const handleSaveBackupPreferences = useCallback(
    async (next: BackupPreferences) => {
      setSavingBackupPreferences(true);
      try {
        const saved = await setBackupPreferencesMutation.mutateAsync({ preferences: next });
        setBackupPreferencesState(saved);
        showToast("success", locale === "zh" ? "备份策略已保存" : "Backup preferences saved");
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setSavingBackupPreferences(false);
      }
    },
    [locale, setBackupPreferencesMutation],
  );

  const handleRenameManagedBackup = useCallback(
    async (backup: ManagedBackupFile) => {
      const nextName = window.prompt(
        locale === "zh" ? "输入新的备份名称" : "Enter a new backup name",
        backup.name.replace(/\.sql$/i, ""),
      );
      if (!nextName || nextName.trim() === "" || nextName.trim() === backup.name.replace(/\.sql$/i, "")) {
        return;
      }
      try {
        await invoke("rename_managed_backup", { path: backup.path, newName: nextName.trim() });
        await loadManagedBackups();
        showToast("success", locale === "zh" ? "备份已重命名" : "Backup renamed");
      } catch (error) {
        showToast("error", String(error));
      }
    },
    [loadManagedBackups, locale],
  );

  const handleDeleteManagedBackup = useCallback(
    async (backup: ManagedBackupFile) => {
      if (!window.confirm(locale === "zh" ? `删除备份「${backup.name}」？` : `Delete backup "${backup.name}"?`)) {
        return;
      }
      setDeletingBackupPath(backup.path);
      try {
        await deleteManagedBackupMutation.mutateAsync({ path: backup.path });
        await loadManagedBackups();
        showToast("success", locale === "zh" ? "备份已删除" : "Backup deleted");
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setDeletingBackupPath((current) => (current === backup.path ? null : current));
      }
    },
    [deleteManagedBackupMutation, loadManagedBackups, locale],
  );

  const handleRestoreManagedBackup = useCallback(
    async (backup: ManagedBackupFile) => {
      if (
        !window.confirm(
          locale === "zh"
            ? `恢复备份「${backup.name}」？这会覆盖当前数据库。`
            : `Restore backup "${backup.name}"? This replaces the current database.`,
        )
      ) {
        return;
      }
      setRestoringBackupPath(backup.path);
      try {
        const message = await invoke<string>("restore_managed_backup", { path: backup.path });
        await refreshMigrationState();
        showToast("success", message);
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setRestoringBackupPath((current) => (current === backup.path ? null : current));
      }
    },
    [locale, refreshMigrationState],
  );

  const handleToggleAutoBackup = useCallback(() => {
    void handleSaveBackupPreferences({
      ...backupPreferences,
      auto_backup_enabled: !backupPreferences.auto_backup_enabled,
    });
  }, [backupPreferences, handleSaveBackupPreferences]);

  const handleBackupRetentionChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setBackupPreferencesState((current) => ({
      ...current,
      retention_count: Math.max(1, Number(event.target.value) || 1),
    }));
  }, []);

  const handleSaveCurrentBackupPreferences = useCallback(() => {
    void handleSaveBackupPreferences(backupPreferences);
  }, [backupPreferences, handleSaveBackupPreferences]);

  const handleRefreshManagedBackups = useCallback(() => {
    void loadManagedBackups();
  }, [loadManagedBackups]);

  const handleOpenSafetyBackupPath = useCallback(() => {
    if (!lastImportSummary) return;
    void openInSystemWithLabel(
      lastImportSummary.safety_backup_path,
      locale === "zh" ? "安全备份路径" : "safety backup path",
    );
  }, [lastImportSummary, locale, openInSystemWithLabel]);

  const handleFullRescan = useCallback(async () => {
    setRescanningAll(true);
    try {
      const result = await invoke<FullRescanResult>("run_full_rescan");
      setLastRescan(result);
      await refreshMigrationState();
      showToast(
        "success",
        settingsText.fullRescanSuccess
          .replace("{mcp}", String(result.mcp_servers))
          .replace("{skills}", String(result.skills))
          .replace("{hooks}", String(result.hooks))
          .replace("{docs}", String(result.instruction_files)),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setRescanningAll(false);
    }
  }, [refreshMigrationState, settingsText.fullRescanSuccess]);

  const handleRepairAll = useCallback(async () => {
    setRepairingAll(true);
    try {
      const result = await invoke<RepairAllResult>("repair_all_migration_issues");
      setLastRescan(result.rescan);
      await refreshMigrationState();
      const status = await fetchMigrationStatusCounts();
      showToast(
        "success",
        settingsText.pendingImportsRepairAllSuccess
          .replace("{roots}", String(result.remapped_roots))
          .replace("{files}", String(result.restored_project_files))
          .replace("{tools}", String(result.bootstrapped_tools))
          .replace("{pending}", String(status.pendingRoots))
          .replace("{issues}", String(status.healthIssues))
          .replace("{auth}", String(status.authGaps)),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setRepairingAll(false);
    }
  }, [fetchMigrationStatusCounts, refreshMigrationState, settingsText.pendingImportsRepairAllSuccess]);

  const handleAutoMatchPending = useCallback(async () => {
    setAutoMatchingPending(true);
    try {
      const result = await invoke<AutoRemapImportedProjectRootsResult>("auto_remap_imported_project_roots");
      await refreshMigrationState();
      const status = await fetchMigrationStatusCounts();
      showToast(
        "success",
        settingsText.pendingImportsAutoMatchSuccess
          .replace("{roots}", String(result.remapped_roots))
          .replace("{files}", String(result.restored_files))
          .replace("{skipped}", String(result.skipped_roots))
          .replace("{pending}", String(status.pendingRoots))
          .replace("{issues}", String(status.healthIssues)),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setAutoMatchingPending(false);
    }
  }, [fetchMigrationStatusCounts, refreshMigrationState, settingsText.pendingImportsAutoMatchSuccess]);

  const handleRefreshMigrationHealth = useCallback(async () => {
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
        settingsText.migrationHealthRefreshSuccess.replace(
          "{count}",
          String(reports.filter(hasToolHealthIssue).length),
        ),
      );
    } catch (error) {
      console.error(error);
      showToast("error", String(error));
    } finally {
      setRefreshingMigrationHealth(false);
    }
  }, [setCustomPaths, setTools, settingsText.migrationHealthRefreshSuccess]);

  const toggleMigrationPanel = useCallback((panel: keyof MigrationPanelState, open: boolean) => {
    setMigrationPanelsOpen((current) => ({ ...current, [panel]: open }));
  }, []);

  const focusMigrationPanel = useCallback(
    (panel: keyof MigrationPanelState) => {
      setMigrationPanelsOpen((current) => ({ ...current, [panel]: true }));
      window.setTimeout(() => {
        migrationPanelRefs[panel].current?.scrollIntoView({ behavior: "smooth", block: "start" });
      }, 0);
    },
    [migrationPanelRefs],
  );

  const handleFocusMigrationPanel = useCallback(
    (panel: string) => {
      focusMigrationPanel(panel as keyof MigrationPanelState);
    },
    [focusMigrationPanel],
  );

  const handleSummaryPanelToggle = useCallback(
    (event: SyntheticEvent<HTMLDetailsElement>) => {
      toggleMigrationPanel("summary", event.currentTarget.open);
    },
    [toggleMigrationPanel],
  );

  const handlePendingPanelToggle = useCallback(
    (event: SyntheticEvent<HTMLDetailsElement>) => {
      toggleMigrationPanel("pending", event.currentTarget.open);
    },
    [toggleMigrationPanel],
  );

  const handleHealthPanelToggle = useCallback(
    (event: SyntheticEvent<HTMLDetailsElement>) => {
      toggleMigrationPanel("health", event.currentTarget.open);
    },
    [toggleMigrationPanel],
  );

  const handleAuthPanelToggle = useCallback(
    (event: SyntheticEvent<HTMLDetailsElement>) => {
      toggleMigrationPanel("auth", event.currentTarget.open);
    },
    [toggleMigrationPanel],
  );

  const handlePendingTargetChange = useCallback((sourcePath: string, nextValue: string) => {
    setRemapTargets((current) => ({ ...current, [sourcePath]: nextValue }));
  }, []);

  const handlePickPendingTarget = useCallback(async (sourcePath: string) => {
    try {
      const picked = await invoke<string | null>("pick_folder");
      if (picked) {
        setRemapTargets((current) => ({ ...current, [sourcePath]: picked }));
      }
    } catch (error) {
      console.error(error);
    }
  }, []);

  const handleApplyPendingTarget = useCallback(
    async (sourcePath: string, rawTargetPath: string) => {
      const targetPath = rawTargetPath.trim();
      if (!targetPath) return;
      setRemappingRoot(sourcePath);
      try {
        const restored = await invoke<number>("remap_imported_project_root", {
          sourcePath,
          targetPath,
        });
        await refreshMigrationState();
        showToast(
          "success",
          settingsText.pendingImportsSuccess.replace("{count}", String(restored)).replace("{target}", targetPath),
        );
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setRemappingRoot((current) => (current === sourcePath ? null : current));
      }
    },
    [refreshMigrationState, settingsText.pendingImportsSuccess],
  );

  const toolHealthIssues = toolReports.filter(hasToolHealthIssue);
  const manualSetupReports = toolReports.filter((report) => !!report.manual_setup_kind);

  useEffect(() => {
    void loadToolReports();
    void loadPendingProjectRoots();
    void loadLastImportSummary();
    void loadManagedBackups();
    void loadBackupPreferences();
    void runScheduledBackupCheck();
  }, [
    loadBackupPreferences,
    loadLastImportSummary,
    loadManagedBackups,
    loadPendingProjectRoots,
    loadToolReports,
    runScheduledBackupCheck,
  ]);

  useEffect(() => {
    if (migrationPanelsInitialized.current) return;
    if (toolsLength === 0 && !lastImportSummary && pendingProjectRoots.length === 0 && toolReports.length === 0) return;
    setMigrationPanelsOpen({
      summary: !!lastImportSummary,
      pending: pendingProjectRoots.length > 0,
      health: toolHealthIssues.length > 0,
      auth: manualSetupReports.length > 0,
    });
    migrationPanelsInitialized.current = true;
  }, [
    lastImportSummary,
    manualSetupReports.length,
    pendingProjectRoots.length,
    toolHealthIssues.length,
    toolReports.length,
    toolsLength,
  ]);

  return {
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
  };
}
