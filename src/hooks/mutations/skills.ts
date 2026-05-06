import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { fetchSkillsPageData, queryKeys } from "../queries";

export interface ImportSkillFileInput {
  targetSkillsDir: string;
  method: string;
}

export interface WriteSkillContentInput {
  filePath: string;
  content: string;
}

export interface UninstallSkillFileInput {
  path: string;
}

export interface ToggleSkillFileInput {
  filePath: string;
  enabled: boolean;
}

export interface DeletePluginInput {
  pluginId: string;
}

export interface BatchUpdateSkillsInput {
  ids: string[];
}

export interface RemoveSyncedSkillInput {
  skillName: string;
  targetSkillsDir: string;
}

export interface CopySkillBetweenToolsInput {
  path: string;
  targetSkillsDir: string;
  method: string;
}

export interface DeleteSkillBackupInput {
  id: string;
}

export interface RestoreSkillBackupInput {
  id: string;
}

function invalidateSkills(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.skillsPage });
  void queryClient.invalidateQueries({ queryKey: queryKeys.skills });
  void queryClient.invalidateQueries({ queryKey: queryKeys.skillBackups });
  void queryClient.invalidateQueries({ queryKey: queryKeys.plugins });
  void queryClient.invalidateQueries({ queryKey: queryKeys.marketplaceLocal });
}

async function fetchUpdatedSkillsPageData() {
  return fetchSkillsPageData();
}

export function useImportSkillFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: ImportSkillFileInput) => {
      await invoke<string>("import_skill_file", { ...input });
      return fetchUpdatedSkillsPageData();
    },
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useWriteSkillContentMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: WriteSkillContentInput) => invoke("write_skill_content", { ...input }),
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useUninstallSkillFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: UninstallSkillFileInput) => {
      await invoke("uninstall_skill_file", { ...input });
      return fetchUpdatedSkillsPageData();
    },
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useToggleSkillFileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: ToggleSkillFileInput) => {
      await invoke<string>("toggle_skill_file", { ...input });
      return fetchUpdatedSkillsPageData();
    },
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useDeletePluginMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: DeletePluginInput) => {
      await invoke("delete_plugin_dir", { pluginName: input.pluginId });
      await invoke("uninstall_plugin", { pluginId: input.pluginId });
      return fetchUpdatedSkillsPageData();
    },
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useBatchUpdateSkillsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: BatchUpdateSkillsInput) => {
      const updated = await invoke<number>("batch_update_skills", { ...input });
      return { updated, data: await fetchUpdatedSkillsPageData() };
    },
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useRemoveSyncedSkillMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: RemoveSyncedSkillInput) => invoke("remove_synced_skill", { ...input }),
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useCopySkillBetweenToolsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CopySkillBetweenToolsInput) => invoke("copy_skill_between_tools", { ...input }),
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useDeleteSkillBackupMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: DeleteSkillBackupInput) => {
      await invoke("delete_skill_backup", { ...input });
      return fetchUpdatedSkillsPageData();
    },
    onSuccess: () => invalidateSkills(queryClient),
  });
}

export function useRestoreSkillBackupMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: RestoreSkillBackupInput) => {
      const restoredTo = await invoke<string>("restore_skill_backup", { ...input });
      return { restoredTo, data: await fetchUpdatedSkillsPageData() };
    },
    onSuccess: () => invalidateSkills(queryClient),
  });
}
