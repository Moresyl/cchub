import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { fetchClaudeMdPageData, queryKeys } from "../queries";

export interface WriteClaudeMdContentInput {
  path: string;
  content: string;
}

export interface SavePromptPresetInput {
  id: string | null;
  name: string;
  content: string;
}

export interface CreateInstructionDocFileInput {
  dirPath: string;
  fileName: string;
  content: string;
}

export interface DeleteClaudeMdFileInput {
  path: string;
}

export interface ToggleClaudeMdFileInput {
  path: string;
}

export interface ActivatePromptPresetInput {
  id: string;
}

export interface DeletePromptPresetInput {
  id: string;
}

function invalidateClaudeMd(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.claudeMdPage });
  void queryClient.invalidateQueries({ queryKey: queryKeys.claudeMd });
  void queryClient.invalidateQueries({ queryKey: queryKeys.promptPresets });
}

async function fetchUpdatedClaudeMdPageData() {
  return fetchClaudeMdPageData();
}

export function useWriteClaudeMdContentMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: WriteClaudeMdContentInput) => invoke("write_claude_md_content", { ...input }),
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}

export function useSavePromptPresetMutation<TResult = unknown>() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SavePromptPresetInput) => invoke<TResult>("save_prompt_preset", { ...input }),
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}

export function useSavePromptPresetAndRefreshMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: SavePromptPresetInput) => {
      await invoke("save_prompt_preset", { ...input });
      return fetchUpdatedClaudeMdPageData();
    },
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}

export function useCreateInstructionDocFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: CreateInstructionDocFileInput) => {
      const path = await invoke<string>("create_instruction_doc_file", { ...input });
      return { path, data: await fetchUpdatedClaudeMdPageData() };
    },
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}

export function useDeleteClaudeMdFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: DeleteClaudeMdFileInput) => {
      await invoke("delete_claude_md_file", { ...input });
      return fetchUpdatedClaudeMdPageData();
    },
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}

export function useEnableClaudeMdFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: ToggleClaudeMdFileInput) => {
      await invoke<string>("enable_claude_md_file", { ...input });
      return fetchUpdatedClaudeMdPageData();
    },
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}

export function useDisableClaudeMdFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: ToggleClaudeMdFileInput) => {
      await invoke<string>("disable_claude_md_file", { ...input });
      return fetchUpdatedClaudeMdPageData();
    },
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}

export function useActivatePromptPresetMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: ActivatePromptPresetInput) => {
      await invoke("activate_prompt_preset", { ...input });
      return fetchUpdatedClaudeMdPageData();
    },
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}

export function useDeletePromptPresetMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: DeletePromptPresetInput) => {
      await invoke("delete_prompt_preset", { ...input });
      return fetchUpdatedClaudeMdPageData();
    },
    onSuccess: () => invalidateClaudeMd(queryClient),
  });
}
