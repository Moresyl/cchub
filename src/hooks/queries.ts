import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { fetchVisibleApps, type ManagedAppId } from "../lib/appPreferences";
import { queryClient } from "../lib/queryClient";
import type { FolderNode } from "../types/skills";

export interface DetectedToolQueryResult {
  id: string;
  name: string;
  config_path: string;
  skills_dir: string;
  mcp_config_path: string;
  installed: boolean;
  install_command: string;
  install_url: string;
}

export interface ConfigProfileQueryResult {
  id: string;
  name: string;
  tool_id: string;
  config_snapshot: string;
  sort_order: number;
  source_type?: string | null;
  source_key?: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface HudStatusQueryResult {
  installed: boolean;
  version: string;
  indexJsPath: string;
  statuslineEnabled: boolean;
  hudConfig: Record<string, unknown>;
}

export interface Hello2ccConfigQueryResult {
  routing_policy: string;
  mirror_session_model: boolean;
  default_agent_model: string;
  primary_model: string;
  subagent_model: string;
  guide_model: string;
  explore_model: string;
  plan_model: string;
  general_model: string;
  team_model: string;
  compatibility_mode: string;
}

export interface Hello2ccStatusQueryResult {
  installed: boolean;
  enabled: boolean;
  version: string;
  installPath: string;
  settingsPath: string;
  config: Hello2ccConfigQueryResult;
}

export interface ProviderConfigFragmentQueryResult {
  id: string;
  name: string;
  targetTools: string[];
  fields: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

export interface McpServerQueryResult {
  id: string;
  name: string;
  command: string | null;
  args: string;
  env: string;
  status: string;
  transport: string;
  source: string;
  package_name: string | null;
  version: string | null;
  config_path: string | null;
}

export interface PluginQueryResult {
  id: string;
  name: string;
  description: string | null;
  source_url: string | null;
  version: string | null;
}

export interface SkillQueryResult {
  id: string;
  name: string;
  description: string | null;
  tool_id: string | null;
  plugin_id: string | null;
  trigger_command: string | null;
  file_path: string | null;
  source_url: string | null;
  baseline_sha256: string | null;
  latest_sha256: string | null;
  last_checked_at: number | null;
  current_sha256: string | null;
}

export interface SkillBackupQueryResult {
  id: string;
  skill_name: string;
  original_path: string;
  backup_path: string;
  created_at: string;
  size_bytes: number;
}

export interface ClaudeMdFileQueryResult {
  path: string;
  project_name: string;
  size_bytes: number;
  modified_at: string | null;
  content_preview: string;
  disabled: boolean;
  tool_name: string;
  file_name: string;
  scope: string;
}

export interface ClaudeMdTemplateQueryResult {
  id: string;
  name: string;
  description: string;
  content: string;
  file_name: string;
  tool_name: string;
}

export interface PromptPresetQueryResult {
  id: string;
  name: string;
  content: string;
  created_at: string;
  updated_at: string;
}

export interface PromptPresetStateQueryResult {
  presets: PromptPresetQueryResult[];
  active_preset_id: string | null;
}

export interface MarketplaceSkillQueryResult {
  id: string;
  name: string;
  description: string;
  description_zh: string | null;
  category: string;
  author: string | null;
  github_url: string | null;
  cover_url: string | null;
  tags: string[];
  content: string;
  file_path: string | null;
}

export interface MarketplaceSearchEntryQueryResult {
  id: string;
  name: string;
  description: string;
  category: string;
  install_type: string;
  package_name: string | null;
  github_url: string | null;
  command: string;
  args: string[];
  env_keys: string[];
  source: string;
}

export interface MarketplaceSearchResultQueryResult {
  entries: MarketplaceSearchEntryQueryResult[];
  total: number;
}

export interface MarketplaceCatalogQueryResult {
  skills: MarketplaceSkillQueryResult[];
  total: number;
}

export interface ToolSettingsQueryResult {
  permissionsLevel: number;
  autoUpdateChannel: string;
  claudeModel: string;
  toolSearchEnabled: boolean;
  codexSettings: {
    approval_mode: string;
    reasoning_effort: string;
    disable_response_storage: boolean;
    context_window_1m: boolean;
  };
  visibleApps: ManagedAppId[];
}

export interface SessionSummaryQueryResult {
  id: string;
  tool_id: string;
  tool_name: string;
  title: string;
  cwd: string | null;
  source_kind: string;
  source_backend: string;
  source_path: string;
  created_at: string | null;
  updated_at: string | null;
  preview: string;
  message_count: number;
  input_tokens: number | null;
  output_tokens: number | null;
  tokens_used: number | null;
  search_hit_count: number;
  can_resume: boolean;
  can_delete: boolean;
}

export interface ActivityItemQueryResult {
  id: number;
  server_id: string;
  server_name: string;
  request_type: string;
  status: string;
  latency_ms: number | null;
  recorded_at: string;
}

export interface ProxyUsageSummaryQueryResult {
  total_requests: number;
  success_requests: number;
  success_rate: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cache_read_tokens: number;
  total_cache_creation_tokens: number;
  total_cost_usd: string;
}

export interface ProxyRequestLogRowQueryResult {
  request_id: string;
  tool_id: string;
  profile_id: string;
  provider_name: string;
  request_model: string | null;
  response_model: string | null;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  total_cost_usd: string;
  latency_ms: number;
  status_code: number;
  is_streaming: boolean;
  error_message: string | null;
  created_at: string;
}

export interface ModelPricingRowQueryResult {
  model_id: string;
  normalized_model_id: string;
  input_cost_per_million: string;
  output_cost_per_million: string;
  cache_read_cost_per_million: string;
  cache_write_cost_per_million: string;
  created_at: string;
  updated_at: string;
}

export const queryKeys = {
  detectTools: ["detect-tools"] as const,
  profiles: ["config-profiles"] as const,
  providerFragments: ["provider-config-fragments"] as const,
  activeProfileIds: ["active-config-profile-ids"] as const,
  profilesPage: ["profiles-page"] as const,
  mcpServers: ["scan-mcp-servers"] as const,
  mcpServersPage: ["mcp-servers-page"] as const,
  plugins: ["plugins"] as const,
  skills: ["scan-skills"] as const,
  skillBackups: ["skill-backups"] as const,
  skillSyncMethod: ["skill-sync-method"] as const,
  visibleApps: ["visible-apps"] as const,
  skillsPage: ["skills-page"] as const,
  claudeMd: ["scan-claude-md"] as const,
  claudeMdTemplates: ["claude-md-templates"] as const,
  promptPresets: ["prompt-presets"] as const,
  claudeMdPage: ["claude-md-page"] as const,
  toolsPage: ["tools-page"] as const,
  marketplaceLocal: ["marketplace-local"] as const,
  marketplaceCatalog: (page = 1, limit = 50, category = "") =>
    ["skillhub-catalog", page, limit, category] as const,
  marketplaceSearch: (query: string, page = 0, pageSize = 50) =>
    ["search-marketplace", query, page, pageSize] as const,
  sessions: (toolId: string | null) => ["sessions", toolId ?? "all"] as const,
  logsPage: (date: string) => ["logs-page", date] as const,
  hudStatus: ["claude-hud-status"] as const,
  hello2ccStatus: ["hello2cc-status"] as const,
  configFiles: (rootId: string) => ["config-file-tree", rootId] as const,
};

function withTimeout<T>(label: string, promise: Promise<T>, timeoutMs = 8_000) {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => {
      reject(new Error(`${label} timed out after ${timeoutMs}ms`));
    }, timeoutMs);

