import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../../hooks/queries";

export interface CodexSettings {
  approval_mode: string;
  approval_policy: string;
  sandbox_mode: string;
  permission_profile: string;
  reasoning_effort: string;
  disable_response_storage: boolean;
  context_window_1m: boolean;
  context_window: number | null;
  legacy_personality: boolean;
  profile_selected: boolean;
  config_revision: string;
}

export const codexSettingsKey = [...queryKeys.toolSettings, "codex"] as const;

function decode(value: unknown): CodexSettings {
  const settings = value as CodexSettings | null;
  if (
    !settings ||
    typeof settings !== "object" ||
    !["default", "custom", "read-only", "workspace-write", "danger-full-access"].includes(settings.approval_mode) ||
    ![settings.approval_policy, settings.sandbox_mode, settings.permission_profile, settings.reasoning_effort].every(
      (value) => typeof value === "string",
    ) ||
    ![
      settings.disable_response_storage,
      settings.context_window_1m,
      settings.legacy_personality,
      settings.profile_selected,
    ].every((value) => typeof value === "boolean") ||
    (settings.context_window !== null &&
      (!Number.isSafeInteger(settings.context_window) || settings.context_window <= 0)) ||
    typeof settings.config_revision !== "string" ||
    !/^[a-f0-9]{64}$/.test(settings.config_revision)
  )
    throw new Error("Invalid settings response");
  return settings;
}

export function useCodexSettings() {
  const client = useQueryClient();
  const generation = useRef(0);
  const mounted = useRef(true);
  const pending = useRef(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current += 1;
    };
  }, []);
  const query = useQuery({
    queryKey: codexSettingsKey,
    queryFn: async () => {
      generation.current += 1;
      return decode(await invoke<unknown>("get_codex_settings"));
    },
    staleTime: 0,
    retry: false,
    throwOnError: false,
    refetchOnMount: "always",
    networkMode: "always",
  });
  const { refetch } = query;
  const reload = useCallback(async () => {
    if (pending.current) return;
    setSaveError(false);
    await refetch();
  }, [refetch]);
  const save = useCallback(
    async (key: string, value: string) => {
      const current = client.getQueryData<CodexSettings>(codexSettingsKey);
      if (pending.current || query.isFetching || query.isError || !current?.config_revision || saveError) return false;
      pending.current = true;
      setSaving(true);
      setSaveError(false);
      await client.cancelQueries({ queryKey: codexSettingsKey });
      const attempt = ++generation.current;
      try {
        const result = decode(
          await invoke<unknown>("set_codex_setting", {
            key,
            value,
            expectedRevision: current.config_revision,
          }),
        );
        if (mounted.current && generation.current === attempt) {
          client.setQueryData(codexSettingsKey, result);
        } else {
          void client.invalidateQueries({ queryKey: codexSettingsKey });
        }
        return true;
      } catch {
        // Native parser messages can contain credential lines; never show them.
        if (mounted.current && generation.current === attempt) setSaveError(true);
        return false;
      } finally {
        pending.current = false;
        if (mounted.current) setSaving(false);
      }
    },
    [client, query.isError, query.isFetching, saveError],
  );
  return {
    settings: query.data,
    loading: query.isPending,
    refreshing: query.isFetching,
    loadError: query.isError,
    saveError,
    saving,
    save,
    reload,
  };
}
