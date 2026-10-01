import { describe, expect, it } from "vitest";
import { normalizeModelCatalog } from "./modelCatalog";

describe("persisted model catalog", () => {
  it("round-trips request billing separately from monetary prices and old catalogs", () => {
    const result = normalizeModelCatalog({
      toolId: "claude",
      models: [
        { id: "free", premiumRequestBilling: { kind: "free", multiplier: 0 } },
        { id: "paid", premiumRequestBilling: { kind: "premium", multiplier: 0.33 }, inputPrice: "0.001" },
        { id: "broken", premiumRequestBilling: { kind: "premium", multiplier: -1 } },
        { id: "old" },
      ],
    });
    expect(result?.models).toEqual([
      { id: "free", premiumRequestBilling: { kind: "free", multiplier: 0 } },
      { id: "paid", premiumRequestBilling: { kind: "premium", multiplier: 0.33 }, inputPrice: "0.001" },
      { id: "broken", premiumRequestBilling: { kind: "unknown", multiplier: null } },
      { id: "old" },
    ]);
    expect(normalizeModelCatalog(JSON.parse(JSON.stringify(result)))).toEqual(result);
  });
  it("keeps duplicate premium identity but treats rate disagreement as uncertain", () => {
    const result = normalizeModelCatalog({
      toolId: "claude",
      models: [
        { id: "paid", premiumRequestBilling: { kind: "premium", multiplier: 1 } },
        { id: "paid", premiumRequestBilling: { kind: "premium", multiplier: 2 } },
        { id: "paid", displayName: "Paid" },
      ],
    });
    expect(result?.models).toEqual([
      { id: "paid", displayName: "Paid", premiumRequestBilling: { kind: "premium", multiplier: null } },
    ]);
  });
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
