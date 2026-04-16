import { invoke } from "@tauri-apps/api/core";

export const MANAGED_APPS = ["claude", "codex", "gemini", "opencode", "openclaw", "hermes"] as const;

export type ManagedAppId = (typeof MANAGED_APPS)[number];

export interface WindowPreferences {
  launch_at_login: boolean;
  launch_hidden: boolean;
  close_to_tray: boolean;
  lightweight_mode: boolean;
}

export interface TerminalOption {
  id: string;
  label: string;
  command: string;
  installed: boolean;
}

export interface TerminalPreferences {
  platform: string;
  selected_terminal: string;
  options: TerminalOption[];
}

export interface EnvironmentConflict {
  id: string;
  kind: string;
  variables: string[];
  affected_apps: string[];
}

export interface LogPreferences {
  level: string;
}

export interface LogFileTargets {
  runtime_log_path: string;
  crash_log_path: string;
}

export interface LocalProviderProxySettings {
  port: number;
  enabled_apps: string[];
}

export interface LocalProviderProxyStatus {
  running: boolean;
  host: string;
  port: number;
  base_url: string;
  enabled_apps: string[];
}

export const DEFAULT_VISIBLE_APPS: ManagedAppId[] = [...MANAGED_APPS];

export function isManagedAppId(value: string): value is ManagedAppId {
  return MANAGED_APPS.includes(value as ManagedAppId);
}

export function normalizeVisibleApps(value: string[] | null | undefined): ManagedAppId[] {
  const next = (value || []).filter(isManagedAppId);
  const deduped = Array.from(new Set(next));
  return deduped.length > 0 ? deduped : [...DEFAULT_VISIBLE_APPS];
}

export function getAppLabel(appId: ManagedAppId): string {
  switch (appId) {
    case "claude":
      return "Claude";
    case "codex":
      return "Codex";
    case "gemini":
      return "Gemini";
    case "opencode":
      return "OpenCode";
    case "openclaw":
      return "OpenClaw";
    case "hermes":
      return "Hermes";
  }
}

export function toolNameToAppId(toolName: string): ManagedAppId | null {
  const normalized = toolName.trim().toLowerCase();
  if (normalized === "claude") return "claude";
  if (normalized === "codex") return "codex";
  if (normalized === "gemini") return "gemini";
  if (normalized === "opencode") return "opencode";
  if (normalized === "openclaw") return "openclaw";
  if (normalized === "hermes") return "hermes";
  return null;
}

export async function fetchVisibleApps(): Promise<ManagedAppId[]> {
  try {
    const visibleApps = await invoke<string[]>("get_visible_apps");
    return normalizeVisibleApps(visibleApps);
  } catch {
    return [...DEFAULT_VISIBLE_APPS];
  }
}