    promise
      .then((value) => {
        window.clearTimeout(timer);
        resolve(value);
      })
      .catch((error) => {
        window.clearTimeout(timer);
        reject(error);
      });
  });
}

export function fetchVisibleAppsQuery() {
  return fetchVisibleApps();
}

export async function fetchProfilesPageData() {
  await invoke("sync_config_profiles");
  const [profiles, tools, activeIds, providerFragments] = await Promise.all([
    invoke<ConfigProfileQueryResult[]>("get_config_profiles"),
    invoke<DetectedToolQueryResult[]>("detect_tools"),
    invoke<string[]>("get_active_config_profile_ids"),
    invoke<ProviderConfigFragmentQueryResult[]>("get_provider_config_fragments").catch(
      () => [] as ProviderConfigFragmentQueryResult[],
    ),
  ]);

  return {
    profiles,
    tools,
    activeIds,
    providerFragments,
  };
}

export async function fetchMcpServersPageData() {
  const [servers, tools] = await Promise.all([
    invoke<McpServerQueryResult[]>("scan_mcp_servers"),
    invoke<DetectedToolQueryResult[]>("detect_tools"),
  ]);

  return { servers, tools };
}

export async function fetchSkillsPageData() {
  const [skills, plugins, tools, skillSyncMethod, visibleApps, skillBackups] =
    await Promise.all([
      invoke<SkillQueryResult[]>("scan_skills"),
      invoke<PluginQueryResult[]>("get_plugins"),
      invoke<DetectedToolQueryResult[]>("detect_tools"),
      invoke<string>("get_skill_sync_method").catch(() => "copy"),
      fetchVisibleAppsQuery(),
      invoke<SkillBackupQueryResult[]>("get_skill_backups").catch(
        () => [] as SkillBackupQueryResult[],
      ),
    ]);

  return {
    skills,
    plugins,
    tools,
    skillSyncMethod,
    visibleApps,
    skillBackups,
  };
}

