import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface WriteWorkflowContentInput {
  path: string;
  content: string;
}

export interface DeleteWorkflowInput {
  path: string;
}

export interface ToggleWorkflowInput {
  path: string;
  enabled: boolean;
}

export interface InstallWorkflowInput {
  toolId: string;
  templateId: string;
}

export interface ImportWorkflowFileInput {
  toolId: string;
}

function invalidateWorkflows(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.workflows });
}

export function useWriteWorkflowContentMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: WriteWorkflowContentInput) => invoke("write_workflow_content", { ...input }),
    onSuccess: () => invalidateWorkflows(queryClient),
  });
}

export function useDeleteWorkflowMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteWorkflowInput) => invoke("delete_workflow", { ...input }),
    onSuccess: () => invalidateWorkflows(queryClient),
  });
}

export function useToggleWorkflowMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: ToggleWorkflowInput) => invoke<string>("toggle_workflow", { ...input }),
    onSuccess: () => invalidateWorkflows(queryClient),
  });
}

export function useInstallWorkflowMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: InstallWorkflowInput) => invoke<string>("install_workflow", { ...input }),
    onSuccess: () => invalidateWorkflows(queryClient),
  });
}

export function useImportWorkflowFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: ImportWorkflowFileInput) => invoke<string>("import_workflow_file", { ...input }),
    onSuccess: () => invalidateWorkflows(queryClient),
  });
}
