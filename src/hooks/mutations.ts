import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type { Hello2ccConfigQueryResult, Hello2ccStatusQueryResult, HudStatusQueryResult } from "./queries";
import { queryKeys } from "./queries";

export interface ApplyConfigProfileMutationResult {
  toolId: string;
  profileId: string;
  activeProfileIds: string[];
  appliedAt: string;
}

export function useSetClaudeHudConfigMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (config: unknown) =>
      invoke<HudStatusQueryResult>("set_claude_hud_config", { config }),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.hudStatus, status);
    },
  });
}

export function useSetHello2ccConfigMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (config: Hello2ccConfigQueryResult) =>
      invoke<Hello2ccStatusQueryResult>("set_hello2cc_config", { config }),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.hello2ccStatus, status);
    },
  });
}

export function useApplyConfigProfileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) =>
      invoke<ApplyConfigProfileMutationResult>("apply_config_profile", { id }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.profiles });
    },
  });
}
