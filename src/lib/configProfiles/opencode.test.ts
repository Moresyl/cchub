import { describe, expect, it } from "vitest";
import { buildStructuredConfig, parseStructuredConfig } from "./index";

describe("OpenCode structured editing", () => {
  it("keeps built-in identity and key-only SDK settings", () => {
    const source = {
      options: { apiKey: "before", timeout: 5000 },
      metadata: { nativeProviderId: "anthropic", nativeModelId: "claude-model" },
    };
    const fields = parseStructuredConfig("opencode", JSON.stringify(source));
    const output = JSON.parse(buildStructuredConfig("opencode", { ...fields, apiKey: "after" }));
    expect(output.metadata.nativeProviderId).toBe("anthropic");
    expect(output.metadata.nativeModelId).toBe("claude-model");
    expect(output.options).toEqual({ apiKey: "after", timeout: 5000 });
    expect(output.npm).toBeUndefined();
    expect(output.name).toBeUndefined();
    expect(source.options.apiKey).toBe("before");
  });

  it("edits the selected native model and retains provider and other model extensions", () => {
    const source = {
      npm: "@ai-sdk/anthropic",
      options: { baseURL: "https://old.test", apiKey: "key", timeout: 123 },
      whitelist: ["selected"],
      models: { alpha: { name: "Alpha" }, selected: { name: "Chosen", contextLimit: 128000, cost: { input: 2 } } },
      metadata: { nativeProviderId: "local", nativeModelId: "selected", owner: "retained" },
    };
    const fields = parseStructuredConfig("opencode", JSON.stringify(source));
    expect(fields.model).toBe("selected");
    expect(fields.openCodeContextLimit).toBe("128000");
    const output = JSON.parse(buildStructuredConfig("opencode", { ...fields, baseUrl: "https://new.test" }));
    expect(output.whitelist).toEqual(["selected"]);
    expect(output.models.alpha).toEqual(source.models.alpha);
    expect(output.models.selected.cost).toEqual({ input: 2 });
    expect(output.options).toEqual({ baseURL: "https://new.test", apiKey: "key", timeout: 123 });
    expect(output.metadata.owner).toBe("retained");
  });
});
