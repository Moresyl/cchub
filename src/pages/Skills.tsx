import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Zap, Package, ExternalLink, Search, X, Monitor, Check, Trash2 } from "lucide-react";
import { t, tReplace, getLocale } from "../lib/i18n";
import type { DetectedTool, FolderNode, SkillCategory } from "../types/skills";
import { showToast } from "../components/Toast";
import SkillCard from "../components/SkillCard";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import EmptyState from "../components/states/EmptyState";
import { type ManagedAppId } from "../lib/appPreferences";
import { fetchSkillsPageData, queryKeys } from "../hooks/queries";
import {
  useBatchUpdateSkillsMutation,
  useCopySkillBetweenToolsMutation,
  useDeletePluginMutation,
  useDeleteSkillBackupMutation,
  useImportSkillFileMutation,
  useInstallPluginMutation,
  useRemoveSyncedSkillMutation,
  useRestoreSkillBackupMutation,
  useToggleSkillFileMutation,
  useUninstallSkillFileMutation,
  useWriteSkillContentMutation,
} from "../hooks/mutations";
import {
  TOOL_ICONS,
  hasSkillUpdate,
  isCommandSkill,
  isPromptSkill,
  isStandaloneSkill,
  type Plugin,
  type Skill,
  type SkillBackup,
} from "./skills/helpers";
import SkillsEditingView from "./skills/EditingView";
import SkillsExplorerView from "./skills/ExplorerView";
import SkillsDetailPanel from "./skills/DetailPanel";
import SkillsConfirmDialogs from "./skills/Dialogs";
import SkillsBackupList from "./skills/BackupList";
import PluginInstallDialog from "./skills/PluginInstallDialog";
import SkillsPageHeader from "./skills/PageHeader";
export default function Skills() {
  const queryClient = useQueryClient();
  const cachedSkillsPageData = queryClient.getQueryData<Awaited<ReturnType<typeof fetchSkillsPageData>>>(
    queryKeys.skillsPage,
  );
  const [skills, setSkills] = useState<Skill[]>(cachedSkillsPageData?.skills ?? []);
  const [plugins, setPlugins] = useState<Plugin[]>(cachedSkillsPageData?.plugins ?? []);
  const [tools, setTools] = useState<DetectedTool[]>(cachedSkillsPageData?.tools ?? []);
  const [loading, setLoading] = useState(!cachedSkillsPageData);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [activeTool, setActiveTool] = useState<string>("claude");
  const [category, setCategory] = useState<SkillCategory>("all");
  const [search, setSearch] = useState("");
  const [selectedSkill, setSelectedSkill] = useState<Skill | null>(null);
  const [skillContent, setSkillContent] = useState<string | null>(null);
  const [loadingContent, setLoadingContent] = useState(false);
  const [showExplorer, setShowExplorer] = useState(false);
  const [folderTree, setFolderTree] = useState<FolderNode | null>(null);
  const [explorerPreview, setExplorerPreview] = useState<string | null>(null);
  const [explorerFile, setExplorerFile] = useState<string | null>(null);
  const [editingSkill, setEditingSkill] = useState(false);
  const [editContent, setEditContent] = useState("");
  const [syncedSkills, setSyncedSkills] = useState<Record<string, Set<string>>>({});
  const [pendingDelete, setPendingDelete] = useState<
    { type: "skill"; item: Skill } | { type: "plugin"; item: Plugin } | null
  >(null);
  const [skillBackups, setSkillBackups] = useState<SkillBackup[]>(cachedSkillsPageData?.skillBackups ?? []);
  const [backupBusyId, setBackupBusyId] = useState<string | null>(null);
  const [pendingBackupDelete, setPendingBackupDelete] = useState<SkillBackup | null>(null);
  const [skillSyncMethod, setSkillSyncMethod] = useState<string>(cachedSkillsPageData?.skillSyncMethod ?? "copy");
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>(
    cachedSkillsPageData?.visibleApps ?? [
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
  const [checkingSkillIds, setCheckingSkillIds] = useState<string[]>([]);
  const [batchUpdating, setBatchUpdating] = useState(false);
  const [bulkTogglingSkills, setBulkTogglingSkills] = useState(false);
  const [pluginInstallOpen, setPluginInstallOpen] = useState(false);
  const [pluginSource, setPluginSource] = useState("");
  const i = t();
  const locale = getLocale();
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const importSkillFileMutation = useImportSkillFileMutation();
  const installPluginMutation = useInstallPluginMutation();
  const writeSkillContentMutation = useWriteSkillContentMutation();
  const uninstallSkillFileMutation = useUninstallSkillFileMutation();
  const toggleSkillFileMutation = useToggleSkillFileMutation();
  const deletePluginMutation = useDeletePluginMutation();
  const batchUpdateSkillsMutation = useBatchUpdateSkillsMutation();
  const removeSyncedSkillMutation = useRemoveSyncedSkillMutation();
  const copySkillBetweenToolsMutation = useCopySkillBetweenToolsMutation();
  const deleteSkillBackupMutation = useDeleteSkillBackupMutation();
  const restoreSkillBackupMutation = useRestoreSkillBackupMutation();
  const refreshSkillUpdates = useCallback(async (targetSkills: Skill[]) => {
    const ids = targetSkills.filter((skill) => skill.file_path && skill.source_url).map((skill) => skill.file_path!);
    if (ids.length === 0) return;
    setCheckingSkillIds(ids);
    try {
      const statuses = await invoke<
        Array<{
          id: string;
          latest_sha256: string | null;
          current_sha256: string | null;
          last_checked_at: number | null;
          update_available: boolean;
          error: string | null;
        }>
      >("check_skill_updates", { ids });
      setSkills((current) =>
        current.map((skill) => {
          const key = skill.file_path || skill.id;
          const next = statuses.find((item) => item.id === key);
          if (!next) return skill;
          return {
            ...skill,
            latest_sha256: next.latest_sha256,
            current_sha256: next.current_sha256 ?? skill.current_sha256,
            last_checked_at: next.last_checked_at,
          };
        }),
      );
    } catch (error) {
      console.error(error);
    } finally {
      setCheckingSkillIds([]);
    }
  }, []);
  const applySkillsPageData = useCallback(
    (data: Awaited<ReturnType<typeof fetchSkillsPageData>>) => {
      setSkills(data.skills);
      setPlugins(data.plugins);
      setTools(data.tools);
      setSkillSyncMethod(data.skillSyncMethod);
      setVisibleApps(data.visibleApps);
      setSkillBackups(data.skillBackups);
      void refreshSkillUpdates(data.skills);
      const firstInstalled = data.tools.find(
        (tool) => tool.installed && data.visibleApps.includes(tool.id as ManagedAppId),
      );
      if (firstInstalled) setActiveTool(firstInstalled.id);
    },
    [refreshSkillUpdates],
  );
  const load = useCallback(
    async (options: { force?: boolean } = {}) => {
      const { force = false } = options;
      if (!queryClient.getQueryData(queryKeys.skillsPage)) {
        setLoading(true);
      }
      setLoadError(null);
      try {
        const data = await queryClient.fetchQuery({
          queryKey: queryKeys.skillsPage,
          queryFn: fetchSkillsPageData,
          staleTime: force ? 0 : 30_000,
        });
        applySkillsPageData(data);
      } catch (e) {
        console.error(e);
        setLoadError(String(e));
      } finally {
        setLoading(false);
      }
    },
    [applySkillsPageData, queryClient],
  );
  const handleInstallPlugin = useCallback(async () => {
    const source = pluginSource.trim();
    if (!source) return;
    try {
      await installPluginMutation.mutateAsync({ sourceUrl: source });
      setPluginSource("");
      setPluginInstallOpen(false);
      await load({ force: true });
      showToast("success", locale === "zh" ? "插件安装成功" : "Plugin installed");
    } catch (error) {
      showToast("error", String(error));
    }
  }, [installPluginMutation, load, locale, pluginSource]);
  const handleImportSkill = useCallback(async () => {
    const tool = tools.find((t) => t.id === activeTool);
    if (!tool?.skills_dir) return;
    try {
      const data = await importSkillFileMutation.mutateAsync({
        targetSkillsDir: tool.skills_dir,
        method: skillSyncMethod,
      });
      applySkillsPageData(data);
      showToast("success", i.skills.importSuccess);
    } catch (e) {
      const msg = String(e);
      if (msg !== "Cancelled") showToast("error", msg);
    }
  }, [activeTool, applySkillsPageData, i.skills.importSuccess, importSkillFileMutation, skillSyncMethod, tools]);
  const viewSkill = useCallback(async (skill: Skill) => {
    setSelectedSkill(skill);
    setSkillContent(null);
    if (skill.file_path) {
      setLoadingContent(true);
      try {
        const content = await invoke<string>("read_skill_content", { filePath: skill.file_path });
        setSkillContent(content);
      } catch (e) {
        console.error(e);
        setSkillContent("Failed to load content");
      } finally {
        setLoadingContent(false);
      }
    }
  }, []);
  const openEditSkill = useCallback(async (skill: Skill) => {
    setSelectedSkill(skill);
    if (skill.file_path) {
      try {
        const content = await invoke<string>("read_skill_content", { filePath: skill.file_path });
        setSkillContent(content);
        setEditContent(content);
        setEditingSkill(true);
      } catch (e) {
        console.error(e);
      }
    }
  }, []);
  const openExplorer = useCallback(async () => {
    const tool = tools.find((t) => t.id === activeTool);
    if (!tool) return;
    setShowExplorer(true);
    setExplorerPreview(null);
    setExplorerFile(null);
    try {
      const tree = await invoke<FolderNode>("get_skill_folder_tree", { baseDir: tool.skills_dir });
      setFolderTree(tree);
    } catch {
      setFolderTree(null);
    }
  }, [activeTool, tools]);
  const previewExplorerFile = useCallback(async (path: string) => {
    setExplorerFile(path);
    try {
      const content = await invoke<string>("read_skill_content", { filePath: path });
      setExplorerPreview(content);
    } catch {
      setExplorerPreview("Failed to load file");
    }
  }, []);
  const handleSaveSkill = useCallback(async () => {
    if (!selectedSkill?.file_path) return;
    try {
      await writeSkillContentMutation.mutateAsync({ filePath: selectedSkill.file_path, content: editContent });
      setSkillContent(editContent);
      setEditingSkill(false);
    } catch (e) {
      console.error(e);
    }
  }, [editContent, selectedSkill, writeSkillContentMutation]);
  const handleDeleteSkill = useCallback((skill: Skill) => {
    if (!skill.file_path) return;
    setPendingDelete({ type: "skill", item: skill });
  }, []);
  const doDeleteSkill = useCallback(
    async (skill: Skill) => {
      if (!skill.file_path) return;
      try {
        const data = await uninstallSkillFileMutation.mutateAsync({ path: skill.file_path });
        if (selectedSkill?.id === skill.id) {
          setSelectedSkill(null);
          setSkillContent(null);
          setEditingSkill(false);
        }
        applySkillsPageData(data);
        showToast("success", locale === "zh" ? "技能已删除，并已自动备份" : "Skill deleted and backed up");
      } catch (e) {
        console.error(e);
      }
    },
    [applySkillsPageData, locale, selectedSkill, uninstallSkillFileMutation],
  );
  const handleToggleSkill = useCallback(
    async (skill: Skill) => {
      if (!skill.file_path) return;
      const isDisabled = skill.file_path.endsWith(".disabled");
      try {
        const data = await toggleSkillFileMutation.mutateAsync({ filePath: skill.file_path, enabled: isDisabled });
        applySkillsPageData(data);
      } catch (e) {
        console.error(e);
      }
    },
    [applySkillsPageData, toggleSkillFileMutation],
  );
  const handleDeletePlugin = useCallback((plugin: Plugin) => {
    setPendingDelete({ type: "plugin", item: plugin });
  }, []);
  const doDeletePlugin = useCallback(
    async (plugin: Plugin) => {
      try {
        const data = await deletePluginMutation.mutateAsync({ pluginId: plugin.id });
        applySkillsPageData(data);
      } catch (e) {
        console.error(e);
      }
    },
    [applySkillsPageData, deletePluginMutation],
  );
  const handleViewSkill = useCallback(
    (skill: Skill) => {
      void viewSkill(skill);
    },
    [viewSkill],
  );
  const handleOpenEditSkill = useCallback(
    (skill: Skill) => {
      void openEditSkill(skill);
    },
    [openEditSkill],
  );
  const handleBatchUpdateSkills = useCallback(async () => {
    const ids = skills
      .filter((skill) => (skill.tool_id ? skill.tool_id === activeTool : activeTool === "claude"))
      .filter((skill) => skill.file_path && hasSkillUpdate(skill))
      .map((skill) => skill.file_path!);
    if (ids.length === 0) return;
    setBatchUpdating(true);
    try {
      const { updated, data } = await batchUpdateSkillsMutation.mutateAsync({ ids });
      applySkillsPageData(data);
      showToast("success", locale === "zh" ? `已更新 ${updated} 个技能` : `Updated ${updated} skill(s)`);
    } catch (error) {
      console.error(error);
      showToast("error", String(error));
    } finally {
      setBatchUpdating(false);
    }
  }, [activeTool, applySkillsPageData, batchUpdateSkillsMutation, locale, skills]);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    if (visibleApps.includes(activeTool as ManagedAppId)) return;
    const nextTool = tools.find((tool) => tool.installed && visibleApps.includes(tool.id as ManagedAppId));
    if (nextTool) setActiveTool(nextTool.id);
  }, [activeTool, tools, visibleApps]);
  useEffect(() => {
    const handleEscape = () => {
      if (search.trim()) {
        setSearch("");
        return;
      }
      if (editingSkill) {
        setEditingSkill(false);
        return;
      }
      if (showExplorer) {
        setShowExplorer(false);
        return;
      }
      if (selectedSkill) {
        setSelectedSkill(null);
        setSkillContent(null);
      }
    };
    window.addEventListener("cchub-shortcut-escape", handleEscape);
    return () => window.removeEventListener("cchub-shortcut-escape", handleEscape);
  }, [editingSkill, search, selectedSkill, showExplorer]);
  useEffect(() => {
    const handleSearchShortcut = () => {
      if (editingSkill || showExplorer) return;
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    };
    window.addEventListener("cchub-shortcut-search", handleSearchShortcut);
    return () => window.removeEventListener("cchub-shortcut-search", handleSearchShortcut);
  }, [editingSkill, showExplorer]);
  const visibleSkills = useMemo(
    () =>
      skills.filter((skill) => {
        if (skill.tool_id) return skill.tool_id === activeTool;
        return activeTool === "claude";
      }),
    [skills, activeTool],
  );
  const visiblePlugins = useMemo(() => (activeTool === "claude" ? plugins : []), [activeTool, plugins]);
  const updatableVisibleSkillCount = useMemo(() => visibleSkills.filter(hasSkillUpdate).length, [visibleSkills]);
  const handleToggleAllSkills = useCallback(
    async (enabled: boolean) => {
      const targets = visibleSkills.filter((skill) => {
        if (!skill.file_path) return false;
        return enabled ? skill.file_path.endsWith(".disabled") : !skill.file_path.endsWith(".disabled");
      });
      if (targets.length === 0 || bulkTogglingSkills) return;
      setBulkTogglingSkills(true);
      let failed = 0;
      for (const skill of targets) {
        try {
          await toggleSkillFileMutation.mutateAsync({ filePath: skill.file_path!, enabled });
        } catch {
          failed += 1;
        }
      }
      await load({ force: true });
      setBulkTogglingSkills(false);
      if (failed > 0) {
        showToast("error", locale === "zh" ? `${failed} 个技能切换失败` : `${failed} skill(s) failed to toggle`);
      } else {
        showToast(
          "success",
          enabled
            ? locale === "zh"
              ? "已全部启用"
              : "All skills enabled"
            : locale === "zh"
              ? "已全部禁用"
              : "All skills disabled",
        );
      }
    },
    [bulkTogglingSkills, load, locale, toggleSkillFileMutation, visibleSkills],
  );
  // 搜索框输入时让过滤跑在低优先级，避免 setState 触发的同步过滤+渲染阻塞键盘输入。
  // useDeferredValue 让 React 在主线程压力大时延后非紧急更新，输入框 echo 始终流畅。
  const deferredSearch = useDeferredValue(search);
  const filteredSkills = useMemo(() => {
    const q = deferredSearch.trim().toLowerCase();
    return visibleSkills.filter((s) => {
      if (q) {
        if (
          !s.name.toLowerCase().includes(q) &&
          !(s.description || "").toLowerCase().includes(q) &&
          !(s.trigger_command || "").toLowerCase().includes(q)
        )
          return false;
      }
      switch (category) {
        case "skill":
          return isStandaloneSkill(s);
        case "prompt":
          return isPromptSkill(s);
        case "command":
          return isCommandSkill(s);
        case "plugin":
          return false;
        default:
          return true; // "all"
      }
    });
  }, [visibleSkills, deferredSearch, category]);
  const filteredPlugins = useMemo(() => {
    if (category !== "all" && category !== "plugin") return [];
    const q = deferredSearch.trim().toLowerCase();
    return visiblePlugins.filter((p) => {
      if (q) {
        if (!p.name.toLowerCase().includes(q) && !(p.description || "").toLowerCase().includes(q)) return false;
      }
      return true;
    });
  }, [visiblePlugins, deferredSearch, category]);
  const visibleToolIds = new Set(visibleApps);
  const visibleTools = tools.filter((tool) => visibleToolIds.has(tool.id as ManagedAppId));
  const installedTools = visibleTools.filter((t) => t.installed);
  if (loading) {
    return <LoadingState label={i.skills.loading} />;
  }
  if (loadError) {
    return (
      <ErrorState
        title={locale === "zh" ? "技能加载失败" : "Failed to load skills"}
        message={loadError}
        retryLabel={i.common.refresh}
        onRetry={() => {
          void load({ force: true });
        }}
      />
    );
  }
  const hasEditChanges = editContent !== (skillContent || "");
  if (editingSkill && selectedSkill) {
    return (
      <SkillsEditingView
        selectedSkill={selectedSkill}
        skillContent={skillContent}
        editContent={editContent}
        setEditContent={setEditContent}
        hasEditChanges={hasEditChanges}
        handleSaveSkill={handleSaveSkill}
        setEditingSkill={setEditingSkill}
        locale={locale}
        i={i}
      />
    );
  }
  if (showExplorer) {
    return (
      <SkillsExplorerView
        folderTree={folderTree}
        explorerPreview={explorerPreview}
        explorerFile={explorerFile}
        previewExplorerFile={previewExplorerFile}
        setShowExplorer={setShowExplorer}
        locale={locale}
        i={i}
      />
    );
  }
  const catTabs: { key: SkillCategory; label: string; count: number }[] = [
    { key: "all", label: i.skills.categoryAll, count: visibleSkills.length + visiblePlugins.length },
    { key: "skill", label: i.skills.categorySkills, count: visibleSkills.filter(isStandaloneSkill).length },
    { key: "prompt", label: i.skills.categoryPrompts, count: visibleSkills.filter(isPromptSkill).length },
    { key: "command", label: i.skills.categoryCommands, count: visibleSkills.filter(isCommandSkill).length },
    { key: "plugin", label: i.skills.categoryPlugins, count: visiblePlugins.length },
  ];
  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <SkillsPageHeader
        title={i.skills.title}
        subtitle={
          <>
            {tReplace(i.skills.totalSkills, { count: visibleSkills.length + visiblePlugins.length })}
            {installedTools.length > 0 && ` · ${tReplace(i.skills.toolCount, { count: installedTools.length })}`}
          </>
        }
        updateLabel={
          locale === "zh" ? `全部更新 (${updatableVisibleSkillCount})` : `Update All (${updatableVisibleSkillCount})`
        }
        importLabel={i.skills.importSkill}
        installPluginLabel={locale === "zh" ? "安装插件" : "Install plugin"}
        exploreLabel={i.skills.explore}
        refreshLabel={i.common.refresh}
        batchUpdating={batchUpdating}
        canBatchUpdate={updatableVisibleSkillCount > 0 && !batchUpdating}
        onBatchUpdate={() => void handleBatchUpdateSkills()}
        onImport={handleImportSkill}
        onInstallPlugin={() => setPluginInstallOpen(true)}
        onExplore={openExplorer}
        onRefresh={() => void load({ force: true })}
      />
      <SkillsBackupList
        skillBackups={skillBackups}
        backupBusyId={backupBusyId}
        setBackupBusyId={setBackupBusyId}
        setPendingBackupDelete={setPendingBackupDelete}
        restoreSkillBackupMutation={restoreSkillBackupMutation}
        applySkillsPageData={applySkillsPageData}
        locale={locale}
      />
      <PluginInstallDialog
        isOpen={pluginInstallOpen}
        source={pluginSource}
        setSource={setPluginSource}
        busy={installPluginMutation.isPending}
        locale={locale}
        onConfirm={() => void handleInstallPlugin()}
        onCancel={() => {
          if (!installPluginMutation.isPending) setPluginInstallOpen(false);
        }}
      />
      {/* Tool Selector */}
      {visibleTools.length > 0 && (
        <div style={{ display: "flex", gap: 8, marginBottom: 16, flexWrap: "wrap", alignItems: "center" }}>
          {visibleTools.map((tool) => {
            const Icon = TOOL_ICONS[tool.id] || Monitor;
            const isActive = activeTool === tool.id;
            return (
              <div key={tool.id} style={{ position: "relative" }}>
                <button
                  className={`btn btn-sm ${isActive ? "btn-primary" : tool.installed ? "btn-secondary" : "btn-ghost"}`}
                  onClick={() => {
                    if (!tool.installed) return;
                    setActiveTool(tool.id);
                    setSelectedSkill(null);
                    setSkillContent(null);
                    setEditingSkill(false);
                  }}
                  style={{ gap: 6, opacity: tool.installed ? 1 : 0.5, cursor: tool.installed ? "pointer" : "default" }}
                  title={
                    tool.installed ? tool.name : locale === "zh" ? `${tool.name} 未安装` : `${tool.name} not installed`
                  }
                >
                  <Icon size={14} />
                  {tool.name}
                  {!tool.installed && (
                    <span
                      style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--danger)", flexShrink: 0 }}
                    />
                  )}
                </button>
              </div>
            );
          })}
          {/* Uninstalled tool hints */}
          {visibleTools.filter((t) => !t.installed).length > 0 && (
            <span style={{ fontSize: 11, color: "var(--text-muted)", marginLeft: 4 }}>
              {locale === "zh" ? "红点 = 未安装" : "red dot = not installed"}
            </span>
          )}
        </div>
      )}
      {visibleSkills.length > 0 && (
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12, flexWrap: "wrap" }}>
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {locale === "zh"
              ? `${visibleSkills.length} 个技能 · ${activeTool}`
              : `${visibleSkills.length} skills · ${activeTool}`}
          </span>
          <button
            className="btn btn-secondary btn-xs"
            type="button"
            onClick={() => void handleToggleAllSkills(true)}
            disabled={
              bulkTogglingSkills ||
              visibleSkills.every((skill) => !skill.file_path || !skill.file_path.endsWith(".disabled"))
            }
          >
            {bulkTogglingSkills ? <div className="spinner" style={{ width: 11, height: 11 }} /> : <Check size={12} />}
            {locale === "zh" ? "全部启用" : "Enable all"}
          </button>
          <button
            className="btn btn-secondary btn-xs"
            type="button"
            onClick={() => void handleToggleAllSkills(false)}
            disabled={
              bulkTogglingSkills ||
              visibleSkills.every((skill) => !skill.file_path || skill.file_path.endsWith(".disabled"))
            }
          >
            {locale === "zh" ? "全部禁用" : "Disable all"}
          </button>
        </div>
      )}
      {/* Search + Category Tabs */}
      <div style={{ display: "flex", gap: 16, marginBottom: 20, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1, maxWidth: 320 }}>
          <Search
            size={15}
            style={{
              position: "absolute",
              left: 14,
              top: "50%",
              transform: "translateY(-50%)",
              color: "var(--text-muted)",
            }}
          />
          <input
            ref={searchInputRef}
            className="input"
            style={{ paddingLeft: 40 }}
            placeholder={i.skills.searchPlaceholder}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          {search && (
            <button
              className="btn btn-ghost btn-icon-sm"
              style={{ position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)" }}
              onClick={() => setSearch("")}
            >
              <X size={14} />
            </button>
          )}
        </div>
        <div className="tab-bar" style={{ flexShrink: 0, overflow: "auto" }}>
          {catTabs.map((cat) => (
            <button
              key={cat.key}
              className={`tab-item ${category === cat.key ? "active" : ""}`}
              onClick={() => {
                setCategory(cat.key);
                setSelectedSkill(null);
                setSkillContent(null);
              }}
              disabled={cat.count === 0 && cat.key !== "all"}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 5,
                opacity: cat.count === 0 && cat.key !== "all" ? 0.4 : 1,
              }}
            >
              {cat.label}
              <span style={{ fontSize: 11, opacity: 0.7 }}>({cat.count})</span>
            </button>
          ))}
        </div>
      </div>
      {/* Content Area */}
      <div style={{ flex: 1, minHeight: 0, display: "flex", gap: 24 }}>
        {/* List */}
        <div
          style={{ flex: selectedSkill ? 1.2 : 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 6 }}
        >
          {/* Skills */}
          {filteredSkills.length > 0 && (
            <div className="stagger">
              {category === "all" && filteredPlugins.length > 0 && (
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6, paddingLeft: 4 }}>
                  <Zap size={14} style={{ color: "var(--warning)" }} />
                  <span
                    style={{
                      fontSize: 12,
                      fontWeight: 600,
                      color: "var(--text-muted)",
                      textTransform: "uppercase",
                      letterSpacing: "0.05em",
                    }}
                  >
                    {i.skills.categorySkills} ({filteredSkills.length})
                  </span>
                </div>
              )}
              {filteredSkills.map((skill) => (
                <SkillCard
                  key={skill.id}
                  skill={skill}
                  selected={selectedSkill?.id === skill.id}
                  disabledLabel={locale === "zh" ? "已禁用" : "Disabled"}
                  editTitle={locale === "zh" ? "编辑" : "Edit"}
                  deleteTitle={locale === "zh" ? "删除" : "Delete"}
                  enableTitle={locale === "zh" ? "启用" : "Enable"}
                  disableTitle={locale === "zh" ? "禁用" : "Disable"}
                  updateAvailable={hasSkillUpdate(skill)}
                  updateLabel={locale === "zh" ? "有更新" : "Update"}
                  latestLabel={locale === "zh" ? "最新" : "Latest"}
                  checkingUpdates={checkingSkillIds.includes(skill.file_path || skill.id)}
                  onView={handleViewSkill}
                  onToggle={handleToggleSkill}
                  onEdit={handleOpenEditSkill}
                  onDelete={handleDeleteSkill}
                />
              ))}
            </div>
          )}
          {/* Plugins */}
          {filteredPlugins.length > 0 && (
            <div className="stagger">
              {category === "all" && filteredSkills.length > 0 && (
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    marginBottom: 6,
                    marginTop: 8,
                    paddingLeft: 4,
                  }}
                >
                  <Package size={14} style={{ color: "var(--success)" }} />
                  <span
                    style={{
                      fontSize: 12,
                      fontWeight: 600,
                      color: "var(--text-muted)",
                      textTransform: "uppercase",
                      letterSpacing: "0.05em",
                    }}
                  >
                    {i.skills.categoryPlugins} ({filteredPlugins.length})
                  </span>
                </div>
              )}
              {filteredPlugins.map((plugin) => (
                <div key={plugin.id} className="card card-hover" style={{ padding: "14px 18px", marginBottom: 6 }}>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 12, minWidth: 0, flex: 1 }}>
                      <div
                        className="icon-box"
                        style={{ background: "var(--success-subtle)", width: 34, height: 34, borderRadius: 6 }}
                      >
                        <Package size={15} style={{ color: "var(--success)" }} />
                      </div>
                      <div style={{ minWidth: 0, flex: 1 }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                          <span style={{ fontSize: 13, fontWeight: 600 }}>{plugin.name}</span>
                          {plugin.version && (
                            <span style={{ fontSize: 11, color: "var(--text-muted)" }}>v{plugin.version}</span>
                          )}
                        </div>
                        {plugin.description && (
                          <p
                            style={{
                              fontSize: 12,
                              color: "var(--text-muted)",
                              marginTop: 2,
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                            }}
                          >
                            {plugin.description}
                          </p>
                        )}
                      </div>
                    </div>
                    <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
                      {plugin.source_url && (
                        <span className="badge badge-accent" style={{ gap: 5 }}>
                          <ExternalLink size={11} />
                          GitHub
                        </span>
                      )}
                      <button
                        className="btn btn-danger-ghost btn-icon-sm"
                        onClick={() => handleDeletePlugin(plugin)}
                        title={locale === "zh" ? "删除" : "Delete"}
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
          {/* Empty */}
          {filteredSkills.length === 0 && filteredPlugins.length === 0 && (
            <EmptyState
              icon={<Zap size={28} style={{ color: "var(--text-muted)" }} />}
              title={
                search
                  ? locale === "zh"
                    ? "未找到匹配结果"
                    : "No results found"
                  : category === "plugin"
                    ? i.skills.noPlugins
                    : i.skills.noSkills
              }
              description={
                search
                  ? locale === "zh"
                    ? "尝试其他关键词"
                    : "Try different keywords"
                  : category === "plugin"
                    ? i.skills.noPluginsTip
                    : i.skills.noSkillsTip
              }
            />
          )}
        </div>
        {/* Detail Panel */}
        {selectedSkill && (
          <SkillsDetailPanel
            selectedSkill={selectedSkill}
            skillContent={skillContent}
            loadingContent={loadingContent}
            editingSkill={editingSkill}
            syncedSkills={syncedSkills}
            setSyncedSkills={setSyncedSkills}
            tools={tools}
            skillSyncMethod={skillSyncMethod}
            handleToggleSkill={handleToggleSkill}
            handleDeleteSkill={handleDeleteSkill}
            setEditingSkill={setEditingSkill}
            setSelectedSkill={setSelectedSkill}
            copySkillBetweenToolsMutation={copySkillBetweenToolsMutation}
            removeSyncedSkillMutation={removeSyncedSkillMutation}
            locale={locale}
            i={i}
          />
        )}
      </div>
      <SkillsConfirmDialogs
        pendingDelete={pendingDelete}
        setPendingDelete={setPendingDelete}
        pendingBackupDelete={pendingBackupDelete}
        setPendingBackupDelete={setPendingBackupDelete}
        setBackupBusyId={setBackupBusyId}
        doDeletePlugin={doDeletePlugin}
        doDeleteSkill={doDeleteSkill}
        deleteSkillBackupMutation={deleteSkillBackupMutation}
        applySkillsPageData={applySkillsPageData}
        locale={locale}
      />
    </div>
  );
}
