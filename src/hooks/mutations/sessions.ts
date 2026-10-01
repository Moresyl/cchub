import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

export interface DeleteSessionMutationInput {
  toolId: string;
  sessionId: string;
  sourcePath: string;
  sourceBackend: string;
}

export interface DeleteSessionsMutationInput {
  sessions: Array<{
    tool_id: string;
    session_id: string;
    source_path: string;
    source_backend: string;
  }>;
}

export interface SessionDeleteResultTarget {
  toolId: string;
  sessionId: string;
  sourcePath: string;
  sourceBackend: string;
}

export interface SessionBatchDeleteResult {
  deleted: SessionDeleteResultTarget[];
  failed: Array<{ target: SessionDeleteResultTarget; error: string }>;
}

export function useDeleteSessionMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteSessionMutationInput) => invoke("delete_session", { ...input }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["sessions"] });
    },
  });
}

export function useDeleteSessionsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteSessionsMutationInput) =>
      invoke<SessionBatchDeleteResult>("delete_sessions_checked", { sessions: input.sessions }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["sessions"] });
    },
  });
}
