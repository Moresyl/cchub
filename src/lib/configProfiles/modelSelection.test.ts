import { describe, expect, it } from "vitest";
import { buildStructuredConfig, createDefaultStructuredFields, parseStructuredConfig } from "./index";
import { discoveredModelFields, selectProfileModel } from "./modelSelection";
import { mergeDraftFields } from "../../pages/profiles/draftMerge";

describe("model capabilities and selection", () => {
  it("round-trips a provider-local catalog through every supported profile format", () => {
    for (const tool of ["claude", "codex", "gemini", "openclaw", "opencode", "hermes", "grokbuild"]) {
      const fields = createDefaultStructuredFields(tool);
      fields.modelCatalog = {
        toolId: tool,
        models: [
          {
            id: "custom",
            contextWindow: 200000,
            supportedReasoningLevels: ["high"],
            nativeEndpoints: ["/responses"],
            inputModalities: ["text", "image"],
          },
        ],
      };
      const restored = parseStructuredConfig(tool, buildStructuredConfig(tool, fields));
      expect(restored.modelCatalog).toEqual(fields.modelCatalog);
      const other = JSON.parse(buildStructuredConfig(tool === "claude" ? "codex" : "claude", fields));
      expect(other.metadata.modelCatalog).toBeUndefined();
    }
  });

  it("invalidates catalog snapshots when connection credentials or protocol change", () => {
    const fields = {
      ...createDefaultStructuredFields("claude"),
      modelCatalog: { toolId: "claude", models: [{ id: "a" }] },
    };
    for (const next of [
      { apiKey: "another" },
      { baseUrl: "https://other.test" },
      { oauthAccountId: "other" },
      { apiFormat: "openai_chat" as const },
      { requestHeaders: { "x-account": "other" } },
    ]) {
      expect(mergeDraftFields(fields, next).modelCatalog).toBeUndefined();
    }
    expect(mergeDraftFields(fields, { model: "b" }).modelCatalog).toEqual(fields.modelCatalog);
  });

  it("switches model-specific limits without leaking them to a new or existing model", () => {
    const original = parseStructuredConfig(
      "opencode",
      JSON.stringify({
        metadata: { nativeModelId: "a", nativeProviderId: "local" },
        models: {
          a: { name: "Alpha", contextLimit: 200000, outputLimit: 16000, cost: { input: 2 } },
          b: {
            name: "Beta",
            contextLimit: 10000,
            variants: { fast: { reasoningEffort: "low" }, slow: { reasoningEffort: "high" } },
          },
        },
      }),
    );
    const edited = { ...original, openCodeContextLimit: "300000" };
    const b = mergeDraftFields(edited, selectProfileModel("opencode", edited, "b"));
    expect(b.openCodeContextLimit).toBe("10000");
    expect(b.openCodeOutputLimit).toBe("");
    expect(b.modelName).toBe("Beta");
    const c = mergeDraftFields(b, selectProfileModel("opencode", b, "c"));
    expect(c.openCodeContextLimit).toBe("");
    expect(c.modelName).toBe("");
    const restored = mergeDraftFields(c, selectProfileModel("opencode", c, "a"));
    expect(restored.openCodeContextLimit).toBe("300000");
    expect(restored.openCodeOutputLimit).toBe("16000");
    const saved = JSON.parse(buildStructuredConfig("opencode", restored));
    expect(saved.models.a.cost).toEqual({ input: 2 });
    expect(saved.models.b.variants.slow).toEqual({ reasoningEffort: "high" });
    expect(saved.metadata.nativeProviderId).toBe("local");
  });

  it("keeps supported discovery fields explicit and never invents tool settings", () => {
    const model = {
      id: "a",
      contextWindow: 200000,
      maxOutputTokens: 16000,
      inputModalities: ["text", "image"],
      outputModalities: ["text"],
      nativeEndpoints: ["/responses"],
      supportedReasoningLevels: ["high"],
    };
    expect(discoveredModelFields("opencode", model)).toEqual({
      openCodeContextLimit: "200000",
      openCodeOutputLimit: "16000",
      openCodeInputModalities: "text,image",
      openCodeOutputModalities: "text",
    });
    expect(discoveredModelFields("openclaw", model)).toEqual({ openClawContextWindow: "200000" });
    expect(discoveredModelFields("claude", model)).toEqual({});
  });

  it("retains a cleared selection after saving without deleting configured models", () => {
    const fields = parseStructuredConfig(
      "opencode",
      JSON.stringify({ metadata: { nativeModelId: "a" }, models: { a: { contextLimit: 10000 } } }),
    );
    const cleared = mergeDraftFields(fields, selectProfileModel("opencode", fields, ""));
    const saved = buildStructuredConfig("opencode", cleared);
    expect(JSON.parse(saved).metadata.nativeModelId).toBe("");
    expect(JSON.parse(saved).models.a.contextLimit).toBe(10000);
    expect(parseStructuredConfig("opencode", saved).model).toBe("");
  });
});
