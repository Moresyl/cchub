import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export type HermesMemoryKind = "memory" | "user";

export interface SaveHermesMemoryContentInput {
  kind: HermesMemoryKind;
  content: string;
}

export interface ToggleHermesMemoryEnabledInput {
  kind: HermesMemoryKind;
  enabled: boolean;
}

function invalidateHermesMemory(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.hermesMemory });
}

export function useSaveHermesMemoryContentMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SaveHermesMemoryContentInput) => invoke("save_hermes_memory_content", { ...input }),
    onSuccess: () => invalidateHermesMemory(queryClient),
  });
}

export function useToggleHermesMemoryEnabledMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: ToggleHermesMemoryEnabledInput) => invoke("toggle_hermes_memory_enabled", { ...input }),
    onSuccess: () => invalidateHermesMemory(queryClient),
  });
}
