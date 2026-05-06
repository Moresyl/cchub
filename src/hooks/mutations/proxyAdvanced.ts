import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface SaveProxyAdvancedConfigInput {
  config: unknown;
  rectifierConfig: unknown | null;
}

function invalidateProxyAdvanced(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.proxyAdvanced });
}

export function useSaveProxyAdvancedConfigMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: SaveProxyAdvancedConfigInput) => {
      await invoke("set_optimizer_config", { config: input.config });
      if (input.rectifierConfig) {
        await invoke("set_rectifier_config", { config: input.rectifierConfig });
      }
    },
    onSuccess: () => invalidateProxyAdvanced(queryClient),
  });
}
