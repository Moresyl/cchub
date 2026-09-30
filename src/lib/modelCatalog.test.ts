import { describe, expect, it } from "vitest";
import { normalizeModelCatalog } from "./modelCatalog";

describe("persisted model catalog", () => {
  it("retains valid IDs without persisting unknown or unsafe response fields", () => {
    expect(
      normalizeModelCatalog({
        toolId: "opencode",
        models: [
          null,
          {
            id: " a ",
            apiKey: "never-save",
            contextWindow: Number.MAX_SAFE_INTEGER + 1,
            maxOutputTokens: 10,
            supportedReasoningLevels: ["high", "high", null],
          },
          { id: "a", displayName: "Alpha", contextWindow: 200000 },
          { id: " " },
        ],
      }),
    ).toEqual({
      toolId: "opencode",
      models: [
        {
          id: "a",
          displayName: "Alpha",
          contextWindow: 200000,
          maxOutputTokens: 10,
          supportedReasoningLevels: ["high"],
        },
      ],
    });
  });
  it("rejects an invalid container and oversized catalogs", () => {
    expect(normalizeModelCatalog(null)).toBeUndefined();
    expect(normalizeModelCatalog({ toolId: "claude", models: {} })).toBeUndefined();
    expect(normalizeModelCatalog({ toolId: "claude", models: Array(10001).fill({ id: "a" }) })).toBeUndefined();
  });
});
