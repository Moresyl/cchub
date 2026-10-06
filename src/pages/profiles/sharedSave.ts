import { buildStructuredConfig, isNativeOpenCodeConfig, type StructuredDraftFields } from "../../lib/configProfiles";
import type { ConfigProfile } from "./helpers";

export function buildSharedSavePayload(
  targetTools: string[],
  fields: StructuredDraftFields,
  draftTool: string,
  draftContent: string,
  editing: ConfigProfile | null,
  profiles: ConfigProfile[],
  native: boolean,
) {
  const group =
    editing?.source_type === "shared" && editing.source_key
      ? profiles.filter((profile) => profile.source_type === "shared" && profile.source_key === editing.source_key)
      : [];
  return targetTools.map((toolId) => {
    const original = group.find((profile) => profile.tool_id === toolId);
    const preserve =
      original && (native || (toolId === "opencode" && isNativeOpenCodeConfig(original.config_snapshot)));
    return {
      toolId,
      configSnapshot:
        native && toolId === draftTool
          ? draftContent
          : preserve
            ? original.config_snapshot
            : buildStructuredConfig(toolId, fields),
    };
  });
}
