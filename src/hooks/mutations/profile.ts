import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { fetchProfilesPageData, queryKeys } from "../queries";

export interface ApplyConfigProfileMutationResult {
  toolId: string;
  profileId: string;
  activeProfileIds: string[];
  appliedAt: string;
}

export interface SaveConfigProfileInput {
  name: string;
  toolId: string;
  configSnapshot: string;
}

export interface UpdateConfigProfileInput {
  id: string;
  name: string;
  configSnapshot: string;
}

export interface SaveSharedConfigProfilesInput {
  name: string;
  profiles: Array<{
    toolId: string;
    configSnapshot: string;
  }>;
  groupKey: string | null;
  replaceProfileId: string | null;
}

export interface DeleteConfigProfileInput {
  id: string;
}

export interface DeleteConfigProfileGroupInput {
  sourceKey: string;
}

export interface ReorderConfigProfilesInput {
  toolId: string;
  orderedIds: string[];
}

export interface SaveProviderConfigFragmentInput {
  name: string;
  targetTools: string[];
  fields: unknown;
}

export interface DeleteProviderConfigFragmentInput {
  id: string;
}

function invalidateProfiles(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.profilesPage });
  void queryClient.invalidateQueries({ queryKey: queryKeys.profiles });
  void queryClient.invalidateQueries({ queryKey: queryKeys.providerFragments });
  void queryClient.invalidateQueries({ queryKey: queryKeys.activeProfileIds });
  void queryClient.invalidateQueries({ queryKey: queryKeys.marketplaceLocal });
}

async function fetchUpdatedProfilesPageData() {
  return fetchProfilesPageData();
}

export function useApplyConfigProfileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => invoke<ApplyConfigProfileMutationResult>("apply_config_profile", { id }),
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useSaveConfigProfileMutation<TResult = unknown>() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SaveConfigProfileInput) => invoke<TResult>("save_config_profile", { ...input }),
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useSaveConfigProfileAndRefreshMutation<TResult = unknown>() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: SaveConfigProfileInput) => {
      const result = await invoke<TResult>("save_config_profile", { ...input });
      return { result, data: await fetchUpdatedProfilesPageData() };
    },
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useUpdateConfigProfileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: UpdateConfigProfileInput) => invoke("update_config_profile", { ...input }),
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useUpdateConfigProfileAndRefreshMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: UpdateConfigProfileInput) => {
      await invoke("update_config_profile", { ...input });
      return fetchUpdatedProfilesPageData();
    },
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useSaveSharedConfigProfilesMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SaveSharedConfigProfilesInput) => invoke<string>("save_shared_config_profiles", { ...input }),
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useSaveSharedConfigProfilesAndRefreshMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: SaveSharedConfigProfilesInput) => {
      const groupKey = await invoke<string>("save_shared_config_profiles", { ...input });
      return { groupKey, data: await fetchUpdatedProfilesPageData() };
    },
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useDeleteConfigProfileMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteConfigProfileInput) => invoke("delete_config_profile", { ...input }),
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useDeleteConfigProfileAndRefreshMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: DeleteConfigProfileInput) => {
      await invoke("delete_config_profile", { ...input });
      return fetchUpdatedProfilesPageData();
    },
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useDeleteConfigProfileGroupMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteConfigProfileGroupInput) => invoke<number>("delete_config_profile_group", { ...input }),
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useDeleteConfigProfileGroupAndRefreshMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: DeleteConfigProfileGroupInput) => {
      const removedCount = await invoke<number>("delete_config_profile_group", { ...input });
      return { removedCount, data: await fetchUpdatedProfilesPageData() };
    },
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useReorderConfigProfilesMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (input: ReorderConfigProfilesInput) => {
      await invoke("reorder_config_profiles", { ...input });
      await invoke("refresh_tray_provider_menu").catch((error) => {
        console.warn("Failed to refresh tray provider menu after profile reorder", error);
      });
    },
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useSaveProviderConfigFragmentMutation<TFragment>() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SaveProviderConfigFragmentInput) =>
      invoke<TFragment>("save_provider_config_fragment", { ...input }),
    onSuccess: () => invalidateProfiles(queryClient),
  });
}

export function useDeleteProviderConfigFragmentMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: DeleteProviderConfigFragmentInput) => invoke("delete_provider_config_fragment", { ...input }),
    onSuccess: () => invalidateProfiles(queryClient),
  });
}
