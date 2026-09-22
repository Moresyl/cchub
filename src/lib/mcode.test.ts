import { describe, expect, it } from "vitest";
import { buildMcodeProvider, createMcodeDraft } from "./mcode";

describe("MiniMax Code provider editing", () => {
  it("preserves custom options and model metadata while editing", () => {
    const original = {
      kind: "custom",
      options: { baseURL: "https://old.example", apiKey: "old", headers: { source: "test" } },
      models: { model: { name: "Display name", limit: { context: 1000 } } },
      extra: "kept",
    };
    const draft = createMcodeDraft("provider", original);
    draft.baseUrl = "https://new.example";
    draft.models += "\nmodel-2, model";
    const result = buildMcodeProvider(draft, original);
    expect(result.options?.headers).toEqual({ source: "test" });
    expect(result.models?.model).toEqual(original.models.model);
    expect(result.models?.["model-2"]).toEqual({ name: "model-2" });
    expect(result.extra).toBe("kept");
  });

  it("creates a provider with no duplicate or blank model IDs", () => {
    const draft = createMcodeDraft();
    draft.models = "a, a\n\nb";
    expect(Object.keys(buildMcodeProvider(draft).models ?? {})).toEqual(["a", "b"]);
  });
});
