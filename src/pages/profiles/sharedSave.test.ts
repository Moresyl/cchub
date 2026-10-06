import { describe, expect, it } from "vitest";
import { createDefaultStructuredFields } from "../../lib/configProfiles";
import { buildSharedSavePayload } from "./sharedSave";
import type { ConfigProfile } from "./helpers";

const shared = (tool_id: string, snapshot: unknown): ConfigProfile => ({
  id: tool_id,
  tool_id,
  name: "Shared",
  config_snapshot: JSON.stringify(snapshot),
  source_type: "shared",
  source_key: "group",
  sort_order: 0,
  created_at: null,
  updated_at: null,
});

describe("shared configuration saving", () => {
  it("preserves native members when editing another shared tool", () => {
    const current = shared("claude", { env: { ANTHROPIC_AUTH_TOKEN: "old" } });
    const native = shared("opencode", {
      settings: { apiKey: "independent" },
      models: { m: { variants: [{ id: "fast" }] } },
    });
    const payload = buildSharedSavePayload(
      ["claude", "opencode"],
      { ...createDefaultStructuredFields("claude"), apiKey: "new" },
      "claude",
      "{}",
      current,
      [current, native],
      false,
    );
    expect(payload[1].configSnapshot).toBe(native.config_snapshot);
    expect(JSON.parse(payload[0].configSnapshot).env.ANTHROPIC_AUTH_TOKEN).toBe("new");
  });

  it("saves the native draft without regenerating other group members or their credentials", () => {
    const current = shared("opencode", { metadata: { nativeFormat: "providers" }, settings: { apiKey: "old" } });
    const other = shared("codex", { auth: { OPENAI_API_KEY: "independent" }, config: "model='m'" });
    const draft = JSON.stringify({ ...JSON.parse(current.config_snapshot), settings: { apiKey: "new" } });
    const payload = buildSharedSavePayload(
      ["opencode", "codex"],
      createDefaultStructuredFields("opencode"),
      "opencode",
      draft,
      current,
      [current, other],
      true,
    );
    expect(payload[0].configSnapshot).toBe(draft);
    expect(payload[1].configSnapshot).toBe(other.config_snapshot);
  });
});
