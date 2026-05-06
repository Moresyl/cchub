import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface SetOpenClawEnvInput {
  env: Record<string, string>;
}

export interface SetOpenClawToolsInput {
  tools: unknown;
}

export interface SetOpenClawAgentsDefaultsInput {
  defaults: unknown;
}

function invalidateOpenClaw(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.openClaw });
}

export function useSetOpenClawEnvMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetOpenClawEnvInput) => invoke("set_openclaw_env", { ...input }),
    onSuccess: () => invalidateOpenClaw(queryClient),
  });
}

export function useSetOpenClawToolsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetOpenClawToolsInput) => invoke("set_openclaw_tools", { ...input }),
    onSuccess: () => invalidateOpenClaw(queryClient),
  });
}

export function useSetOpenClawAgentsDefaultsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetOpenClawAgentsDefaultsInput) => invoke("set_openclaw_agents_defaults", { ...input }),
    onSuccess: () => invalidateOpenClaw(queryClient),
  });
}
