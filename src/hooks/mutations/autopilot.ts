import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface DeleteAutopilotLogInput {
  runId: string;
}

export interface ClearAutopilotLogsResult {
  deletedCount: number;
}

function invalidateAutopilot(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.autopilot });
}

export function useDeleteAutopilotLogMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteAutopilotLogInput) => invoke("delete_autopilot_log", { ...input }),
    onSuccess: () => invalidateAutopilot(queryClient),
  });
}

export function useClearAutopilotLogsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => invoke<ClearAutopilotLogsResult>("clear_autopilot_logs"),
    onSuccess: () => invalidateAutopilot(queryClient),
  });
}
