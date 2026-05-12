/* eslint-disable react-hooks/exhaustive-deps */
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { getLocale, t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import { type FeaturedSkillBundle } from "../components/FeaturedSkillBundleCard";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import type { DetectedTool } from "../types/skills";
import { type ManagedAppId } from "../lib/appPreferences";
import { useMarketplaceFilters, type McpCategory, type SkillCategory } from "../stores/marketplaceFilters";
import {
  fetchMarketplaceCatalogPage,
  fetchMarketplaceLocalData,
  fetchMarketplaceSearchPage,
  queryKeys,
} from "../hooks/queries";
import { useUpdateMcpServerConfigMutation } from "../hooks/mutations";

import {
  dedupByName,
  type InstalledMcpServer,
  type InstalledSkillRecord,
  type RegistryEntry,
  type SkillEntry,
} from "./marketplace/helpers";
import {
  buildFeaturedBundles,
  buildRecommendedRepos,
  computeBundleInstalledCount,
  performInstallBundle,
} from "./marketplace/bundles";
import { McpEditView, SkillEditView } from "./marketplace/EditViews";
import { McpGrid, SkillsGrid } from "./marketplace/Grids";
import { MarketplaceHeader } from "./marketplace/Header";
import {
  loadMcpPage,
  performInstallMcp,
  performInstallSkill,
  performLoadCustomSource,
  performLoadRecommendedRepo,
  performRefreshInstalledMcpDetails,
  performSaveMcpConfig,
  performSaveSkillContent,
  performUninstallMcp,
  performUninstallSkill,
} from "./marketplace/handlers";
import { performMarketplaceLoadAll } from "./marketplace/loadAll";
import {
  CustomSourceModal,
  EnvModal,
  McpPreviewModal,
  SkillPreviewModal,
  UninstallSkillDialog,
} from "./marketplace/Modals";

export default function Marketplace() {
  const queryClient = useQueryClient();
  const cachedLocalData = queryClient.getQueryData<Awaited<ReturnType<typeof fetchMarketplaceLocalData>>>(
    queryKeys.marketplaceLocal,
  );
  const cachedCatalogData = queryClient.getQueryData<Awaited<ReturnType<typeof fetchMarketplaceCatalogPage>>>(
    queryKeys.marketplaceCatalog(),
  );
  const cachedSearchData = queryClient.getQueryData<Awaited<ReturnType<typeof fetchMarketplaceSearchPage>>>(
    queryKeys.marketplaceSearch("mcp server"),
  );
  // scan_mcp_servers / scan_skills 会按工具返回每条记录，同名 skill/server 装到
  // 多个工具时会出现多条 —— 列表视图只需要每个 name 一条，per-tool 的"已安装"
  // 状态另由 installedIdsByTool / installedSkillsByTool 驱动。
  const initialLocalEntries: RegistryEntry[] = dedupByName(cachedLocalData?.servers ?? []).map((s) => ({
    id: s.id,
    name: s.name,
    description: s.command
      ? `${s.command} ${(() => {
          try {
            return JSON.parse(s.args || "[]").join(" ");
          } catch {
            return "";
          }
        })()}`
      : "",
    category: s.source || "local",
    install_type: "local",
    package_name: s.package_name,
    github_url: null,
    command: s.command || "",
    args: (() => {
      try {
        return JSON.parse(s.args || "[]");
      } catch {
        return [];
      }
    })(),
    env_keys: (() => {
      try {
        return Object.keys(JSON.parse(s.env || "{}"));
      } catch {
        return [];
      }
    })(),
    source: "local",
  }));
  const initialLocalSkills: SkillEntry[] = dedupByName(cachedLocalData?.installedSkills ?? []).map((s) => ({
    id: `local-${s.name}`,
    name: s.name,
    description: s.description || (s.trigger_command ? `/${s.trigger_command}` : ""),
    description_zh: null,
    category: s.plugin_id ? "plugin" : "local",
    author: s.plugin_id || null,
    github_url: null,
    cover_url: null,
    tags: s.trigger_command ? [s.trigger_command] : [],
    content: "",
    file_path: s.file_path,
  }));
  const initialSkillEntries: SkillEntry[] = (() => {
    const localNames = new Set(initialLocalSkills.map((skill) => skill.name.toLowerCase()));
    const marketSkills = cachedCatalogData?.skills ?? [];
    return [...initialLocalSkills, ...marketSkills.filter((skill) => !localNames.has(skill.name.toLowerCase()))];
  })();
  const tab = useMarketplaceFilters((state) => state.tab);
  const setTab = useMarketplaceFilters((state) => state.setTab);
  const updateMcpServerConfigMutation = useUpdateMcpServerConfigMutation();
  const [entries, setEntries] = useState<RegistryEntry[]>([
    ...initialLocalEntries,
    ...(cachedSearchData?.entries.filter((entry) => !initialLocalEntries.some((item) => item.id === entry.id)) ?? []),
  ]);
  const [skillEntries, setSkillEntries] = useState<SkillEntry[]>(initialSkillEntries);
  const [loading, setLoading] = useState(!(cachedLocalData || cachedCatalogData || cachedSearchData));
  const [loadError, setLoadError] = useState<string | null>(null);
  const search = useMarketplaceFilters((state) => state.search);
  const setSearch = useMarketplaceFilters((state) => state.setSearch);
  const mcpCategory = useMarketplaceFilters((state) => state.mcpCategory);
  const setMcpCategory = useMarketplaceFilters((state) => state.setMcpCategory);
  const skillCategory = useMarketplaceFilters((state) => state.skillCategory);
  const setSkillCategory = useMarketplaceFilters((state) => state.setSkillCategory);
  const activeTool = useMarketplaceFilters((state) => state.activeTool);
  const setActiveTool = useMarketplaceFilters((state) => state.setActiveTool);
  const [tools, setTools] = useState<DetectedTool[]>([]);
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>([
    "claude",
    "codex",
    "gemini",
    "opencode",
    "openclaw",
    "hermes",
  ]);
  // Per-tool installed indices: { toolId -> Set of server names/ids } and { toolId -> Set of skill names (lowercase) }
  // Initial Claude state derived from `cachedLocalData` so the UI doesn't flash empty.
  const [installedIdsByTool, setInstalledIdsByTool] = useState<Record<string, Set<string>>>(() => ({
    claude: new Set(cachedLocalData?.servers.flatMap((server) => [server.id, server.name]) ?? []),
  }));
  const [installedSkillsByTool, setInstalledSkillsByTool] = useState<Record<string, Set<string>>>(() => ({
    claude: new Set(cachedLocalData?.installedSkills.map((skill) => skill.name.toLowerCase()) ?? []),
  }));
  const [installing, setInstalling] = useState<string | null>(null);
  const [showEnvModal, setShowEnvModal] = useState<RegistryEntry | null>(null);
  const [envValues, setEnvValues] = useState<Record<string, string>>({});
  const showTranslation = true;
  const [showCustomSource, setShowCustomSource] = useState(false);
  const [customUrl, setCustomUrl] = useState("");
  const [loadingCustom, setLoadingCustom] = useState(false);
  const [customSources, setCustomSources] = useState<{ url: string; count: number; skillIds: string[] }[]>([]);
  const [loadingRepo, setLoadingRepo] = useState<string | null>(null);
  const [mcpPage, setMcpPage] = useState(0);
  const [mcpTotal, setMcpTotal] = useState(cachedSearchData?.total ?? 0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [installedMcpDetails, setInstalledMcpDetails] = useState<InstalledMcpServer[]>(cachedLocalData?.servers ?? []);
  const [previewMcp, setPreviewMcp] = useState<RegistryEntry | null>(null);
  const [previewSkill, setPreviewSkill] = useState<SkillEntry | null>(null);
  const [editingSkill, setEditingSkill] = useState<SkillEntry | null>(null);
  const [skillContent, setSkillContent] = useState("");
  const [editSkillContent, setEditSkillContent] = useState("");
  const [editingMcp, setEditingMcp] = useState<InstalledMcpServer | null>(null);
  const [editCommand, setEditCommand] = useState("");
  const [editArgs, setEditArgs] = useState("");
  const [editEnv, setEditEnv] = useState("");
  const [pendingUninstall, setPendingUninstall] = useState<SkillEntry | null>(null);
  const [installingBundle, setInstallingBundle] = useState<string | null>(null);
  const [bundleProgress, setBundleProgress] = useState<Record<string, number>>({});
  const i = t();
  const locale = getLocale();
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const localeText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );

  // Stable empty set so memos don't re-fire when active tool has no installs
  const emptySetRef = useRef<Set<string>>(new Set<string>());
  const currentToolInstalledIds = installedIdsByTool[activeTool] ?? emptySetRef.current;
  const currentToolInstalledSkills = installedSkillsByTool[activeTool] ?? emptySetRef.current;
  const visibleToolIds = useMemo(() => new Set(visibleApps), [visibleApps]);
  const visibleTools = useMemo(
    () => tools.filter((tool) => visibleToolIds.has(tool.id as ManagedAppId)),
    [tools, visibleToolIds],
  );

  // Helper: rebuild per-tool installed sets after a fresh `scan_skills` run.
  const rebuildSkillsByTool = useCallback((records: InstalledSkillRecord[]) => {
    const map: Record<string, Set<string>> = {};
    for (const s of records) {
      const tid = s.tool_id ?? "claude";
      if (!map[tid]) map[tid] = new Set();
      map[tid].add(s.name.toLowerCase());
    }
    return map;
  }, []);

  // Helper: probe each MCP server name across all 6 tools so the per-tool
  // map covers Claude AND any tool the server has been synced to.
  const rebuildMcpByTool = useCallback(async (serverNames: string[]) => {
    if (serverNames.length === 0) return {} as Record<string, Set<string>>;
    const results = await Promise.all(
      serverNames.map(async (name) => {
        try {
          const map = await invoke<Record<string, boolean>>("check_mcp_server_in_tools", { serverName: name });
          return { name, map };
        } catch {
          return { name, map: { claude: true } as Record<string, boolean> };
        }
      }),
    );
    const out: Record<string, Set<string>> = {};
    for (const { name, map } of results) {
      for (const [toolId, present] of Object.entries(map)) {
        if (!present) continue;
        if (!out[toolId]) out[toolId] = new Set();
        out[toolId].add(name);
      }
    }
    return out;
  }, []);

  const loadAll = useCallback(
    (options: { force?: boolean } = {}) =>
      performMarketplaceLoadAll(queryClient, options, {
        setLoading,
        setLoadError,
        setEntries,
        setSkillEntries,
        setInstalledMcpDetails,
        setInstalledIdsByTool,
        setInstalledSkillsByTool,
        setTools,
        setVisibleApps,
        setActiveTool,
        setMcpTotal,
        setMcpPage,
        rebuildSkillsByTool,
        rebuildMcpByTool,
      }),
    [queryClient, rebuildMcpByTool, rebuildSkillsByTool],
  );

  useEffect(() => {
    void loadAll();
  }, [loadAll]);
  useEffect(() => {
    const handleSearchShortcut = () => {
      if (editingSkill || editingMcp) return;
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    };
    window.addEventListener("cchub-shortcut-search", handleSearchShortcut);
    return () => window.removeEventListener("cchub-shortcut-search", handleSearchShortcut);
  }, [editingSkill, editingMcp]);

  const formatJson = useCallback((raw: string): string => {
    try {
      return JSON.stringify(JSON.parse(raw), null, 2);
    } catch {
      return raw;
    }
  }, []);

  const refreshInstalledMcpDetails = useCallback(
    () =>
      performRefreshInstalledMcpDetails({
        queryClient,
        setInstalledMcpDetails,
        rebuildMcpByTool,
        setInstalledIdsByTool,
      }),
    [queryClient, rebuildMcpByTool],
  );

  const findInstalledSkill = useCallback(
    async (skill: SkillEntry) => {
      const skills = await invoke<InstalledSkillRecord[]>("scan_skills");
      // Prefer skills installed under the active tool so cross-tool name collisions
      // resolve to the file the user is currently looking at.
      const matches = skills.filter(
        (item) =>
          item.name.toLowerCase() === skill.name.toLowerCase() || item.name.toLowerCase() === skill.id.toLowerCase(),
      );
      if (matches.length === 0) return null;
      const inActiveTool = matches.find((m) => (m.tool_id ?? "claude") === activeTool);
      return inActiveTool ?? matches[0];
    },
    [activeTool],
  );

  const openSkillPreview = useCallback(async (skill: SkillEntry) => {
    if (!skill.content && skill.file_path) {
      try {
        const content = await invoke<string>("read_skill_content", { filePath: skill.file_path });
        const updated = { ...skill, content };
        setSkillEntries((prev) => prev.map((s) => (s.id === skill.id ? updated : s)));
        setPreviewSkill(updated);
        return;
      } catch (e) {
        console.error(e);
      }
    }
    setPreviewSkill(skill);
  }, []);

  const startSkillEdit = useCallback(
    async (skill: SkillEntry) => {
      try {
        const installed = await findInstalledSkill(skill);
        if (!installed?.file_path) {
          showToast("error", locale === "zh" ? "未找到已安装的技能文件" : "Installed skill file not found");
          return;
        }
        const content = await invoke<string>("read_skill_content", { filePath: installed.file_path });
        const updatedSkill = { ...skill, file_path: installed.file_path, content };
        setSkillEntries((prev) =>
          prev.map((s) =>
            s.id === skill.id || s.name.toLowerCase() === skill.name.toLowerCase()
              ? { ...s, file_path: installed.file_path, content }
              : s,
          ),
        );
        setPreviewSkill(null);
        setEditingSkill(updatedSkill);
        setSkillContent(content);
        setEditSkillContent(content);
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? "打开技能编辑器失败" : "Failed to open skill editor");
      }
    },
    [findInstalledSkill, locale],
  );

  const handleSaveSkillContent = useCallback(() => {
    if (!editingSkill) return;
    return performSaveSkillContent({
      editingSkill,
      editSkillContent,
      locale,
      setSkillContent,
      setEditingSkill,
      setSkillEntries,
    });
  }, [editingSkill, editSkillContent, locale]);

  const startMcpEdit = useCallback(
    async (entry: RegistryEntry) => {
      try {
        let installed =
          installedMcpDetails.find((server) => server.id === entry.id || server.name === entry.name) || null;
        if (!installed) {
          const refreshed = await refreshInstalledMcpDetails();
          installed = refreshed.find((server) => server.id === entry.id || server.name === entry.name) || null;
        }
        if (!installed) {
          showToast("error", locale === "zh" ? "未找到已安装的 MCP 配置" : "Installed MCP config not found");
          return;
        }
        setPreviewMcp(null);
        setEditingMcp(installed);
        setEditCommand(installed.command || "");
        setEditArgs(formatJson(installed.args || "[]"));
        setEditEnv(formatJson(installed.env || "{}"));
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? "打开 MCP 编辑器失败" : "Failed to open MCP editor");
      }
    },
    [installedMcpDetails, locale, refreshInstalledMcpDetails],
  );

  const handleSaveMcpConfig = useCallback(() => {
    if (!editingMcp) return;
    return performSaveMcpConfig({
      editingMcp,
      editCommand,
      editArgs,
      editEnv,
      locale,
      updateMcpServerConfigMutation,
      refreshInstalledMcpDetails,
      setEditingMcp,
      setEntries,
    });
  }, [editArgs, editCommand, editEnv, editingMcp, locale, refreshInstalledMcpDetails, updateMcpServerConfigMutation]);

  const handleSearch = useCallback(async () => {
    if (!search.trim()) {
      await loadAll({ force: true });
      return;
    }
    setLoading(true);
    setLoadError(null);
    try {
      if (tab === "mcp") {
        const result = await queryClient.fetchQuery({
          queryKey: queryKeys.marketplaceSearch(search, 0, 50),
          queryFn: () => fetchMarketplaceSearchPage(search, 0, 50),
          staleTime: 0,
        });
        setEntries(result.entries);
        setMcpTotal(result.total);
        setMcpPage(0);
      } else {
        // Search skills via SkillHub API
        const results = await invoke<SkillEntry[]>("search_skillhub_skills", { query: search, limit: 30 });
        setSkillEntries(results);
      }
    } catch (e) {
      console.error(e);
      setLoadError(String(e));
    } finally {
      setLoading(false);
    }
  }, [loadAll, queryClient, search, tab]);

  const doInstallMcp = useCallback(
    (entry: RegistryEntry, envVals: Record<string, string>) =>
      performInstallMcp({
        entry,
        envVals,
        activeTool,
        locale,
        setInstalling,
        setShowEnvModal,
        setInstalledIdsByTool,
      }),
    [activeTool, locale],
  );

  const handleInstallMcp = useCallback(
    async (entry: RegistryEntry) => {
      if (entry.env_keys.length > 0) {
        const vals: Record<string, string> = {};
        entry.env_keys.forEach((k) => {
          vals[k] = "";
        });
        setEnvValues(vals);
        setShowEnvModal(entry);
        return;
      }
      await doInstallMcp(entry, {});
    },
    [doInstallMcp],
  );

  const handleInstallSkill = useCallback(
    (skill: SkillEntry) =>
      performInstallSkill({
        skill,
        activeTool,
        locale,
        tools,
        setInstalling,
        setInstalledSkillsByTool,
      }),
    [activeTool, locale, tools],
  );

  const handleUninstallSkill = useCallback(async (skill: SkillEntry) => {
    setPendingUninstall(skill);
  }, []);

  const doUninstallSkill = useCallback(
    (skill: SkillEntry) =>
      performUninstallSkill({
        skill,
        activeTool,
        locale,
        editingSkill,
        setInstalledSkillsByTool,
        setSkillEntries,
        setEditingSkill,
        setSkillContent,
        setEditSkillContent,
      }),
    [activeTool, editingSkill, locale],
  );

  const handleCustomSource = useCallback(
    () =>
      performLoadCustomSource({
        customUrl,
        locale,
        setLoadingCustom,
        setSkillEntries,
        setCustomSources,
        setCustomUrl,
      }),
    [customUrl, locale],
  );

  const removeCustomSource = useCallback(
    (index: number) => {
      const source = customSources[index];
      if (!source) return;
      const idsToRemove = new Set(source.skillIds);
      setSkillEntries((prev) => prev.filter((s) => !idsToRemove.has(s.id)));
      setCustomSources((prev) => prev.filter((_, i) => i !== index));
    },
    [customSources],
  );

  const handleSearchChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setSearch(event.target.value);
  }, []);

  const handleSearchKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter") {
        void handleSearch();
      }
    },
    [handleSearch],
  );

  const handleClearSearch = useCallback(() => {
    setSearch("");
    if (tab === "mcp") {
      void loadAll({ force: true });
    }
  }, [loadAll, tab]);

  const handleSelectCategory = useCallback(
    (categoryKey: string) => {
      if (tab === "mcp") {
        setMcpCategory(categoryKey as McpCategory);
        return;
      }
      setSkillCategory(categoryKey as SkillCategory);
    },
    [tab],
  );

  const handleCustomUrlChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setCustomUrl(event.target.value);
  }, []);

  const handleCustomUrlKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter") {
        void handleCustomSource();
      }
    },
    [handleCustomSource],
  );

  const handleOpenRecommendedRepo = useCallback((repoName: string) => {
    void shellOpen(`https://github.com/${repoName}`);
  }, []);

  const handleLoadRecommendedRepo = useCallback(
    (repoName: string, branch: string) =>
      performLoadRecommendedRepo({
        repoName,
        branch,
        locale,
        setLoadingRepo,
        setSkillEntries,
        setCustomSources,
      }),
    [locale],
  );

  const featuredBundles = useMemo<FeaturedSkillBundle[]>(() => buildFeaturedBundles(locale), [locale]);

  const bundleInstalledCount = useCallback(
    (bundle: FeaturedSkillBundle) =>
      computeBundleInstalledCount(bundle, skillEntries, currentToolInstalledSkills, bundleProgress),
    [bundleProgress, currentToolInstalledSkills, skillEntries],
  );

  const handleInstallBundle = useCallback(
    (bundle: FeaturedSkillBundle) =>
      performInstallBundle({
        bundle,
        locale,
        activeTool,
        tools,
        setInstallingBundle,
        setBundleProgress,
        setSkillEntries,
        setCustomSources,
        setInstalledSkillsByTool,
      }),
    [activeTool, locale, tools],
  );

  const mcpCategories = useMemo<{ key: McpCategory; label: string }[]>(
    () => [
      { key: "all", label: `${locale === "zh" ? "全部" : "All"} (${entries.length})` },
      {
        key: "installed",
        label: `${locale === "zh" ? "已安装" : "Installed"} (${entries.filter((e) => currentToolInstalledIds.has(e.name) || currentToolInstalledIds.has(e.id)).length})`,
      },
    ],
    [entries, currentToolInstalledIds, locale],
  );

  const skillCategories = useMemo<{ key: SkillCategory; label: string }[]>(
    () => [
      { key: "all", label: `${locale === "zh" ? "全部" : "All"} (${skillEntries.length})` },
      {
        key: "installed",
        label: `${locale === "zh" ? "已安装" : "Installed"} (${skillEntries.filter((s) => currentToolInstalledSkills.has(s.name.toLowerCase())).length})`,
      },
    ],
    [currentToolInstalledSkills, locale, skillEntries],
  );

  const filteredMcp = useMemo(
    () =>
      entries.filter((e) => {
        if (mcpCategory === "installed") {
          if (!currentToolInstalledIds.has(e.name) && !currentToolInstalledIds.has(e.id)) return false;
        } else if (mcpCategory !== "all" && e.category !== mcpCategory) return false;
        if (search) {
          const q = search.toLowerCase();
          if (!e.name.toLowerCase().includes(q) && !e.description.toLowerCase().includes(q)) return false;
        }
        return true;
      }),
    [entries, currentToolInstalledIds, mcpCategory, search],
  );

  const filteredSkills = useMemo(
    () =>
      skillEntries.filter((s) => {
        if (skillCategory === "installed") {
          if (!currentToolInstalledSkills.has(s.name.toLowerCase())) return false;
        } else if (skillCategory !== "all" && s.category !== skillCategory) return false;
        if (search) {
          const q = search.toLowerCase();
          if (
            !s.name.toLowerCase().includes(q) &&
            !s.description.toLowerCase().includes(q) &&
            !(s.description_zh || "").toLowerCase().includes(q)
          )
            return false;
        }
        return true;
      }),
    [currentToolInstalledSkills, search, skillCategory, skillEntries],
  );

  const recommendedRepos = useMemo(() => buildRecommendedRepos(locale), [locale]);

  const activeCategoryKey = tab === "mcp" ? mcpCategory : skillCategory;
  const currentCategories = tab === "mcp" ? mcpCategories : skillCategories;
  const handleOpenGithub = useCallback((url: string) => {
    void shellOpen(url);
  }, []);
  const handlePreviewMcp = useCallback((entry: RegistryEntry) => {
    setPreviewMcp(entry);
  }, []);
  const handleOpenSkillPreview = useCallback(
    (skill: SkillEntry) => {
      void openSkillPreview(skill);
    },
    [openSkillPreview],
  );
  const handleEditMcp = useCallback(
    (entry: RegistryEntry) => {
      void startMcpEdit(entry);
    },
    [startMcpEdit],
  );
  const handleInstallMcpCard = useCallback(
    (entry: RegistryEntry) => {
      void handleInstallMcp(entry);
    },
    [handleInstallMcp],
  );
  const handleUninstallMcpCard = useCallback(
    (entry: RegistryEntry) =>
      performUninstallMcp({
        entry,
        activeTool,
        locale,
        setInstalledIdsByTool,
      }),
    [activeTool, locale],
  );
  const handleEditMarketSkill = useCallback(
    (skill: SkillEntry) => {
      void startSkillEdit(skill);
    },
    [startSkillEdit],
  );
  const handleInstallMarketSkill = useCallback(
    (skill: SkillEntry) => {
      void handleInstallSkill(skill);
    },
    [handleInstallSkill],
  );
  const handleUninstallMarketSkill = useCallback(
    (skill: SkillEntry) => {
      void handleUninstallSkill(skill);
    },
    [handleUninstallSkill],
  );
  const handleLoadPrevMcpPage = useCallback(
    () => loadMcpPage({ queryClient, search, setLoadingMore, setEntries, setMcpPage, setMcpTotal }, mcpPage - 1),
    [mcpPage, queryClient, search],
  );
  const handleLoadNextMcpPage = useCallback(
    () => loadMcpPage({ queryClient, search, setLoadingMore, setEntries, setMcpPage, setMcpTotal }, mcpPage + 1),
    [mcpPage, queryClient, search],
  );

  if (loading) {
    return <LoadingState label={i.marketplace.loading} />;
  }

  if (loadError) {
    return (
      <ErrorState
        title={localeText("市场加载失败", "Failed to load marketplace", "マーケットの読み込みに失敗しました")}
        message={loadError}
        retryLabel={i.common.refresh}
        onRetry={() => {
          void loadAll({ force: true });
        }}
      />
    );
  }

  const hasSkillChanges = editSkillContent !== skillContent;
  const originalMcpCommand = editingMcp?.command || "";
  const originalMcpArgs = editingMcp ? formatJson(editingMcp.args || "[]") : "";
  const originalMcpEnv = editingMcp ? formatJson(editingMcp.env || "{}") : "";
  const hasMcpChanges =
    !!editingMcp && (editCommand !== originalMcpCommand || editArgs !== originalMcpArgs || editEnv !== originalMcpEnv);

  if (editingSkill) {
    return (
      <SkillEditView
        locale={locale}
        editingSkill={editingSkill}
        skillContent={skillContent}
        editSkillContent={editSkillContent}
        hasSkillChanges={hasSkillChanges}
        setEditingSkill={setEditingSkill}
        setEditSkillContent={setEditSkillContent}
        handleSaveSkillContent={handleSaveSkillContent}
      />
    );
  }

  if (editingMcp) {
    return (
      <McpEditView
        locale={locale}
        editingMcp={editingMcp}
        editCommand={editCommand}
        editArgs={editArgs}
        editEnv={editEnv}
        originalMcpCommand={originalMcpCommand}
        originalMcpArgs={originalMcpArgs}
        originalMcpEnv={originalMcpEnv}
        hasMcpChanges={hasMcpChanges}
        setEditingMcp={setEditingMcp}
        setEditCommand={setEditCommand}
        setEditArgs={setEditArgs}
        setEditEnv={setEditEnv}
        handleSaveMcpConfig={handleSaveMcpConfig}
      />
    );
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <MarketplaceHeader
        locale={locale}
        localeText={localeText}
        tab={tab}
        search={search}
        activeTool={activeTool}
        visibleTools={visibleTools}
        entriesCount={entries.length}
        skillEntriesCount={skillEntries.length}
        activeCategoryKey={activeCategoryKey}
        currentCategories={currentCategories}
        searchInputRef={searchInputRef}
        setShowCustomSource={setShowCustomSource}
        setActiveTool={setActiveTool}
        setTab={setTab}
        setSearch={setSearch}
        setPreviewSkill={setPreviewSkill}
        setPreviewMcp={setPreviewMcp}
        handleSearchChange={handleSearchChange}
        handleSearchKeyDown={handleSearchKeyDown}
        handleClearSearch={handleClearSearch}
        handleSearch={handleSearch}
        handleSelectCategory={handleSelectCategory}
      />

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {tab === "mcp" ? (
          <McpGrid
            locale={locale}
            filteredMcp={filteredMcp}
            currentToolInstalledIds={currentToolInstalledIds}
            installing={installing}
            activeTool={activeTool}
            mcpCategory={mcpCategory}
            mcpTotal={mcpTotal}
            mcpPage={mcpPage}
            loadingMore={loadingMore}
            handlePreviewMcp={handlePreviewMcp}
            handleInstallMcpCard={handleInstallMcpCard}
            handleEditMcp={handleEditMcp}
            handleUninstallMcpCard={handleUninstallMcpCard}
            handleOpenGithub={handleOpenGithub}
            handleLoadPrevMcpPage={handleLoadPrevMcpPage}
            handleLoadNextMcpPage={handleLoadNextMcpPage}
          />
        ) : (
          <SkillsGrid
            locale={locale}
            filteredSkills={filteredSkills}
            currentToolInstalledSkills={currentToolInstalledSkills}
            installing={installing}
            activeTool={activeTool}
            showTranslation={showTranslation}
            featuredBundles={featuredBundles}
            installingBundle={installingBundle}
            bundleInstalledCount={bundleInstalledCount}
            handleInstallBundle={handleInstallBundle}
            handleOpenGithub={handleOpenGithub}
            handleOpenSkillPreview={handleOpenSkillPreview}
            handleInstallMarketSkill={handleInstallMarketSkill}
            handleEditMarketSkill={handleEditMarketSkill}
            handleUninstallMarketSkill={handleUninstallMarketSkill}
          />
        )}
      </div>

      <EnvModal
        locale={locale}
        showEnvModal={showEnvModal}
        envValues={envValues}
        setShowEnvModal={setShowEnvModal}
        setEnvValues={setEnvValues}
        doInstallMcp={doInstallMcp}
      />

      <CustomSourceModal
        locale={locale}
        show={showCustomSource}
        customUrl={customUrl}
        loadingCustom={loadingCustom}
        customSources={customSources}
        loadingRepo={loadingRepo}
        recommendedRepos={recommendedRepos}
        setShow={setShowCustomSource}
        handleCustomUrlChange={handleCustomUrlChange}
        handleCustomUrlKeyDown={handleCustomUrlKeyDown}
        handleCustomSource={handleCustomSource}
        handleOpenRecommendedRepo={handleOpenRecommendedRepo}
        handleLoadRecommendedRepo={handleLoadRecommendedRepo}
        removeCustomSource={removeCustomSource}
      />

      <SkillPreviewModal
        locale={locale}
        previewSkill={previewSkill}
        currentToolInstalledSkills={currentToolInstalledSkills}
        showTranslation={showTranslation}
        setPreviewSkill={setPreviewSkill}
        startSkillEdit={startSkillEdit}
        handleUninstallSkill={handleUninstallSkill}
        handleInstallSkill={handleInstallSkill}
      />

      <McpPreviewModal
        locale={locale}
        previewMcp={previewMcp}
        currentToolInstalledIds={currentToolInstalledIds}
        setPreviewMcp={setPreviewMcp}
        startMcpEdit={startMcpEdit}
        handleInstallMcp={handleInstallMcp}
      />

      <UninstallSkillDialog
        locale={locale}
        pendingUninstall={pendingUninstall}
        setPendingUninstall={setPendingUninstall}
        doUninstallSkill={doUninstallSkill}
      />
    </div>
  );
}
