import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  queryKeys,
  type Hello2ccConfigQueryResult,
  type Hello2ccStatusQueryResult,
  type HudStatusQueryResult,
} from "../queries";

export interface SetClaudeSettingInput {
  command: string;
  args: Record<string, unknown>;
}

export interface SetCodexSettingInput {
  key: string;
  value: string;
}

export interface SetClaudeStatuslineInput {
  enabled: boolean;
}

export interface SetHello2ccEnabledInput {
  enabled: boolean;
}

export type SetClaudeHudConfigInput = unknown;
export type SetHello2ccConfigInput = Hello2ccConfigQueryResult;

export interface UpdateClaudeHudResult {
  version: string;
  skipped: boolean;
}

function invalidateToolSettings(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.toolsPage });
  void queryClient.invalidateQueries({ queryKey: queryKeys.toolSettings });
  void queryClient.invalidateQueries({ queryKey: queryKeys.hudStatus });
  void queryClient.invalidateQueries({ queryKey: queryKeys.hello2ccStatus });
}

export function useSetClaudeSettingMutation<TValue>() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetClaudeSettingInput) => invoke<TValue>(input.command, input.args),
    onSuccess: () => invalidateToolSettings(queryClient),
  });
}

export function useSetCodexSettingMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetCodexSettingInput) => invoke("set_codex_setting", { ...input }),
    onSuccess: () => invalidateToolSettings(queryClient),
  });
}

export function useSetClaudeStatuslineMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetClaudeStatuslineInput) => invoke("set_claude_statusline", { ...input }),
    onSuccess: () => invalidateToolSettings(queryClient),
  });
}

export function useSetHello2ccEnabledMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetHello2ccEnabledInput) => invoke("set_hello2cc_enabled", { ...input }),
    onSuccess: () => invalidateToolSettings(queryClient),
  });
}

export function useUpdateClaudeHudMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => invoke<UpdateClaudeHudResult>("update_claude_hud"),
    onSuccess: () => invalidateToolSettings(queryClient),
  });
}

export function useUpdateHello2ccMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => invoke<Hello2ccStatusQueryResult>("update_hello2cc"),
    onSuccess: () => invalidateToolSettings(queryClient),
  });
}

export function useSetClaudeHudConfigMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (config: SetClaudeHudConfigInput) => invoke<HudStatusQueryResult>("set_claude_hud_config", { config }),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.hudStatus, status);
      invalidateToolSettings(queryClient);
    },
  });
}

export function useSetHello2ccConfigMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (config: SetHello2ccConfigInput) =>
      invoke<Hello2ccStatusQueryResult>("set_hello2cc_config", { config }),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.hello2ccStatus, status);
      invalidateToolSettings(queryClient);
    },
  });
}
