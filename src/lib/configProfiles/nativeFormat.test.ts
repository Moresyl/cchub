import { describe, expect, it } from "vitest";
import { isNativeOpenCodeConfig } from "./nativeFormat";

describe("native configuration detection", () => {
  it("recognizes full documents and native fragments including empty overrides", () => {
    for (const value of [
      { providers: { builtin: {} } },
      { settings: { apiKey: "fixture" } },
      { package: "custom" },
      { canonical: "openai" },
      { metadata: { nativeFormat: "providers" }, models: { m: {} } },
    ]) {
      expect(isNativeOpenCodeConfig(JSON.stringify(value))).toBe(true);
    }
  });

  it("keeps legacy fragments and malformed drafts out of the native parser", () => {
    for (const value of [
      "{",
      "null",
      "[]",
      "false",
      JSON.stringify({ npm: "custom", options: {} }),
      JSON.stringify({ metadata: { nativeFormat: "provider" } }),
    ]) {
      expect(isNativeOpenCodeConfig(value)).toBe(false);
    }
  });
});
