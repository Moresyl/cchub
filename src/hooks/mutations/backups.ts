import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type { BackupPreferences } from "../../types/settings";
import { queryKeys } from "../queries";

export interface SetBackupPreferencesInput {
  preferences: BackupPreferences;
}

export interface DeleteManagedBackupInput {
  path: string;
}

function invalidateBackups(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.managedBackups });
}

export function useSaveBackupToFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => invoke<string>("save_backup_to_file"),
    onSuccess: () => invalidateBackups(queryClient),
  });
}

export function useSetBackupPreferencesMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetBackupPreferencesInput) => invoke<BackupPreferences>("set_backup_preferences", { ...input }),
    onSuccess: () => invalidateBackups(queryClient),
  });
}

export function useDeleteManagedBackupMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteManagedBackupInput) => invoke("delete_managed_backup", { ...input }),
    onSuccess: () => invalidateBackups(queryClient),
  });
}
