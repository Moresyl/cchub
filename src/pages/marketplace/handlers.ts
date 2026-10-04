/* eslint-disable @typescript-eslint/no-explicit-any */
import { invoke } from "@tauri-apps/api/core";
import type { QueryClient } from "@tanstack/react-query";

import { showToast } from "../../components/Toast";
import { fetchMarketplaceLocalData, fetchMarketplaceSearchPage, queryKeys } from "../../hooks/queries";
import type { DetectedTool } from "../../types/skills";

import type { InstalledMcpServer, InstalledSkillRecord, RegistryEntry, SkillEntry } from "./helpers";
import { resolveInstalledMcp } from "./mcpSource";

export interface InstallMcpContext {
  entry: RegistryEntry;
  envVals: Record<string, string>;
  activeTool: string;
  locale: string;
  setInstalling: (v: string | null) => void;
  setShowEnvModal: (v: RegistryEntry | null) => void;
  setInstalledIdsByTool: (updater: (prev: Record<string, Set<string>>) => Record<string, Set<string>>) => void;
}

export async function performInstallMcp(ctx: InstallMcpContext): Promise<void> {
  const { entry, envVals, activeTool, locale } = ctx;
  ctx.setInstalling(entry.id);
  ctx.setShowEnvModal(null);
  try {
    await invoke("install_from_marketplace", {
      name: entry.name,
      command: entry.command,
      args: entry.args,
      envValues: envVals,
      targetTool: activeTool || "claude",
    });
    ctx.setInstalledIdsByTool((prev) => {
      const next = { ...prev };
      const set = new Set(next[activeTool] ?? []);
      set.add(entry.name);
      set.add(entry.id);
      next[activeTool] = set;
      return next;
    });
  } catch (e) {
    console.error(e);
    showToast("error", locale === "zh" ? "安装失败" : "Installation failed");
  } finally {
    ctx.setInstalling(null);
  }
}

export interface InstallSkillContext {
  skill: SkillEntry;
  activeTool: string;
  locale: string;
  tools: DetectedTool[];
  setInstalling: (v: string | null) => void;
  setInstalledSkillsByTool: (updater: (prev: Record<string, Set<string>>) => Record<string, Set<string>>) => void;
}

export async function performInstallSkill(ctx: InstallSkillContext): Promise<void> {
  const { skill, activeTool, locale, tools } = ctx;
  ctx.setInstalling(skill.id);
  try {
    const tool = tools.find((t) => t.id === activeTool);
    await invoke<string>("install_skill_from_marketplace", {
      name: skill.id,
      content: skill.content,
      description: skill.description,
      triggerCommand: null,
      sourceUrl: skill.github_url,
      targetDir: tool?.skills_dir ?? null,
    });
    ctx.setInstalledSkillsByTool((prev) => {
      const next = { ...prev };
      const set = new Set(next[activeTool] ?? []);
      set.add(skill.name.toLowerCase());
      next[activeTool] = set;
      return next;
    });
  } catch (e) {
    console.error(e);
    showToast("error", locale === "zh" ? "安装失败" : "Installation failed");
  } finally {
    ctx.setInstalling(null);
  }
}

export interface UninstallSkillContext {
  skill: SkillEntry;
  activeTool: string;
  locale: string;
  editingSkill: SkillEntry | null;
  setInstalledSkillsByTool: (updater: (prev: Record<string, Set<string>>) => Record<string, Set<string>>) => void;
  setSkillEntries: (updater: (prev: SkillEntry[]) => SkillEntry[]) => void;
  setEditingSkill: (v: SkillEntry | null) => void;
  setSkillContent: (v: string) => void;
  setEditSkillContent: (v: string) => void;
}

