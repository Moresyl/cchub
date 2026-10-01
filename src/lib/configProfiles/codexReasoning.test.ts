import { describe, expect, it } from "vitest";
import {
  applyPresetToFields,
  buildStructuredConfig,
  createDefaultStructuredFields,
  parseStructuredConfig,
} from "./index";
import { parseCodexStructuredConfig } from "../codexConfig";
import { mergeSharedDraftFields } from "../../pages/profiles/draftMerge";

describe("profile reasoning serialization", () => {
  it.each(["", "none", "minimal", "max", "future-effort", 'custom"level'])(
    "round-trips explicit effort %j",
    (effort) => {
      const fields = { ...createDefaultStructuredFields("codex"), codexReasoningEffort: effort };
      const saved = buildStructuredConfig("codex", fields);
      const config = JSON.parse(saved).config;
      expect(parseCodexStructuredConfig(config).reasoningEffort).toBe(effort);
      expect(parseStructuredConfig("codex", saved).codexReasoningEffort).toBe(effort);
      if (!effort) expect(config).not.toContain("model_reasoning_effort");
      else expect(config).toContain("model_reasoning_effort");
    },
  );

  it("does not manufacture an effort when importing an omitted or quoted TOML setting", () => {
    for (const config of [
      "model = 'a'",
      "model_reasoning_effort = ''",
      "[unrelated]\nmodel_reasoning_effort = 'high'",
    ]) {
      expect(parseStructuredConfig("codex", JSON.stringify({ config })).codexReasoningEffort).toBe("");
    }
    expect(
      parseStructuredConfig("codex", JSON.stringify({ config: "'model_reasoning_effort' = 'max'" }))
        .codexReasoningEffort,
    ).toBe("max");
  });

  it("retains explicit default through custom presets and shared-tool merges", () => {
    const fields = { ...createDefaultStructuredFields("codex"), codexReasoningEffort: "" };
    expect(applyPresetToFields("codex", "codex-custom", fields).codexReasoningEffort).toBe("");
    expect(applyPresetToFields("codex", "unknown", fields).codexReasoningEffort).toBe("");
    const parsed = parseStructuredConfig("codex", buildStructuredConfig("codex", fields));
    expect(
      mergeSharedDraftFields({ ...fields, codexReasoningEffort: "high" }, "codex", parsed, true).codexReasoningEffort,
    ).toBe("");
  });
});
