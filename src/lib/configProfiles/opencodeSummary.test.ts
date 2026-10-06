import { describe, expect, it } from "vitest";
import { openCodeSummary } from "./opencodeSummary";

describe("configuration card summary", () => {
  it("shows the selected native model and effective variant endpoint rather than the first model", () => {
    expect(
      openCodeSummary({
        metadata: { nativeFormat: "providers", nativeModelId: "selected#fast", iconUrl: "icon" },
        settings: { baseURL: "https://provider.test" },
        models: {
          first: {},
          selected: {
            settings: { baseURL: "https://model.test" },
            variants: [
              { id: "slow", settings: { baseURL: "https://unrelated.test" } },
              { id: "fast", settings: { baseURL: "https://selected.test" } },
            ],
          },
        },
      }),
    ).toEqual({ model: "selected#fast", baseUrl: "https://selected.test", iconUrl: "icon" });
  });

  it("preserves legacy endpoints and explicit default-model selection", () => {
    expect(
      openCodeSummary({
        options: { baseURL: "https://legacy.test" },
        metadata: { nativeModelId: "" },
        models: { first: {} },
      }),
    ).toEqual({ baseUrl: "https://legacy.test", model: undefined, iconUrl: undefined });
    expect(openCodeSummary({ options: {}, models: { first: {} } }).model).toBe("first");
  });

  it("does not crash on incomplete or invalid draft fields", () => {
    expect(openCodeSummary({ settings: false, models: [], metadata: null })).toEqual({
      baseUrl: undefined,
      model: undefined,
      iconUrl: undefined,
    });
    expect(
      openCodeSummary({
        package: "custom",
        metadata: { nativeModelId: "missing" },
        models: { missing: { variants: [null, true] } },
      }).model,
    ).toBe("missing");
  });
});
