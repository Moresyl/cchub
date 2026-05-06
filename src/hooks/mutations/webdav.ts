import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface SetWebDavSyncSettingsInput {
  settings: unknown;
  passwordTouched: boolean;
}

function invalidateWebDav(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.webdavSync });
}

export function useSetWebDavSyncSettingsMutation<TSettings>() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetWebDavSyncSettingsInput) => invoke<TSettings>("set_webdav_sync_settings", { ...input }),
    onSuccess: () => invalidateWebDav(queryClient),
  });
}
