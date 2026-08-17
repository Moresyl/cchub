import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface UninstallMcpServerInput {
  name: string;
}

export interface UpdateMcpServerConfigInput {
  name: string;
  command: string;
  args: unknown[];
  env: Record<string, string>;
}

export interface InstallMcpServerInput {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
}

export interface ToggleMcpAppInput {
  serverId: string;
  app: string;
  enabled: boolean;
}

export interface BulkToggleMcpAppInput {
  serverIds: string[];
  app: string;
  enabled: boolean;
}

export interface BulkToggleMcpAppResult {
  succeeded: string[];
  failed: Array<{ serverId: string; error: string }>;
}

function invalidateMcpServers(queryClient: ReturnType<typeof useQueryClient>) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.mcpServersPage });
}

export function useToggleMcpAppMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: ToggleMcpAppInput) => invoke("toggle_mcp_app", { ...input }),
    onSettled: () => invalidateMcpServers(queryClient),
  });
}

/** Apply app sync changes serially because each tool writes a whole config file. */
export function useBulkToggleMcpAppMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ serverIds, app, enabled }: BulkToggleMcpAppInput): Promise<BulkToggleMcpAppResult> => {
      const succeeded: string[] = [];
      const failed: Array<{ serverId: string; error: string }> = [];
      for (const serverId of serverIds) {
        try {
          await invoke("toggle_mcp_app", { serverId, app, enabled });
          succeeded.push(serverId);
        } catch (error) {
          failed.push({ serverId, error: String(error) });
        }
      }
      return { succeeded, failed };
    },
    onSettled: () => invalidateMcpServers(queryClient),
  });
}

export function useUninstallMcpServerMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: UninstallMcpServerInput) => invoke("uninstall_mcp_server", { ...input }),
    onSuccess: () => invalidateMcpServers(queryClient),
  });
}

export function useUpdateMcpServerConfigMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: UpdateMcpServerConfigInput) => invoke("update_mcp_server_config", { ...input }),
    onSuccess: () => invalidateMcpServers(queryClient),
  });
}

export function useInstallMcpServerMutation<TCreatedServer>() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: InstallMcpServerInput) => invoke<TCreatedServer>("install_mcp_server", { ...input }),
    onSuccess: () => invalidateMcpServers(queryClient),
  });
}
