import { describe, expect, it } from "vitest";

import { buildResumeCommand, buildSessionToolFilters, TOOL_ORDER } from "./helpers";

describe("session helpers", () => {
  it("keeps all nine managed tools available to the session filter", () => {
    const filters = buildSessionToolFilters(TOOL_ORDER, "All Apps");

    expect(filters.map((filter) => filter.id)).toEqual([
      "all",
      "claude",
      "codex",
      "gemini",
      "grokbuild",
      "opencode",
      "openclaw",
      "hermes",
      "pi",
      "mcode",
    ]);
  });

  it("builds native resume commands for Grok Build and MiniMax Code", () => {
    expect(buildResumeCommand("grokbuild", "grok-session")).toBe("grok --resume grok-session");
    expect(buildResumeCommand("mcode", "mcode-session")).toBe("mcode --session mcode-session");
  });
});