export async function performUninstallSkill(ctx: UninstallSkillContext): Promise<void> {
  const { skill, activeTool, locale, editingSkill } = ctx;
  try {
    const skills = await invoke<InstalledSkillRecord[]>("scan_skills");
    // 只删除当前工具下的同名 skill，避免误删用户在其他工具下保留的副本
    const installed = skills.find(
      (s) =>
        (s.name.toLowerCase() === skill.name.toLowerCase() || s.name.toLowerCase() === skill.id.toLowerCase()) &&
        (s.tool_id ?? "claude") === activeTool,
    );
    if (installed?.file_path) {
      await invoke("uninstall_skill_file", { path: installed.file_path });
      ctx.setInstalledSkillsByTool((prev) => {
        const next = { ...prev };
        if (next[activeTool]) {
          const set = new Set(next[activeTool]);
          set.delete(skill.name.toLowerCase());
          next[activeTool] = set;
        }
        return next;
      });
      const otherToolsHave = skills.some(
        (s) =>
          (s.name.toLowerCase() === skill.name.toLowerCase() || s.name.toLowerCase() === skill.id.toLowerCase()) &&
          (s.tool_id ?? "claude") !== activeTool,
      );
      if (!otherToolsHave) {
        ctx.setSkillEntries((prev) =>
          prev.map((item) =>
            item.id === skill.id || item.name.toLowerCase() === skill.name.toLowerCase()
              ? { ...item, file_path: null, content: "" }
              : item,
          ),
        );
      }
      if (editingSkill && editingSkill.name.toLowerCase() === skill.name.toLowerCase()) {
        ctx.setEditingSkill(null);
        ctx.setSkillContent("");
        ctx.setEditSkillContent("");
      }
    } else {
      showToast(
        "error",
        locale === "zh"
          ? `当前工具（${activeTool}）下未找到该技能文件`
          : `Skill file not found under active tool (${activeTool})`,
      );
    }
  } catch (e) {
    console.error(e);
  }
}

export interface SaveSkillContentContext {
  editingSkill: SkillEntry;
  editSkillContent: string;
  locale: string;
  setSkillContent: (v: string) => void;
  setEditingSkill: (updater: (prev: SkillEntry | null) => SkillEntry | null) => void;
  setSkillEntries: (updater: (prev: SkillEntry[]) => SkillEntry[]) => void;
}

export async function performSaveSkillContent(ctx: SaveSkillContentContext): Promise<void> {
  const { editingSkill, editSkillContent, locale } = ctx;
  if (!editingSkill?.file_path) return;
  try {
    await invoke("write_skill_content", { filePath: editingSkill.file_path, content: editSkillContent });
    ctx.setSkillContent(editSkillContent);
    ctx.setEditingSkill((prev) => (prev ? { ...prev, content: editSkillContent } : prev));
    ctx.setSkillEntries((prev) =>
      prev.map((s) =>
        s.id === editingSkill.id || s.name.toLowerCase() === editingSkill.name.toLowerCase()
          ? { ...s, file_path: editingSkill.file_path, content: editSkillContent }
          : s,
      ),
    );
    showToast("success", locale === "zh" ? "技能已保存" : "Skill saved");
  } catch (e) {
    console.error(e);
    showToast("error", locale === "zh" ? "技能保存失败" : "Failed to save skill");
  }
}

// 加载远程 GitHub skill 仓库到当前会话；同时记录到 customSources 里作为一个"已加载的源"。
export interface LoadRecommendedRepoContext {
  repoName: string;
  branch: string;
  locale: string;
  setLoadingRepo: (v: string | null) => void;
  setSkillEntries: (updater: (prev: SkillEntry[]) => SkillEntry[]) => void;
  setCustomSources: (
    updater: (
      prev: { url: string; count: number; skillIds: string[] }[],
    ) => { url: string; count: number; skillIds: string[] }[],
  ) => void;
}

export async function performLoadRecommendedRepo(ctx: LoadRecommendedRepoContext): Promise<void> {
  const { repoName, branch, locale } = ctx;
  const [owner, repo] = repoName.split("/");
  ctx.setLoadingRepo(repoName);
  try {
    const skills = await invoke<SkillEntry[]>("fetch_skills_from_repo", { owner, repo, branch });
    const newIds: string[] = [];
    ctx.setSkillEntries((prev) => {
      const existingIds = new Set(prev.map((s) => s.id));
      const newEntries = skills.filter((s) => !existingIds.has(s.id));
      newEntries.forEach((s) => newIds.push(s.id));
      return [...prev, ...newEntries];
    });
    ctx.setCustomSources((prev) => [...prev, { url: `github:${repoName}`, count: newIds.length, skillIds: newIds }]);
  } catch (e) {
    console.error(e);
    showToast("error", locale === "zh" ? `加载失败: ${e}` : `Load failed: ${e}`);
  } finally {
    ctx.setLoadingRepo(null);
  }
}

// 把用户输入的自定义 URL 加载成 skill 列表，并入会话状态。
export interface LoadCustomSourceContext {
  customUrl: string;
  locale: string;
  setLoadingCustom: (v: boolean) => void;
  setSkillEntries: (updater: (prev: SkillEntry[]) => SkillEntry[]) => void;
  setCustomSources: (
    updater: (
      prev: { url: string; count: number; skillIds: string[] }[],
    ) => { url: string; count: number; skillIds: string[] }[],
  ) => void;
  setCustomUrl: (v: string) => void;
}

