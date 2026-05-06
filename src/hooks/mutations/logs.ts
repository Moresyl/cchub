import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type { ModelPricingRowQueryResult } from "../queries";
import { queryKeys } from "../queries";

export interface ModelPricingDraftInput {
  model_id: string;
  input_cost_per_million: string;
  output_cost_per_million: string;
  cache_read_cost_per_million: string;
  cache_write_cost_per_million: string;
}

export interface SaveModelPricingInput {
  entry: ModelPricingDraftInput;
}

export interface DeleteModelPricingInput {
  modelId: string;
}

function invalidateLogs(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: ["logs-page"] });
  void queryClient.invalidateQueries({ queryKey: queryKeys.logsPage("") });
}

export function useSaveModelPricingMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SaveModelPricingInput) =>
      invoke<ModelPricingRowQueryResult>("save_model_pricing", { ...input }),
    onSuccess: () => invalidateLogs(queryClient),
  });
}

export function useDeleteModelPricingMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteModelPricingInput) => invoke("delete_model_pricing", { ...input }),
    onSuccess: () => invalidateLogs(queryClient),
  });
}
