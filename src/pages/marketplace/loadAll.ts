/* eslint-disable @typescript-eslint/no-explicit-any */
import { invoke } from "@tauri-apps/api/core";
import type { QueryClient } from "@tanstack/react-query";

import {
  fetchMarketplaceCatalogPage,
  fetchMarketplaceLocalData,
  fetchMarketplaceSearchPage,
  fetchSkillsPageData,
  queryKeys,
} from "../../hooks/queries";
import { type ManagedAppId } from "../../lib/appPreferences";
import type { DetectedTool } from "../../types/skills";

import {
  dedupByName,
  type InstalledMcpServer,
  type InstalledSkillRecord,
  type RegistryEntry,
  type SkillEntry,
} from "./helpers";

// loadAll 把市场首屏需要的所有数据拉到一起：本地 MCP + 本地技能 + 远程 catalog + 远程搜索 + 工具栏元数据。
// 拆出来主要是把 1500+ 行的 Marketplace.tsx 主体瘦身。
export interface MarketplaceLoadAllOptions {
  force?: boolean;
}

export interface MarketplaceLoadAllSetters {
  setLoading: (v: boolean) => void;
  setLoadError: (v: string | null) => void;
  setEntries: (updater: RegistryEntry[] | ((prev: RegistryEntry[]) => RegistryEntry[])) => void;
  setSkillEntries: (v: SkillEntry[]) => void;
  setInstalledMcpDetails: (v: InstalledMcpServer[]) => void;
  setInstalledIdsByTool: (v: Record<string, Set<string>>) => void;
  setInstalledSkillsByTool: (v: Record<string, Set<string>>) => void;
  setTools: (v: DetectedTool[]) => void;
  setVisibleApps: (v: ManagedAppId[]) => void;
  setActiveTool: (updater: (prev: string) => string) => void;
  setMcpTotal: (v: number) => void;
  setMcpPage: (v: number) => void;
  rebuildSkillsByTool: (records: InstalledSkillRecord[]) => Record<string, Set<string>>;
  rebuildMcpByTool: (serverNames: string[]) => Promise<Record<string, Set<string>>>;
}

export async function performMarketplaceLoadAll(
  queryClient: QueryClient,
  options: MarketplaceLoadAllOptions,
  setters: MarketplaceLoadAllSetters,
): Promise<void> {
  const { force = false } = options;
  if (!queryClient.getQueryData(queryKeys.marketplaceLocal)) {
    setters.setLoading(true);
  }
  setters.setLoadError(null);
  try {
    const localData = await queryClient.fetchQuery({
      queryKey: queryKeys.marketplaceLocal,
      queryFn: fetchMarketplaceLocalData,
      staleTime: force ? 0 : 30_000,
    });
    const localSkillEntries: SkillEntry[] = dedupByName(localData.installedSkills).map((s) => ({
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
    const scannedEntries: RegistryEntry[] = dedupByName(localData.servers).map((s) => ({
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
    setters.setInstalledMcpDetails(localData.servers);
    // 先给 Claude 一份基础集合，避免 UI 在等待 per-tool 探测时空一帧
    setters.setInstalledIdsByTool({
      claude: new Set(localData.servers.flatMap((s) => [s.id, s.name])),
    });
    setters.setEntries(scannedEntries);

    try {
      const allSkills = await invoke<InstalledSkillRecord[]>("scan_skills");
      setters.setInstalledSkillsByTool(setters.rebuildSkillsByTool(allSkills));
    } catch (err) {
      console.warn("scan_skills failed; falling back to Claude-only set", err);
      setters.setInstalledSkillsByTool({
        claude: new Set(localData.installedSkills.map((s) => s.name.toLowerCase())),
      });
    }

    void setters.rebuildMcpByTool(localData.servers.map((s) => s.name)).then((map) => {
      if (Object.keys(map).length > 0) setters.setInstalledIdsByTool(map);
    });

    try {
      const skillsPageData = await queryClient.fetchQuery({
        queryKey: queryKeys.skillsPage,
        queryFn: fetchSkillsPageData,
        staleTime: force ? 0 : 30_000,
      });
      setters.setTools(skillsPageData.tools);
      setters.setVisibleApps(skillsPageData.visibleApps);
      const firstInstalled = skillsPageData.tools.find(
        (t) => t.installed && skillsPageData.visibleApps.includes(t.id as ManagedAppId),
      );
      if (firstInstalled) {
        setters.setActiveTool((prev) => {
          const prevTool = skillsPageData.tools.find((t) => t.id === prev);
          const prevVisible = skillsPageData.visibleApps.includes(prev as ManagedAppId);
          return prevTool?.installed && prevVisible ? prev : firstInstalled.id;
        });
      }
    } catch (err) {
      console.warn("fetchSkillsPageData failed for marketplace tool tabs", err);
    }

    const catalogPromise = queryClient.fetchQuery({
      queryKey: queryKeys.marketplaceCatalog(),
      queryFn: () => fetchMarketplaceCatalogPage(),
      staleTime: force ? 0 : 30_000,
    });
    const searchPromise = queryClient.fetchQuery({
      queryKey: queryKeys.marketplaceSearch("mcp server"),
      queryFn: () => fetchMarketplaceSearchPage("mcp server"),
      staleTime: force ? 0 : 30_000,
    });
    const repositorySkillsPromise = invoke<SkillEntry[]>("discover_available_skills");

    const [catalogResult, searchResult, repositorySkillsResult] = await Promise.allSettled([
      catalogPromise,
      searchPromise,
      repositorySkillsPromise,
    ]);
    const repositorySkills = repositorySkillsResult.status === "fulfilled" ? repositorySkillsResult.value : [];
    if (repositorySkillsResult.status === "rejected") {
      console.warn("Configured skill repositories unavailable:", repositorySkillsResult.reason);
    }

    if (catalogResult.status === "fulfilled") {
      const marketSkills = catalogResult.value.skills || [];
      const merged = dedupByName([...localSkillEntries, ...repositorySkills, ...marketSkills]);
      setters.setSkillEntries(merged);
    } else {
      console.warn("SkillHub API failed, showing local and configured repository skills:", catalogResult.reason);
      setters.setSkillEntries(dedupByName([...localSkillEntries, ...repositorySkills]));
    }

    if (searchResult.status === "fulfilled") {
      setters.setEntries((prev) => {
        const ids = new Set(prev.map((entry) => entry.id));
        return [...prev, ...searchResult.value.entries.filter((entry) => !ids.has(entry.id))];
      });
      setters.setMcpTotal(searchResult.value.total);
      setters.setMcpPage(0);
    }
  } catch (e) {
    console.error(e);
    setters.setLoadError(String(e));
  } finally {
    setters.setLoading(false);
  }
}
