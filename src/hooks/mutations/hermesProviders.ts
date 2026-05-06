import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface HermesProviderInput {
  name: string;
  baseUrl: string;
  apiMode: string;
  model: string;
  apiKeyEnv: string;
}

export interface SaveHermesProviderInput {
  provider: HermesProviderInput;
}

export interface DeleteHermesProviderInput {
  name: string;
}

export interface SetHermesActiveProviderInput {
  name: string;
}

function invalidateHermesProviders(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.hermesProviders });
}

export function useSaveHermesProviderMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SaveHermesProviderInput) => invoke("save_hermes_provider", { ...input }),
    onSuccess: () => invalidateHermesProviders(queryClient),
  });
}

export function useDeleteHermesProviderMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteHermesProviderInput) => invoke("delete_hermes_provider", { ...input }),
    onSuccess: () => invalidateHermesProviders(queryClient),
  });
}

export function useSetHermesActiveProviderMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetHermesActiveProviderInput) => invoke("set_hermes_active_provider", { ...input }),
    onSuccess: () => invalidateHermesProviders(queryClient),
  });
}