export async function performLoadCustomSource(ctx: LoadCustomSourceContext): Promise<void> {
  const { customUrl, locale } = ctx;
  if (!customUrl.trim()) return;
  ctx.setLoadingCustom(true);
  try {
    const custom = await invoke<SkillEntry[]>("fetch_custom_skill_source", { url: customUrl.trim() });
    const newIds: string[] = [];
    ctx.setSkillEntries((prev) => {
      const existingIds = new Set(prev.map((s) => s.id));
      const newEntries = custom.filter((s) => !existingIds.has(s.id));
      newEntries.forEach((s) => newIds.push(s.id));
      return [...prev, ...newEntries];
    });
    ctx.setCustomSources((prev) => [...prev, { url: customUrl.trim(), count: newIds.length, skillIds: newIds }]);
    ctx.setCustomUrl("");
  } catch (e) {
    console.error(e);
    showToast("error", locale === "zh" ? "加载失败，请检查 URL 格式" : "Failed to load. Check URL format.");
  } finally {
    ctx.setLoadingCustom(false);
  }
}

// MCP/Skill 翻页搜索 — 抽出避免主体被两段 paging 代码塞满。
export interface MarketplacePageContext {
  queryClient: QueryClient;
  search: string;
  setLoadingMore: (v: boolean) => void;
  setEntries: (v: RegistryEntry[]) => void;
  setMcpPage: (v: number) => void;
  setMcpTotal: (v: number) => void;
}

export async function loadMcpPage(ctx: MarketplacePageContext, page: number): Promise<void> {
  ctx.setLoadingMore(true);
  try {
    const result = await ctx.queryClient.fetchQuery({
      queryKey: queryKeys.marketplaceSearch(ctx.search || "mcp server", page, 50),
      queryFn: () => fetchMarketplaceSearchPage(ctx.search || "mcp server", page, 50),
      staleTime: 0,
    });
    ctx.setEntries(result.entries);
    ctx.setMcpPage(page);
    ctx.setMcpTotal(result.total);
  } catch (e) {
    console.error(e);
  } finally {
    ctx.setLoadingMore(false);
  }
}

// 关闭编辑器后回填新的 MCP 详情：从同一个 query 拿一遍最新值。
export interface RefreshInstalledMcpContext {
  queryClient: QueryClient;
  setInstalledMcpDetails: (v: InstalledMcpServer[]) => void;
  rebuildMcpByTool: (servers: InstalledMcpServer[]) => Promise<Record<string, Set<string>>>;
  setInstalledIdsByTool: (v: Record<string, Set<string>>) => void;
}

export async function performRefreshInstalledMcpDetails(
  ctx: RefreshInstalledMcpContext,
): Promise<InstalledMcpServer[]> {
  const { servers } = await ctx.queryClient.fetchQuery({
    queryKey: queryKeys.marketplaceLocal,
    queryFn: fetchMarketplaceLocalData,
    staleTime: 0,
  });
  ctx.setInstalledMcpDetails(servers);
  ctx.setInstalledIdsByTool(await ctx.rebuildMcpByTool(servers));
  return servers;
}

// 卸载已安装的 MCP 服务：仅对当前 tool 解绑，不动其他工具下的同名安装。
export interface UninstallMcpContext {
  entry: RegistryEntry;
  activeTool: string;
  locale: string;
  refreshInstalledMcpDetails: () => Promise<InstalledMcpServer[]>;
  setInstalledIdsByTool: (updater: (prev: Record<string, Set<string>>) => Record<string, Set<string>>) => void;
}

export async function performUninstallMcp(ctx: UninstallMcpContext): Promise<void> {
  const { entry, activeTool, locale } = ctx;
  try {
    const server = await resolveInstalledMcp(entry, activeTool, await ctx.refreshInstalledMcpDetails());
    await invoke("unsync_mcp_server_from_tool", { serverName: server.id, targetTool: activeTool });
    ctx.setInstalledIdsByTool((prev) => {
      const next = { ...prev };
      if (next[activeTool]) {
        const set = new Set(next[activeTool]);
        set.delete(entry.name);
        set.delete(entry.id);
        next[activeTool] = set;
      }
      return next;
    });
    showToast(
      "success",
      locale === "zh" ? `已从 ${activeTool} 卸载 ${entry.name}` : `Removed ${entry.name} from ${activeTool}`,
    );
  } catch (e) {
    console.error(e);
    showToast("error", locale === "zh" ? `卸载失败: ${e}` : `Uninstall failed: ${e}`);
  }
}
