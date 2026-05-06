import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type {
  LocalProviderProxySettings,
  LocalProviderProxyStatus,
  LogPreferences,
  TerminalPreferences,
  WindowPreferences,
} from "../../lib/appPreferences";
import { queryKeys } from "../queries";

export interface SetLocalProviderProxySettingsInput {
  settings: LocalProviderProxySettings;
}

export interface SetWindowPreferencesInput {
  preferences: WindowPreferences;
}

export interface SetVisibleAppsInput {
  visibleApps: string[];
}

export interface SetPreferredTerminalInput {
  terminalId: string;
}

export interface SetLogPreferencesInput {
  preferences: LogPreferences;
}

export interface SetSkillSyncMethodInput {
  method: "symlink" | "copy";
}

export interface SaveCustomPathInput {
  toolId: string;
  configDir: string | null;
  mcpConfigPath: string | null;
  skillsDir: string | null;
}

export interface SetProxyInput {
  proxyUrl: string;
}

function invalidateSettings(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.toolsPage });
  void queryClient.invalidateQueries({ queryKey: queryKeys.detectTools });
  void queryClient.invalidateQueries({ queryKey: queryKeys.visibleApps });
  void queryClient.invalidateQueries({ queryKey: queryKeys.skillSyncMethod });
  void queryClient.invalidateQueries({ queryKey: queryKeys.skillsPage });
}

export function useSetLocalProviderProxySettingsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetLocalProviderProxySettingsInput) =>
      invoke<LocalProviderProxyStatus>("set_local_provider_proxy_settings", { ...input }),
    onSuccess: () => invalidateSettings(queryClient),
  });
}

export function useSetWindowPreferencesMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetWindowPreferencesInput) => invoke<WindowPreferences>("set_window_preferences", { ...input }),
    onSuccess: () => invalidateSettings(queryClient),
  });
}

export function useSetVisibleAppsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetVisibleAppsInput) => invoke<string[]>("set_visible_apps", { ...input }),
    onSuccess: () => invalidateSettings(queryClient),
  });
}

export function useSetPreferredTerminalMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: SetPreferredTerminalInput) => {
      await invoke("set_preferred_terminal", { ...input });
      return invoke<TerminalPreferences>("get_terminal_preferences");
    },
    onSuccess: () => invalidateSettings(queryClient),
  });
}

export function useSetLogPreferencesMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetLogPreferencesInput) => invoke<LogPreferences>("set_log_preferences", { ...input }),
    onSuccess: () => invalidateSettings(queryClient),
  });
}

export function useSetSkillSyncMethodMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetSkillSyncMethodInput) => invoke("set_skill_sync_method", { ...input }),
    onSuccess: () => invalidateSettings(queryClient),
  });
}

export function useSaveCustomPathMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SaveCustomPathInput) => invoke("save_custom_path", { ...input }),
    onSuccess: () => invalidateSettings(queryClient),
  });
}

export function useSetProxyMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetProxyInput) => invoke("set_proxy", { ...input }),
    onSuccess: () => invalidateSettings(queryClient),
  });
}
