import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface CreateMcpClientInput {
  name: string;
  configPath: string | null;
}

export interface McpClientMutationResult {
  id: string;
  name: string;
  config_path: string;
  server_access: Record<string, boolean>;
  created_at: string | null;
}

export interface DeleteMcpClientInput {
  id: string;
}

export interface UpdateMcpClientAccessInput {
  id: string;
  serverAccess: Record<string, boolean>;
}

function invalidateMcpClients(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.mcpClientsPage });
}

export function useCreateMcpClientMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreateMcpClientInput) => invoke<McpClientMutationResult>("create_mcp_client", { ...input }),
    onSuccess: () => invalidateMcpClients(queryClient),
  });
}

export function useDeleteMcpClientMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteMcpClientInput) => invoke("delete_mcp_client", { ...input }),
    onSuccess: () => invalidateMcpClients(queryClient),
  });
}

export function useUpdateMcpClientAccessMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: UpdateMcpClientAccessInput) => invoke("update_mcp_client_access", { ...input }),
    onSuccess: () => invalidateMcpClients(queryClient),
  });
}
