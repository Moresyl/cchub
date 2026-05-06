import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface SaveHookToSettingsInput {
  event: string;
  matcher: string | null;
  command: string;
  timeout: number | null;
  scope: "global" | "project";
  projectPath: string | null;
  editIndex: number | null;
}

export interface DeleteHookFromSettingsInput {
  event: string;
  index: number;
  scope: "global" | "project";
  projectPath: string | null;
}

export interface RemapImportedProjectRootInput {
  sourcePath: string;
  targetPath: string;
}

export interface UpdateHookInput {
  previous: DeleteHookFromSettingsInput | null;
  next: SaveHookToSettingsInput;
  remapProjectRoot?: RemapImportedProjectRootInput | null;
}

export interface HookMutationResult {
  id: string;
  event: string;
  matcher: string | null;
  command: string;
  scope: string;
  project_path: string | null;
  source_event: string | null;
  source_index: number | null;
  enabled: boolean;
  timeout: number | null;
}

function invalidateHooks(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.hooks });
}

export function useSaveHookToSettingsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: SaveHookToSettingsInput) => {
      await invoke("save_hook_to_settings", { ...input });
      return invoke<HookMutationResult[]>("scan_hooks");
    },
    onSuccess: () => invalidateHooks(queryClient),
  });
}

export function useDeleteHookFromSettingsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: DeleteHookFromSettingsInput) => {
      await invoke("delete_hook_from_settings", { ...input });
      return invoke<HookMutationResult[]>("scan_hooks");
    },
    onSuccess: () => invalidateHooks(queryClient),
  });
}

export function useUpdateHookInSettingsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: UpdateHookInput) => {
      if (input.previous) {
        await invoke("delete_hook_from_settings", { ...input.previous });
      }
      await invoke("save_hook_to_settings", { ...input.next });
      if (input.remapProjectRoot) {
        await invoke("remap_imported_project_root", { ...input.remapProjectRoot });
      }
      return invoke<HookMutationResult[]>("scan_hooks");
    },
    onSuccess: () => invalidateHooks(queryClient),
  });
}
