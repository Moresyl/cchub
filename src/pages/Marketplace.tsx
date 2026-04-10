import { memo, useCallback, useEffect, lazy, Suspense, useMemo, useRef, useState, type ChangeEvent, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import {
  Store, Search, Download, CheckCircle, X, ExternalLink, Key,
  Plug, Zap, Plus, Globe, Edit3, Trash2, Save, ArrowLeft, RotateCcw, Wand2,
} from "lucide-react";
import { t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import MarketplaceCategoryTab from "../components/MarketplaceCategoryTab";
import MarketplaceCustomSourceRow from "../components/MarketplaceCustomSourceRow";
import MarketplaceMcpCard, { type MarketplaceMcpCardEntry } from "../components/MarketplaceMcpCard";
import MarketplaceRecommendedRepoRow from "../components/MarketplaceRecommendedRepoRow";
import MarketplaceSkillCard, { type MarketplaceSkillCardEntry } from "../components/MarketplaceSkillCard";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

const MarkdownEditor = lazy(() => import("../components/MarkdownEditor"));
const CodeEditor = lazy(() => import("../components/CodeEditor"));

type RegistryEntry = MarketplaceMcpCardEntry;

type SkillEntry = MarketplaceSkillCardEntry;

interface InstalledMcpServer {
  id: string; name: string; command: string | null; args: string; env: string;
  status: string; transport: string; source: string; package_name: string | null;
  version: string | null; config_path: string | null;
}

interface InstalledSkillRecord {
  id: string; name: string; description: string | null;
  plugin_id: string | null; trigger_command: string | null; file_path: string | null;
}

type MarketTab = "mcp" | "skills";

type McpCategory = "all" | "installed" | "search" | "database" | "ai" | "dev-tools" | "browser" | "filesystem" | "cloud" | "productivity";
type SkillCategory = "all" | "installed" | "development" | "testing" | "documentation" | "devops" | "ai-ml" | "security" | "backend";

const MCP_CATEGORY_ZH: Record<string, string> = {
  local: "本地", search: "搜索", database: "数据库", ai: "AI",
  "dev-tools": "开发工具", browser: "浏览器", filesystem: "文件系统",
  cloud: "云服务", productivity: "效率", npm: "npm",
  "official-plugin": "官方插件", "community-plugin": "社区插件",
};

export default function Marketplace() {
  const [tab, setTab] = useState<MarketTab>("mcp");
  const [entries, setEntries] = useState<RegistryEntry[]>([]);
  const [skillEntries, setSkillEntries] = useState<SkillEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [mcpCategory, setMcpCategory] = useState<McpCategory>("all");
  const [skillCategory, setSkillCategory] = useState<SkillCategory>("all");
  const [installedIds, setInstalledIds] = useState<Set<string>>(new Set());
  const [installedSkills, setInstalledSkills] = useState<Set<string>>(new Set());
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
  const [mcpTotal, setMcpTotal] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [installedMcpDetails, setInstalledMcpDetails] = useState<InstalledMcpServer[]>([]);
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
  const i = t();
  const locale = localStorage.getItem("cchub-locale") || "zh";
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const localeText = useCallback((zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  ), [locale]);

  const loadAll = useCallback(async () => {
    setLoading(true);
    try {
      try {
        const servers = await invoke<InstalledMcpServer[]>("scan_mcp_servers");
        setInstalledMcpDetails(servers);
        setInstalledIds(new Set(servers.flatMap((s) => [s.id, s.name])));
        const scannedEntries: RegistryEntry[] = servers.map((s) => ({
          id: s.id,
          name: s.name,
          description: s.command ? `${s.command} ${(() => { try { return JSON.parse(s.args || "[]").join(" "); } catch { return ""; } })()}` : "",
          category: s.source || "local",
          install_type: "local",
          package_name: s.package_name,
          github_url: null,
          command: s.command || "",
          args: (() => { try { return JSON.parse(s.args || "[]"); } catch { return []; } })(),
          env_keys: (() => { try { return Object.keys(JSON.parse(s.env || "{}")); } catch { return []; } })(),
          source: "local",
        }));
        setEntries(scannedEntries);
      } catch { /* ignore */ }

      let localSkillEntries: SkillEntry[] = [];
      try {
        const skills = await invoke<InstalledSkillRecord[]>("scan_skills");
        setInstalledSkills(new Set(skills.map((s) => s.name.toLowerCase())));
        localSkillEntries = skills.map((s) => ({
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
      } catch { /* ignore */ }

      try {
        const result = await invoke<{ skills: SkillEntry[]; total: number }>("get_skillhub_catalog", { page: 1, limit: 50, category: "" });
        const marketSkills = result.skills || [];
        const localNames = new Set(localSkillEntries.map((s) => s.name.toLowerCase()));
        const merged = [...localSkillEntries, ...marketSkills.filter((s) => !localNames.has(s.name.toLowerCase()))];
        setSkillEntries(merged);
      } catch (e) {
        console.warn("SkillHub API failed, showing local skills only:", e);
        setSkillEntries(localSkillEntries);
      }

      try {
        const result = await invoke<{ entries: RegistryEntry[]; total: number }>("search_marketplace", { query: "mcp server", page: 0, pageSize: 50 });
        setEntries((prev) => {
          const ids = new Set(prev.map((e) => e.id));
          return [...prev, ...result.entries.filter((e) => !ids.has(e.id))];
        });
        setMcpTotal(result.total);
        setMcpPage(0);
      } catch { /* ignore */ }
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { void loadAll(); }, [loadAll]);
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
    try { return JSON.stringify(JSON.parse(raw), null, 2); } catch { return raw; }
  }, []);

  const refreshInstalledMcpDetails = useCallback(async () => {
    const servers = await invoke<InstalledMcpServer[]>("scan_mcp_servers");
    setInstalledMcpDetails(servers);
    setInstalledIds(new Set(servers.flatMap((s) => [s.id, s.name])));
    return servers;
  }, []);

  const findInstalledSkill = useCallback(async (skill: SkillEntry) => {
    const skills = await invoke<InstalledSkillRecord[]>("scan_skills");
    return skills.find((item) => (
      item.name.toLowerCase() === skill.name.toLowerCase() ||
      item.name.toLowerCase() === skill.id.toLowerCase()
    )) || null;
  }, []);

  const openSkillPreview = useCallback(async (skill: SkillEntry) => {
    if (!skill.content && skill.file_path) {
      try {
        const content = await invoke<string>("read_skill_content", { filePath: skill.file_path });
        const updated = { ...skill, content };
        setSkillEntries(prev => prev.map(s => s.id === skill.id ? updated : s));
        setPreviewSkill(updated);
        return;
      } catch (e) {
        console.error(e);
      }
    }
    setPreviewSkill(skill);
  }, []);

  const startSkillEdit = useCallback(async (skill: SkillEntry) => {
    try {
      const installed = await findInstalledSkill(skill);
      if (!installed?.file_path) {
        showToast("error", locale === "zh" ? "未找到已安装的技能文件" : "Installed skill file not found");
        return;
      }
      const content = await invoke<string>("read_skill_content", { filePath: installed.file_path });
      const updatedSkill = { ...skill, file_path: installed.file_path, content };
      setSkillEntries(prev => prev.map(s => (
        s.id === skill.id || s.name.toLowerCase() === skill.name.toLowerCase()
          ? { ...s, file_path: installed.file_path, content }
          : s
      )));
      setPreviewSkill(null);
      setEditingSkill(updatedSkill);
      setSkillContent(content);
      setEditSkillContent(content);
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? "打开技能编辑器失败" : "Failed to open skill editor");
    }
  }, [findInstalledSkill, locale]);

  const handleSaveSkillContent = useCallback(async () => {
    if (!editingSkill?.file_path) return;
    try {
      await invoke("write_skill_content", { filePath: editingSkill.file_path, content: editSkillContent });
      setSkillContent(editSkillContent);
      setEditingSkill(prev => prev ? { ...prev, content: editSkillContent } : prev);
      setSkillEntries(prev => prev.map(s => (
        s.id === editingSkill.id || s.name.toLowerCase() === editingSkill.name.toLowerCase()
          ? { ...s, file_path: editingSkill.file_path, content: editSkillContent }
          : s
      )));
      showToast("success", locale === "zh" ? "技能已保存" : "Skill saved");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? "技能保存失败" : "Failed to save skill");
    }
  }, [editingSkill, editSkillContent, locale]);

  const startMcpEdit = useCallback(async (entry: RegistryEntry) => {
    try {
      let installed = installedMcpDetails.find((server) => server.id === entry.id || server.name === entry.name) || null;
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
  }, [installedMcpDetails, locale, refreshInstalledMcpDetails]);

  const handleSaveMcpConfig = useCallback(async () => {
    if (!editingMcp) return;
    try {
      const args = JSON.parse(editArgs);
      const env = JSON.parse(editEnv);
      await invoke("update_mcp_server_config", {
        name: editingMcp.name,
        command: editCommand,
        args,
        env,
      });
      const refreshed = await refreshInstalledMcpDetails();
      const updated = refreshed.find(server => server.id === editingMcp.id || server.name === editingMcp.name) || null;
      setEditingMcp(updated || {
        ...editingMcp,
        command: editCommand,
        args: JSON.stringify(args),
        env: JSON.stringify(env),
      });
      setEntries(prev => prev.map(entry => (
        entry.id === editingMcp.id || entry.name === editingMcp.name
          ? {
              ...entry,
              command: editCommand,
              args,
              env_keys: Object.keys(env),
              description: `${editCommand} ${Array.isArray(args) ? args.join(" ") : ""}`.trim(),
            }
          : entry
      )));
      showToast("success", locale === "zh" ? "MCP 配置已保存" : "MCP config saved");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? "JSON 格式错误，请检查参数和环境变量" : "Invalid JSON format");
    }
  }, [editArgs, editCommand, editEnv, editingMcp, locale, refreshInstalledMcpDetails]);

  const handleSearch = useCallback(async () => {
    if (!search.trim()) { await loadAll(); return; }
    setLoading(true);
    try {
      if (tab === "mcp") {
        const result = await invoke<{ entries: RegistryEntry[]; total: number }>("search_marketplace", { query: search, page: 0, pageSize: 50 });
        setEntries(result.entries);
        setMcpTotal(result.total);
        setMcpPage(0);
      } else {
        // Search skills via SkillHub API
        const results = await invoke<SkillEntry[]>("search_skillhub_skills", { query: search, limit: 30 });
        setSkillEntries(results);
      }
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }, [loadAll, search, tab]);

  const doInstallMcp = useCallback(async (entry: RegistryEntry, envVals: Record<string, string>) => {
    setInstalling(entry.id);
    setShowEnvModal(null);
    try {
      await invoke("install_from_marketplace", {
        name: entry.name, command: entry.command, args: entry.args, envValues: envVals,
      });
      setInstalledIds(prev => new Set([...prev, entry.name, entry.id]));
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? "安装失败" : "Installation failed");
    }
    finally { setInstalling(null); }
  }, [locale]);

  const handleInstallMcp = useCallback(async (entry: RegistryEntry) => {
    if (entry.env_keys.length > 0) {
      const vals: Record<string, string> = {};
      entry.env_keys.forEach((k) => { vals[k] = ""; });
      setEnvValues(vals);
      setShowEnvModal(entry);
      return;
    }
    await doInstallMcp(entry, {});
  }, [doInstallMcp]);

  const handleInstallSkill = useCallback(async (skill: SkillEntry) => {
    setInstalling(skill.id);
    try {
      await invoke<string>("install_skill_from_marketplace", {
        name: skill.id, content: skill.content, targetDir: null,
      });
      setInstalledSkills(prev => new Set([...prev, skill.name.toLowerCase()]));
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? "安装失败" : "Installation failed");
    }
    finally { setInstalling(null); }
  }, [locale]);

  const handleUninstallSkill = useCallback(async (skill: SkillEntry) => {
    setPendingUninstall(skill);
  }, []);

  const doUninstallSkill = useCallback(async (skill: SkillEntry) => {
    try {
      const skills = await invoke<InstalledSkillRecord[]>("scan_skills");
      const installed = skills.find(s => s.name.toLowerCase() === skill.name.toLowerCase() || s.name.toLowerCase() === skill.id.toLowerCase());
      if (installed?.file_path) {
        await invoke("uninstall_skill_file", { path: installed.file_path });
        setInstalledSkills(prev => {
          const next = new Set(prev);
          next.delete(skill.name.toLowerCase());
          return next;
        });
        setSkillEntries(prev => prev.map(item => (
          item.id === skill.id || item.name.toLowerCase() === skill.name.toLowerCase()
            ? { ...item, file_path: null, content: "" }
            : item
        )));
        if (editingSkill && editingSkill.name.toLowerCase() === skill.name.toLowerCase()) {
          setEditingSkill(null);
          setSkillContent("");
          setEditSkillContent("");
        }
      }
    } catch (e) { console.error(e); }
  }, [editingSkill]);

  const handleCustomSource = useCallback(async () => {
    if (!customUrl.trim()) return;
    setLoadingCustom(true);
    try {
      const custom = await invoke<SkillEntry[]>("fetch_custom_skill_source", { url: customUrl.trim() });
      const newIds: string[] = [];
      setSkillEntries(prev => {
        const existingIds = new Set(prev.map(s => s.id));
        const newEntries = custom.filter(s => !existingIds.has(s.id));
        newEntries.forEach(s => newIds.push(s.id));
        return [...prev, ...newEntries];
      });
      setCustomSources(prev => [...prev, { url: customUrl.trim(), count: newIds.length, skillIds: newIds }]);
      setCustomUrl("");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? "加载失败，请检查 URL 格式" : "Failed to load. Check URL format.");
    }
    finally { setLoadingCustom(false); }
  }, [customUrl, locale]);

  const removeCustomSource = useCallback((index: number) => {
    const source = customSources[index];
    if (!source) return;
    const idsToRemove = new Set(source.skillIds);
    setSkillEntries(prev => prev.filter(s => !idsToRemove.has(s.id)));
    setCustomSources(prev => prev.filter((_, i) => i !== index));
  }, [customSources]);

  const handleSearchChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setSearch(event.target.value);
  }, []);

  const handleSearchKeyDown = useCallback((event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      void handleSearch();
    }
  }, [handleSearch]);

  const handleClearSearch = useCallback(() => {
    setSearch("");
    if (tab === "mcp") {
      void loadAll();
    }
  }, [loadAll, tab]);

  const handleSelectCategory = useCallback((categoryKey: string) => {
    if (tab === "mcp") {
      setMcpCategory(categoryKey as McpCategory);
      return;
    }
    setSkillCategory(categoryKey as SkillCategory);
  }, [tab]);

  const handleCustomUrlChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setCustomUrl(event.target.value);
  }, []);

  const handleCustomUrlKeyDown = useCallback((event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      void handleCustomSource();
    }
  }, [handleCustomSource]);

  const handleOpenRecommendedRepo = useCallback((repoName: string) => {
    void shellOpen(`https://github.com/${repoName}`);
  }, []);

  const handleLoadRecommendedRepo = useCallback(async (repoName: string, branch: string) => {
    const [owner, repo] = repoName.split("/");
    setLoadingRepo(repoName);
    try {
      const skills = await invoke<SkillEntry[]>("fetch_skills_from_repo", { owner, repo, branch });
      const newIds: string[] = [];
      setSkillEntries(prev => {
        const existingIds = new Set(prev.map(s => s.id));
        const newEntries = skills.filter(s => !existingIds.has(s.id));
        newEntries.forEach(s => newIds.push(s.id));
        return [...prev, ...newEntries];
      });
      setCustomSources(prev => [...prev, { url: `github:${repoName}`, count: newIds.length, skillIds: newIds }]);
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `加载失败: ${e}` : `Load failed: ${e}`);
    } finally {
      setLoadingRepo(null);
    }
  }, [locale]);

  const mcpCategories = useMemo<{ key: McpCategory; label: string }[]>(() => [
    { key: "all", label: `${locale === "zh" ? "全部" : "All"} (${entries.length})` },
    { key: "installed", label: `${locale === "zh" ? "已安装" : "Installed"} (${entries.filter(e => installedIds.has(e.name) || installedIds.has(e.id)).length})` },
  ], [entries, installedIds, locale]);

  const skillCategories = useMemo<{ key: SkillCategory; label: string }[]>(() => [
    { key: "all", label: `${locale === "zh" ? "全部" : "All"} (${skillEntries.length})` },
    { key: "installed", label: `${locale === "zh" ? "已安装" : "Installed"} (${skillEntries.filter(s => installedSkills.has(s.name.toLowerCase())).length})` },
  ], [installedSkills, locale, skillEntries]);

  const filteredMcp = useMemo(() => entries.filter(e => {
    if (mcpCategory === "installed") {
      if (!installedIds.has(e.name) && !installedIds.has(e.id)) return false;
    } else if (mcpCategory !== "all" && e.category !== mcpCategory) return false;
    if (search) {
      const q = search.toLowerCase();
      if (!e.name.toLowerCase().includes(q) && !e.description.toLowerCase().includes(q)) return false;
    }
    return true;
  }), [entries, installedIds, mcpCategory, search]);

  const filteredSkills = useMemo(() => skillEntries.filter(s => {
    if (skillCategory === "installed") {
      if (!installedSkills.has(s.name.toLowerCase())) return false;
    } else if (skillCategory !== "all" && s.category !== skillCategory) return false;
    if (search) {
      const q = search.toLowerCase();
      if (!s.name.toLowerCase().includes(q) && !s.description.toLowerCase().includes(q) && !(s.description_zh || "").toLowerCase().includes(q)) return false;
    }
    return true;
  }), [installedSkills, search, skillCategory, skillEntries]);

  const recommendedRepos = useMemo(() => [
    {
      name: "levnikolaevich/claude-code-skills",
      branch: "master",
      desc: locale === "zh" ? "125 个技能 — 全流程交付（研发/测试/评审）" : "125 skills — full delivery workflow",
    },
    {
      name: "rohitg00/awesome-claude-code-toolkit",
      branch: "main",
      desc: locale === "zh" ? "35 个技能 + 135 个 Agent + 42 个命令" : "35 skills + 135 agents + 42 commands",
    },
    {
      name: "JimLiu/baoyu-skills",
      branch: "main",
      desc: locale === "zh" ? "18 个技能 — 宝玉技能合集" : "18 skills — Baoyu skills collection",
    },
    {
      name: "cexll/myclaude",
      branch: "master",
      desc: locale === "zh" ? "11 个技能 — 浏览器/代码/开发等" : "11 skills — browser/code/dev",
    },
  ], [locale]);

  const activeCategoryKey = tab === "mcp" ? mcpCategory : skillCategory;
  const currentCategories = tab === "mcp" ? mcpCategories : skillCategories;
  const handleOpenGithub = useCallback((url: string) => {
    void shellOpen(url);
  }, []);
  const handlePreviewMcp = useCallback((entry: RegistryEntry) => {
    setPreviewMcp(entry);
  }, []);
  const handleOpenSkillPreview = useCallback((skill: SkillEntry) => {
    void openSkillPreview(skill);
  }, [openSkillPreview]);
  const handleEditMcp = useCallback((entry: RegistryEntry) => {
    void startMcpEdit(entry);
  }, [startMcpEdit]);
  const handleInstallMcpCard = useCallback((entry: RegistryEntry) => {
    void handleInstallMcp(entry);
  }, [handleInstallMcp]);
  const handleEditMarketSkill = useCallback((skill: SkillEntry) => {
    void startSkillEdit(skill);
  }, [startSkillEdit]);
  const handleInstallMarketSkill = useCallback((skill: SkillEntry) => {
    void handleInstallSkill(skill);
  }, [handleInstallSkill]);
  const handleUninstallMarketSkill = useCallback((skill: SkillEntry) => {
    void handleUninstallSkill(skill);
  }, [handleUninstallSkill]);
  const handleLoadPrevMcpPage = useCallback(async () => {
    setLoadingMore(true);
    try {
      const prevPage = mcpPage - 1;
      const result = await invoke<{ entries: RegistryEntry[]; total: number }>("search_marketplace", {
        query: search || "mcp server", page: prevPage, pageSize: 50,
      });
      setEntries(result.entries);
      setMcpPage(prevPage);
      setMcpTotal(result.total);
    } catch (e) { console.error(e); }
    finally { setLoadingMore(false); }
  }, [mcpPage, search]);
  const handleLoadNextMcpPage = useCallback(async () => {
    setLoadingMore(true);
    try {
      const nextPage = mcpPage + 1;
      const result = await invoke<{ entries: RegistryEntry[]; total: number }>("search_marketplace", {
        query: search || "mcp server", page: nextPage, pageSize: 50,
      });
      setEntries(result.entries);
      setMcpPage(nextPage);
      setMcpTotal(result.total);
    } catch (e) { console.error(e); }
    finally { setLoadingMore(false); }
  }, [mcpPage, search]);

  if (loading) {
    return <div className="loading-center"><div className="spinner" /><span style={{ fontSize: 13, color: "var(--text-muted)" }}>{i.marketplace.loading}</span></div>;
  }

  const hasSkillChanges = editSkillContent !== skillContent;
  const originalMcpCommand = editingMcp?.command || "";
  const originalMcpArgs = editingMcp ? formatJson(editingMcp.args || "[]") : "";
  const originalMcpEnv = editingMcp ? formatJson(editingMcp.env || "{}") : "";
  const hasMcpChanges = !!editingMcp && (
    editCommand !== originalMcpCommand ||
    editArgs !== originalMcpArgs ||
    editEnv !== originalMcpEnv
  );

  if (editingSkill) {
    return (
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <div className="page-header">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <button className="btn btn-ghost btn-icon-sm" onClick={() => setEditingSkill(null)}>
              <ArrowLeft size={16} />
            </button>
            <Zap size={18} style={{ color: "var(--warning)" }} />
            <h2 className="page-title" style={{ margin: 0 }}>{editingSkill.name}</h2>
            {hasSkillChanges && (
              <span className="badge badge-warning">{locale === "zh" ? "未保存" : "Unsaved"}</span>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            {hasSkillChanges && (
              <button className="btn btn-secondary btn-sm" onClick={() => setEditSkillContent(skillContent)}>
                <RotateCcw size={14} />{locale === "zh" ? "撤销" : "Revert"}
              </button>
            )}
            <button className="btn btn-primary btn-sm" onClick={handleSaveSkillContent} disabled={!hasSkillChanges}>
              <Save size={14} />{i.common.save}
            </button>
          </div>
        </div>

        {editingSkill.file_path && (
          <div style={{ marginBottom: 16 }}>
            <div className="code-block" style={{ fontSize: 11 }}>{editingSkill.file_path}</div>
          </div>
        )}

        <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
          <Suspense fallback={
            <div style={{ display: "flex", alignItems: "center", gap: 10, padding: 40, justifyContent: "center" }}>
              <div className="spinner" style={{ width: 18, height: 18 }} />
            </div>
          }>
            <MarkdownEditor
              value={editSkillContent}
              onChange={setEditSkillContent}
              minHeight={500}
            />
          </Suspense>
        </div>
      </div>
    );
  }

  if (editingMcp) {
    return (
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <div className="page-header">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <button className="btn btn-ghost btn-icon-sm" onClick={() => setEditingMcp(null)} title={locale === "zh" ? "返回" : "Back"}>
              <ArrowLeft size={16} />
            </button>
            <div>
              <h2 className="page-title">{editingMcp.name}</h2>
              <p className="page-subtitle">{locale === "zh" ? "编辑 MCP 服务器配置" : "Edit MCP server configuration"}</p>
            </div>
          </div>
          {hasMcpChanges && (
            <span className="badge badge-warning">{locale === "zh" ? "未保存" : "Unsaved"}</span>
          )}
        </div>

        <div style={{ flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 20, paddingBottom: 20 }}>
          <div>
            <span className="field-label">{locale === "zh" ? "命令" : "Command"}</span>
            <input
              className="input"
              style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
              value={editCommand}
              onChange={(e) => setEditCommand(e.target.value)}
              placeholder="npx, node, python..."
            />
          </div>

          <div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
              <span className="field-label" style={{ marginBottom: 0 }}>{locale === "zh" ? "参数" : "Arguments"}</span>
              <button className="btn btn-ghost btn-icon-sm" title="Format" onClick={() => {
                try { setEditArgs(JSON.stringify(JSON.parse(editArgs), null, 2)); } catch { /* ignore */ }
              }}><Wand2 size={12} /></button>
            </div>
            <CodeEditor
              value={editArgs}
              onChange={setEditArgs}
              language="json"
              minHeight={160}
            />
          </div>

          <div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
              <span className="field-label" style={{ marginBottom: 0 }}>{locale === "zh" ? "环境变量" : "Environment"}</span>
              <button className="btn btn-ghost btn-icon-sm" title="Format" onClick={() => {
                try { setEditEnv(JSON.stringify(JSON.parse(editEnv), null, 2)); } catch { /* ignore */ }
              }}><Wand2 size={12} /></button>
            </div>
            <CodeEditor
              value={editEnv}
              onChange={setEditEnv}
              language="json"
              minHeight={160}
            />
          </div>
        </div>

        <div className="sticky-footer" style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          {hasMcpChanges && (
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => {
                setEditCommand(originalMcpCommand);
                setEditArgs(originalMcpArgs);
                setEditEnv(originalMcpEnv);
              }}
            >
              <RotateCcw size={14} />{locale === "zh" ? "撤销" : "Revert"}
            </button>
          )}
          <button className="btn btn-primary btn-sm" onClick={handleSaveMcpConfig} disabled={!hasMcpChanges}>
            <Save size={14} />{i.common.save}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {/* Header */}
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.marketplace.title}</h2>
          <p className="page-subtitle">{i.marketplace.subtitle}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {tab === "skills" && (
            <button className="btn btn-secondary btn-sm" onClick={() => setShowCustomSource(true)} style={{ gap: 6 }}>
              <Plus size={14} />{locale === "zh" ? "自定义源" : "Custom Source"}
            </button>
          )}
        </div>
      </div>

      {/* Tab Switch: MCP Servers / Skills */}
      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        <button
          className={`btn btn-sm ${tab === "mcp" ? "btn-primary" : "btn-secondary"}`}
          onClick={() => { setTab("mcp"); setSearch(""); }}
          style={{ gap: 6 }}
        >
          <Plug size={14} />{locale === "zh" ? "MCP 服务" : "MCP Servers"}
          <span style={{ fontSize: 11, opacity: 0.7 }}>({entries.length})</span>
        </button>
        <button
          className={`btn btn-sm ${tab === "skills" ? "btn-primary" : "btn-secondary"}`}
          onClick={() => { setTab("skills"); setSearch(""); }}
          style={{ gap: 6 }}
        >
          <Zap size={14} />{locale === "zh" ? "技能" : "Skills"}
          <span style={{ fontSize: 11, opacity: 0.7 }}>({skillEntries.length})</span>
        </button>
      </div>

      {/* Search + Categories */}
      <div style={{ display: "flex", gap: 16, marginBottom: 20, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1, maxWidth: 400, display: "flex", gap: 8 }}>
          <div style={{ position: "relative", flex: 1 }}>
            <Search size={15} style={{ position: "absolute", left: 14, top: "50%", transform: "translateY(-50%)", color: "var(--text-muted)" }} />
            <input
              ref={searchInputRef}
              className="input"
              style={{ paddingLeft: 40, paddingRight: search ? 36 : 12 }}
              placeholder={tab === "mcp" ? i.marketplace.searchPlaceholder : localeText("搜索技能...", "Search skills...", "スキルを検索...")}
              value={search}
              onChange={handleSearchChange}
              onKeyDown={handleSearchKeyDown}
            />
            {search && (
              <button
                className="btn btn-ghost btn-icon-sm"
                style={{ position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)" }}
                onClick={handleClearSearch}
              ><X size={14} /></button>
            )}
          </div>
          <button className="btn btn-primary btn-sm" onClick={handleSearch} style={{ flexShrink: 0, gap: 5 }}>
            <Search size={13} />{locale === "zh" ? "搜索" : "Search"}
          </button>
        </div>
        <div className="tab-bar" style={{ flexWrap: "wrap" }}>
          {currentCategories.map((cat) => (
            <MarketplaceCategoryTab
              key={cat.key}
              categoryKey={cat.key}
              label={cat.label}
              active={activeCategoryKey === cat.key}
              onSelect={handleSelectCategory}
            />
          ))}
        </div>
      </div>

      {/* Grid */}
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {tab === "mcp" ? (
          /* MCP Servers Grid */
          filteredMcp.length === 0 ? (
            <EmptyState icon={Store} text={i.marketplace.noResults} sub={i.marketplace.noResultsTip} />
          ) : (
            <>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))", gap: 16 }} className="stagger">
              {filteredMcp.map(entry => {
                const isInstalled = installedIds.has(entry.name) || installedIds.has(entry.id);
                const isInstalling = installing === entry.id;
                return (
                  <MarketplaceMcpCard
                    key={entry.id}
                    entry={entry}
                    categoryLabel={locale === "zh" ? (MCP_CATEGORY_ZH[entry.category] || entry.category) : entry.category}
                    installed={isInstalled}
                    installing={isInstalling}
                    installedLabel={i.marketplace.installed}
                    installLabel={i.marketplace.install}
                    installingLabel={i.marketplace.installing}
                    editTitle={locale === "zh" ? "编辑" : "Edit"}
                    githubLabel="GitHub"
                    keysLabel={locale === "zh" ? "密钥" : "keys"}
                    onPreview={handlePreviewMcp}
                    onInstall={handleInstallMcpCard}
                    onEdit={handleEditMcp}
                    onOpenGithub={handleOpenGithub}
                  />
                );
              })}
            </div>
            {mcpCategory !== "installed" && mcpTotal > 50 && (
              <div style={{ display: "flex", justifyContent: "center", alignItems: "center", gap: 12, padding: "20px 0" }}>
                <button
                  className="btn btn-secondary btn-sm"
                  disabled={mcpPage === 0 || loadingMore}
                  onClick={() => void handleLoadPrevMcpPage()}
                >
                  {locale === "zh" ? "上一页" : "Prev"}
                </button>
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {locale === "zh"
                    ? `第 ${mcpPage + 1} / ${Math.ceil(mcpTotal / 50)} 页（共 ${mcpTotal} 个）`
                    : `Page ${mcpPage + 1} / ${Math.ceil(mcpTotal / 50)} (${mcpTotal} total)`}
                </span>
                <button
                  className="btn btn-secondary btn-sm"
                  disabled={(mcpPage + 1) * 50 >= mcpTotal || loadingMore}
                  onClick={() => void handleLoadNextMcpPage()}
                >
                  {loadingMore ? <div className="spinner" style={{ width: 12, height: 12 }} /> : null}
                  {locale === "zh" ? "下一页" : "Next"}
                </button>
              </div>
            )}
            </>
          )
        ) : (
          /* Skills Grid */
          filteredSkills.length === 0 ? (
            <EmptyState icon={Zap} text={locale === "zh" ? "未找到技能" : "No skills found"} sub={locale === "zh" ? "尝试其他关键词或添加自定义源" : "Try different keywords or add custom source"} />
          ) : (
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))", gap: 16 }} className="stagger">
              {filteredSkills.map(skill => {
                const isInstalled = installedSkills.has(skill.name.toLowerCase());
                const isInstalling = installing === skill.id;
                const desc = showTranslation && locale === "zh" && skill.description_zh ? skill.description_zh : skill.description;
                return (
                  <MarketplaceSkillCard
                    key={skill.id}
                    skill={skill}
                    description={desc}
                    installed={isInstalled}
                    installing={isInstalling}
                    installedLabel={i.marketplace.installed}
                    installLabel={i.marketplace.install}
                    installingLabel={i.marketplace.installing}
                    editTitle={locale === "zh" ? "编辑" : "Edit"}
                    uninstallTitle={locale === "zh" ? "卸载" : "Uninstall"}
                    githubLabel="GitHub"
                    onPreview={handleOpenSkillPreview}
                    onInstall={handleInstallMarketSkill}
                    onEdit={handleEditMarketSkill}
                    onUninstall={handleUninstallMarketSkill}
                    onOpenGithub={handleOpenGithub}
                  />
                );
              })}
            </div>
          )
        )}
      </div>

      {/* Env Modal */}
      {showEnvModal && (
        <div style={{ position: "fixed", inset: 0, background: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setShowEnvModal(null)}>
          <div className="section-card" style={{ width: 440, maxHeight: "80vh", overflowY: "auto" }} onClick={e => e.stopPropagation()}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}>
              <h3 style={{ fontSize: 16, fontWeight: 700 }}>{i.marketplace.envRequired}</h3>
              <button className="btn btn-ghost btn-icon-sm" onClick={() => setShowEnvModal(null)}><X size={16} /></button>
            </div>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginBottom: 16 }}>
              {locale === "zh" ? `安装 ${showEnvModal.name} 需要配置以下环境变量：` : `${showEnvModal.name} requires the following environment variables:`}
            </p>
            <div style={{ display: "flex", flexDirection: "column", gap: 12, marginBottom: 20 }}>
              {showEnvModal.env_keys.map(key => (
                <div key={key}>
                  <span className="field-label">{key}</span>
                  <input className="input" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 13 }}
                    placeholder={`${locale === "zh" ? "输入" : "Enter"} ${key}`}
                    value={envValues[key] || ""} onChange={e => setEnvValues(prev => ({ ...prev, [key]: e.target.value }))} />
                </div>
              ))}
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
              <button className="btn btn-secondary btn-sm" onClick={() => setShowEnvModal(null)}>{i.common.cancel}</button>
              <button className="btn btn-primary btn-sm" onClick={() => doInstallMcp(showEnvModal, envValues)}>
                <Download size={13} />{i.marketplace.install}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Custom Source Modal */}
      {showCustomSource && (
        <div style={{ position: "fixed", inset: 0, background: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setShowCustomSource(false)}>
          <div className="section-card" style={{ width: 520, maxHeight: "80vh", overflowY: "auto" }} onClick={e => e.stopPropagation()}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <Globe size={18} style={{ color: "var(--text-secondary)" }} />
                <h3 style={{ fontSize: 16, fontWeight: 700 }}>{locale === "zh" ? "自定义源管理" : "Custom Sources"}</h3>
              </div>
              <button className="btn btn-ghost btn-icon-sm" onClick={() => setShowCustomSource(false)}><X size={16} /></button>
            </div>

            {/* Built-in sources */}
            <div style={{ marginBottom: 20 }}>
              <span className="field-label">{locale === "zh" ? "内置源" : "Built-in Sources"}</span>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "10px 14px", borderRadius: 8, background: "var(--bg-input)" }}>
                  <Plug size={14} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>MCP Registry</div>
                    <div style={{ fontSize: 11, color: "var(--text-muted)" }}>{locale === "zh" ? "内置精选列表 + npm 搜索" : "Curated list + npm search"}</div>
                  </div>
                  <span className="badge badge-success" style={{ fontSize: 10 }}>{locale === "zh" ? "默认" : "Default"}</span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "10px 14px", borderRadius: 8, background: "var(--bg-input)" }}>
                  <Zap size={14} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>Skills Registry</div>
                    <div style={{ fontSize: 11, color: "var(--text-muted)" }}>{locale === "zh" ? "内置技能注册表" : "Built-in skills registry"}</div>
                  </div>
                  <span className="badge badge-success" style={{ fontSize: 10 }}>{locale === "zh" ? "默认" : "Default"}</span>
                </div>
              </div>
            </div>

            {/* Recommended skill repos */}
            <div style={{ marginBottom: 20 }}>
              <span className="field-label">{locale === "zh" ? "推荐技能仓库" : "Recommended Skill Repos"}</span>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                {recommendedRepos.map((repo) => (
                  <MarketplaceRecommendedRepoRow
                    key={repo.name}
                    repoName={repo.name}
                    branch={repo.branch}
                    description={repo.desc}
                    isLoaded={customSources.some((source) => source.url === `github:${repo.name}`)}
                    isLoading={loadingRepo === repo.name}
                    openLabel="GitHub"
                    loadLabel={locale === "zh" ? "加载技能" : "Load Skills"}
                    loadingLabel={locale === "zh" ? "加载中" : "Loading"}
                    loadedLabel={locale === "zh" ? "已加载" : "Loaded"}
                    onOpen={handleOpenRecommendedRepo}
                    onLoad={handleLoadRecommendedRepo}
                  />
                ))}
              </div>
            </div>

            {/* Existing sources list */}
            {customSources.length > 0 && (
              <div style={{ marginBottom: 20 }}>
                <span className="field-label">{locale === "zh" ? "已添加的源" : "Added Sources"}</span>
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {customSources.map((source, idx) => (
                    <MarketplaceCustomSourceRow
                      key={source.url}
                      index={idx}
                      url={source.url}
                      count={source.count}
                      countLabel={locale === "zh" ? "个技能" : "skills"}
                      removeTitle={locale === "zh" ? "移除" : "Remove"}
                      onRemove={removeCustomSource}
                    />
                  ))}
                </div>
              </div>
            )}

            {/* Add new source */}
            <div>
              <span className="field-label">{locale === "zh" ? "添加新源" : "Add New Source"}</span>
              <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
                {locale === "zh"
                  ? "输入一个返回技能列表 JSON 的 URL"
                  : "Enter a URL that returns a JSON array of skill entries"}
              </p>
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  className="input"
                  style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12, flex: 1 }}
                  placeholder="https://example.com/skills.json"
                  value={customUrl}
                  onChange={handleCustomUrlChange}
                  onKeyDown={handleCustomUrlKeyDown}
                />
                <button className="btn btn-primary btn-sm" onClick={handleCustomSource} disabled={!customUrl.trim() || loadingCustom} style={{ flexShrink: 0 }}>
                  {loadingCustom ? (
                    <><div className="spinner" style={{ width: 14, height: 14 }} />{locale === "zh" ? "加载中" : "Loading"}</>
                  ) : (
                    <><Plus size={13} />{locale === "zh" ? "添加" : "Add"}</>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Skill Preview Modal */}
      {previewSkill && (
        <div style={{ position: "fixed", inset: 0, background: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
          onClick={() => setPreviewSkill(null)}>
          <div className="section-card" style={{ width: 720, maxWidth: 720, maxHeight: "85vh", display: "flex", flexDirection: "column" }}
            onClick={e => e.stopPropagation()}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 18 }}>
              <div>
                <h3 style={{ fontSize: 16, fontWeight: 700 }}>{previewSkill.name}</h3>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
                  {showTranslation && locale === "zh" && previewSkill.description_zh ? previewSkill.description_zh : previewSkill.description}
                </p>
              </div>
              <button className="btn btn-ghost btn-icon-sm" onClick={() => setPreviewSkill(null)}><X size={16} /></button>
            </div>
            {/* Tags & meta */}
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 16 }}>
              <span className="badge badge-muted">{previewSkill.category}</span>
              {previewSkill.author && <span className="badge badge-muted">{previewSkill.author}</span>}
              {previewSkill.tags.map(tag => (
                <span key={tag} className="badge badge-muted" style={{ fontSize: 10 }}>{tag}</span>
              ))}
              {previewSkill.github_url && (
                <button className="badge badge-accent"
                  style={{ fontSize: 10, display: "flex", alignItems: "center", gap: 3, cursor: "pointer", border: "none", background: "var(--accent-subtle)" }}
                  onClick={() => shellOpen(previewSkill.github_url!)}>
                  <ExternalLink size={10} />GitHub
                </button>
              )}
            </div>
            {/* Content preview */}
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
              <span className="field-label" style={{ marginBottom: 0 }}>{locale === "zh" ? "技能内容" : "Content"}</span>
              {installedSkills.has(previewSkill.name.toLowerCase()) && (
                <button className="btn btn-secondary btn-xs" onClick={() => void startSkillEdit(previewSkill)} style={{ gap: 5 }}>
                  <Edit3 size={12} />{locale === "zh" ? "编辑" : "Edit"}
                </button>
              )}
            </div>
            <div className="markdown-preview" style={{ flex: 1, overflowY: "auto", fontSize: 13, lineHeight: 1.8, minHeight: 200 }}>
              <Markdown remarkPlugins={[remarkGfm]}>{previewSkill.content}</Markdown>
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
              <button className="btn btn-secondary btn-sm" onClick={() => setPreviewSkill(null)}>{i.common.cancel}</button>
              {installedSkills.has(previewSkill.name.toLowerCase()) ? (
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <button className="btn btn-danger-ghost btn-sm" onClick={() => { handleUninstallSkill(previewSkill); setPreviewSkill(null); }} style={{ gap: 5 }}>
                    <Trash2 size={13} />{locale === "zh" ? "卸载" : "Uninstall"}
                  </button>
                  <span className="badge badge-success" style={{ display: "flex", alignItems: "center", gap: 4, padding: "6px 12px" }}>
                    <CheckCircle size={12} />{i.marketplace.installed}
                  </span>
                </div>
              ) : (
                <button className="btn btn-primary btn-sm" onClick={() => { handleInstallSkill(previewSkill); setPreviewSkill(null); }}>
                  <Download size={13} />{i.marketplace.install}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
      {/* MCP Preview Modal */}
      {previewMcp && (() => {
        const isInstalled = installedIds.has(previewMcp.name) || installedIds.has(previewMcp.id);
        return (
          <div style={{ position: "fixed", inset: 0, background: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}
            onClick={() => setPreviewMcp(null)}>
            <div className="section-card" style={{ width: 560, maxHeight: "80vh", overflowY: "auto" }}
              onClick={e => e.stopPropagation()}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 18 }}>
                <h3 style={{ fontSize: 16, fontWeight: 700 }}>{previewMcp.name}</h3>
                <button className="btn btn-ghost btn-icon-sm" onClick={() => setPreviewMcp(null)}><X size={16} /></button>
              </div>
              <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 16, lineHeight: 1.6 }}>
                {previewMcp.description}
              </p>
              <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 16 }}>
                <span className="badge badge-muted">{locale === "zh" ? (MCP_CATEGORY_ZH[previewMcp.category] || previewMcp.category) : previewMcp.category}</span>
                {previewMcp.install_type && <span className="badge badge-muted">{previewMcp.install_type}</span>}
                {previewMcp.package_name && <span className="badge badge-accent" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 10 }}>{previewMcp.package_name}</span>}
              </div>
              {/* Command info */}
              <div style={{ marginBottom: 16 }}>
                <span className="field-label">{locale === "zh" ? "安装命令" : "Command"}</span>
                <div style={{ padding: "10px 14px", borderRadius: 6, background: "var(--bg-input)", border: "1px solid var(--border-default)", fontFamily: "'JetBrains Mono', monospace", fontSize: 12, color: "var(--text-secondary)", wordBreak: "break-all" }}>
                  {previewMcp.command} {previewMcp.args.join(" ")}
                </div>
              </div>
              {previewMcp.env_keys.length > 0 && (
                <div style={{ marginBottom: 16 }}>
                  <span className="field-label">{locale === "zh" ? "需要的环境变量" : "Required Environment Variables"}</span>
                  <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                    {previewMcp.env_keys.map(key => (
                      <span key={key} className="badge badge-warning" style={{ fontSize: 10, display: "flex", alignItems: "center", gap: 3 }}>
                        <Key size={10} />{key}
                      </span>
                    ))}
                  </div>
                </div>
              )}
              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 8 }}>
                {previewMcp.github_url && (
                  <button className="btn btn-secondary btn-sm"
                    onClick={() => shellOpen(previewMcp.github_url!)}
                    style={{ gap: 5 }}>
                    <ExternalLink size={13} />GitHub
                  </button>
                )}
                <button className="btn btn-secondary btn-sm" onClick={() => setPreviewMcp(null)}>
                  {locale === "zh" ? "关闭" : "Close"}
                </button>
                {isInstalled ? (
                  <>
                    <button className="btn btn-secondary btn-sm" onClick={() => void startMcpEdit(previewMcp)} style={{ gap: 5 }}>
                      <Edit3 size={13} />{locale === "zh" ? "编辑" : "Edit"}
                    </button>
                    <span className="badge badge-success" style={{ display: "flex", alignItems: "center", gap: 4, padding: "6px 12px" }}>
                      <CheckCircle size={12} />{i.marketplace.installed}
                    </span>
                  </>
                ) : (
                  <button className="btn btn-primary btn-sm" onClick={() => { handleInstallMcp(previewMcp); setPreviewMcp(null); }} style={{ gap: 5 }}>
                    <Download size={13} />{i.marketplace.install}
                  </button>
                )}
              </div>
            </div>
          </div>
        );
      })()}
      <ConfirmDialog
        isOpen={!!pendingUninstall}
        title={locale === "zh" ? "卸载技能" : "Uninstall Skill"}
        message={locale === "zh" ? `确定卸载技能「${pendingUninstall?.name}」？` : `Uninstall skill "${pendingUninstall?.name}"?`}
        confirmText={locale === "zh" ? "卸载" : "Uninstall"}
        variant="destructive"
        onConfirm={() => { if (pendingUninstall) void doUninstallSkill(pendingUninstall); setPendingUninstall(null); }}
        onCancel={() => setPendingUninstall(null)}
      />
    </div>
  );
}

const EmptyState = memo(function EmptyState({ icon: Icon, text, sub }: { icon: typeof Store; text: string; sub: string }) {
  return (
    <div className="card empty-state">
      <div className="empty-icon"><Icon size={28} style={{ color: "var(--text-muted)" }} /></div>
      <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{text}</p>
      <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8 }}>{sub}</p>
    </div>
  );
});
