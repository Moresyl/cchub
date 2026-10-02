import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../../hooks/queries";
import { useConfirmedToolSettings } from "./useConfirmedToolSettings";

export interface ClaudeSettings {
  permission_mode: string;
  allow_count: number;
  ask_count: number;
  deny_count: number;
  auto_update: string;
  model: string;
  tool_search: string;
  legacy_tool_search: string;
  config_revision: string;
}
export const claudeSettingsKey = [...queryKeys.toolSettings, "claude"] as const;
function decode(value: unknown): ClaudeSettings {
  const settings = value as ClaudeSettings | null;
  if (
    !settings ||
    typeof settings !== "object" ||
    ![
      settings.permission_mode,
      settings.auto_update,
      settings.model,
      settings.tool_search,
      settings.legacy_tool_search,
    ].every((value) => typeof value === "string") ||
    ![settings.allow_count, settings.ask_count, settings.deny_count].every(
      (value) => Number.isSafeInteger(value) && value >= 0,
    ) ||
    typeof settings.config_revision !== "string" ||
    !/^[a-f0-9]{64}$/.test(settings.config_revision)
  )
    throw new Error("Invalid settings response");
  return settings;
}
const configuration = {
  queryKey: claudeSettingsKey,
  read: async () => decode(await invoke<unknown>("get_claude_settings")),
  write: async (key: string, value: string, expectedRevision: string) =>
    decode(await invoke<unknown>("set_claude_setting", { key, value, expectedRevision })),
};
export function useClaudeSettings() {
  return useConfirmedToolSettings(configuration);
}
