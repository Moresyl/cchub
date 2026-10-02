import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface SaveProxyAdvancedConfigInput {
  config: unknown;
  rectifierConfig: unknown;
  expectedRevision: string;
}

function invalidateProxyAdvanced(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.proxyAdvanced });
}

export function useSaveProxyAdvancedConfigMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: SaveProxyAdvancedConfigInput) => {
      return invoke<string>("set_proxy_advanced_config", {
        config: input.config,
        rectifierConfig: input.rectifierConfig,
        expectedRevision: input.expectedRevision,
      });
    },
    onSuccess: () => invalidateProxyAdvanced(queryClient),
  });
}
