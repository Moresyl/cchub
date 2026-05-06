import { describe, expect, it } from "vitest";
import { tReplace } from "./i18n";

describe("tReplace", () => {
  it("replaces string and number placeholders", () => {
    expect(
      tReplace("Loaded {count} items from {source}", {
        count: 3,
        source: "cache",
      }),
    ).toBe("Loaded 3 items from cache");
  });

  it("leaves missing placeholders unchanged", () => {
    expect(tReplace("Hello {name}, {missing}", { name: "CCHub" })).toBe("Hello CCHub, {missing}");
  });

  it("replaces only the first occurrence for each key", () => {
    expect(tReplace("{name} -> {name}", { name: "Claude" })).toBe("Claude -> {name}");
  });
});
