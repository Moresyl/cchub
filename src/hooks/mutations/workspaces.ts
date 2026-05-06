import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface CreateWorkspaceInput {
  name: string;
  description: string | null;
  basePath: string | null;
}

export interface WorkspaceMutationResult {
  id: string;
  name: string;
  description: string | null;
  base_path: string | null;
  is_active: boolean;
  created_at: string | null;
}

export interface SwitchWorkspaceInput {
  id: string;
}

export interface DeleteWorkspaceInput {
  id: string;
}

export interface UpdateWorkspaceInput {
  id: string;
  name: string;
  description: string | null;
  basePath: string | null;
}

function invalidateWorkspaces(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.workspaces });
}

export function useCreateWorkspaceMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreateWorkspaceInput) => invoke<WorkspaceMutationResult>("create_workspace", { ...input }),
    onSuccess: () => invalidateWorkspaces(queryClient),
  });
}

export function useSwitchWorkspaceMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SwitchWorkspaceInput) => invoke("switch_workspace", { ...input }),
    onSuccess: () => invalidateWorkspaces(queryClient),
  });
}

export function useDeleteWorkspaceMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteWorkspaceInput) => invoke("delete_workspace", { ...input }),
    onSuccess: () => invalidateWorkspaces(queryClient),
  });
}

export function useUpdateWorkspaceMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: UpdateWorkspaceInput) => invoke("update_workspace", { ...input }),
    onSuccess: () => invalidateWorkspaces(queryClient),
  });
}
