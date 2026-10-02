import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../../hooks/queries";
import { useConfirmedToolSettings } from "./useConfirmedToolSettings";

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

const configuration = {
  queryKey: codexSettingsKey,
  read: async () => decode(await invoke<unknown>("get_codex_settings")),
  write: async (key: string, value: string, expectedRevision: string) =>
    decode(await invoke<unknown>("set_codex_setting", { key, value, expectedRevision })),
};
export function useCodexSettings() {
  return useConfirmedToolSettings(configuration);
}
