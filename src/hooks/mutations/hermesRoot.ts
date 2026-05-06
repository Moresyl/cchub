import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface SetHermesRootOverrideInput {
  value: string | null;
}

function invalidateHermesRoot(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.hermesRootOverride });
  void queryClient.invalidateQueries({ queryKey: queryKeys.detectTools });
}

export function useSetHermesRootOverrideMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetHermesRootOverrideInput) => invoke("set_hermes_root_override", { ...input }),
    onSuccess: () => invalidateHermesRoot(queryClient),
  });
}
