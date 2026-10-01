import { describe, expect, it } from "vitest";
import { codexReasoningChoices } from "./codexReasoning";

describe("Codex reasoning capability choices", () => {
  it.each([undefined, null])("keeps unknown capabilities distinct from an explicit empty list (%s)", (levels) => {
    expect(codexReasoningChoices("future-effort", levels)).toEqual({
      reported: false,
      levels: ["low", "medium", "high", "xhigh"],
      unsupported: false,
    });
    expect(codexReasoningChoices("", [])).toEqual({ reported: true, levels: [], unsupported: false });
    expect(codexReasoningChoices("high", [])).toEqual({ reported: true, levels: [], unsupported: true });
  });

  it("uses reported levels in provider order without inventing or duplicating options", () => {
    const levels = [" max ", "none", "max", " ", "minimal"];
    expect(codexReasoningChoices("max", levels)).toEqual({
      reported: true,
      levels: ["max", "none", "minimal"],
      unsupported: false,
    });
    expect(codexReasoningChoices("high", levels).unsupported).toBe(true);
    expect(codexReasoningChoices("", levels).unsupported).toBe(false);
    expect(levels).toEqual([" max ", "none", "max", " ", "minimal"]);
  });
});
