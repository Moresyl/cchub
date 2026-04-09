import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type { FolderNode } from "../types/skills";

export interface DetectedToolQueryResult {
  id: string;
  name: string;
  installed: boolean;
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

export const queryKeys = {
  detectTools: ["detect-tools"] as const,
  profiles: ["config-profiles"] as const,
  hudStatus: ["claude-hud-status"] as const,
  hello2ccStatus: ["hello2cc-status"] as const,
  configFiles: (rootId: string) => ["config-file-tree", rootId] as const,
};

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