export async function fetchToolsPageData(): Promise<ToolSettingsQueryResult> {
  const [
    permissionsLevel,
    autoUpdateChannel,
    claudeModel,
    toolSearchEnabled,
    codexSettings,
    visibleApps,
  ] = await Promise.all([
    invoke<number>("get_claude_permissions_level").catch(() => 0),
    invoke<string>("get_claude_auto_update").catch(() => "latest"),
    invoke<string>("get_claude_model").catch(() => ""),
    invoke<boolean>("get_claude_tool_search").catch(() => false),
    invoke<{
      approval_mode: string;
      reasoning_effort: string;
      disable_response_storage: boolean;
      context_window_1m: boolean;
    }>("get_codex_settings").catch(() => ({
      approval_mode: "suggest",
      reasoning_effort: "medium",
      disable_response_storage: false,
      context_window_1m: false,
    })),
    fetchVisibleAppsQuery(),
  ]);

  return {
    permissionsLevel,
    autoUpdateChannel,
    claudeModel,
    toolSearchEnabled,
    codexSettings,
    visibleApps,
  };
}

export async function fetchClaudeMdPageData() {
  const [files, templates, visibleApps, presetState] = await Promise.all([
    invoke<ClaudeMdFileQueryResult[]>("scan_claude_md"),
    invoke<ClaudeMdTemplateQueryResult[]>("get_claude_md_templates"),
    fetchVisibleAppsQuery(),
    invoke<PromptPresetStateQueryResult>("get_prompt_presets").catch(() => ({
      presets: [],
      active_preset_id: null,
    })),
  ]);

  return {
    files,
    templates,
    visibleApps,
    presetState,
  };
}

export async function fetchMarketplaceLocalData() {
  // 复用 mcpServersPage / skillsPage 的缓存，避免重复扫描 scan_mcp_servers / scan_skills
  const [mcpPage, skillsPage] = await Promise.all([
    queryClient.ensureQueryData({
      queryKey: queryKeys.mcpServersPage,
      queryFn: fetchMcpServersPageData,
    }),
    queryClient.ensureQueryData({
      queryKey: queryKeys.skillsPage,
      queryFn: fetchSkillsPageData,
    }),
  ]);

  return { servers: mcpPage.servers, installedSkills: skillsPage.skills };
}

export function fetchMarketplaceCatalogPage(page = 1, limit = 50, category = "") {
  return withTimeout(
    "get_skillhub_catalog",
    invoke<MarketplaceCatalogQueryResult>("get_skillhub_catalog", { page, limit, category }),
  );
}

export function fetchMarketplaceSearchPage(query: string, page = 0, pageSize = 50) {
  return withTimeout(
    "search_marketplace",
    invoke<MarketplaceSearchResultQueryResult>("search_marketplace", {
      query,
      page,
      pageSize,
    }),
  );
}

export function fetchSessionsPageData(toolId: string | null) {
  return invoke<SessionSummaryQueryResult[]>("get_sessions", {
    toolId,
    query: null,
    limit: 240,
  });
}

export async function fetchLogsPageData(date: string) {
  const [activities, proxySummary, recentProxyLogs, modelPricingRows] = await Promise.all([
    invoke<ActivityItemQueryResult[]>("get_activity_logs", { date }),
    invoke<ProxyUsageSummaryQueryResult>("get_proxy_usage_summary"),
    invoke<ProxyRequestLogRowQueryResult[]>("get_recent_proxy_request_logs", {
      limit: 240,
    }),
    invoke<ModelPricingRowQueryResult[]>("list_model_pricing"),
  ]);

  return {
    activities,
    proxySummary,
    recentProxyLogs,
    modelPricingRows,
  };
}

export function useDetectTools() {
  return useQuery({
    queryKey: queryKeys.detectTools,
    queryFn: () => invoke<DetectedToolQueryResult[]>("detect_tools"),
    staleTime: 30_000,
  });
}

export function useProfiles(enabled = true) {
  return useQuery({
    queryKey: queryKeys.profiles,
    queryFn: () => invoke<ConfigProfileQueryResult[]>("get_config_profiles"),
    staleTime: 10_000,
    enabled,
  });
}

export function useConfigFiles(rootId: string, enabled = true) {
  return useQuery({
    queryKey: queryKeys.configFiles(rootId),
    queryFn: () => invoke<FolderNode>("get_config_file_tree", { rootId }),
    staleTime: 5_000,
    enabled: enabled && Boolean(rootId),
  });
}

export function useHudStatus() {
  return useQuery({
    queryKey: queryKeys.hudStatus,
    queryFn: () => invoke<HudStatusQueryResult>("get_claude_hud_status"),
    staleTime: 60_000,
  });
}

export function useHello2ccStatus() {
  return useQuery({
    queryKey: queryKeys.hello2ccStatus,
    queryFn: () => invoke<Hello2ccStatusQueryResult>("get_hello2cc_status"),
    staleTime: 60_000,
  });
}
