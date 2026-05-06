import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface ToggleMcpServerInput {
  id: string;
  enabled: boolean;
}

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

function invalidateMcpServers(queryClient: ReturnType<typeof useQueryClient>) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.mcpServersPage });
}

export function useToggleMcpServerMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: ToggleMcpServerInput) => invoke("toggle_mcp_server", { ...input }),
    onSuccess: () => invalidateMcpServers(queryClient),
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
