import { describe, expect, it } from "vitest";

import {
  buildResumeCommand,
  buildSessionToolFilters,
  countSessionHits,
  sameSession,
  sessionDetailArgs,
  sessionSelectionKey,
  TOOL_ORDER,
} from "./helpers";
import { sessionFixture } from "./testFixtures";

describe("session helpers", () => {
  it("keeps Codex twin identity stable without merging other owners or backends", () => {
    const plain = sessionFixture({ source_backend: "jsonl" });
    const packed = { ...plain, source_path: `${plain.source_path}.zst` };
    expect(sameSession(plain, packed)).toBe(true);
    expect(sameSession(plain, { ...packed, source_path: "C:/other/one.jsonl.zst" })).toBe(false);
    expect(sameSession(plain, { ...packed, source_backend: "sqlite" })).toBe(false);
    expect(sameSession({ ...plain, tool_id: "claude" }, { ...packed, tool_id: "claude" })).toBe(false);
  });
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

  it("separates delimiter collisions, source paths, backends and tools", () => {
    const session = sessionFixture();
    for (const overrides of [
      { source_path: "C:/fixture/two.jsonl" },
      { source_backend: "sqlite" },
      { tool_id: "claude" },
      { id: "other" },
    ]) {
      expect(sameSession(session, sessionFixture(overrides))).toBe(false);
    }
    expect(sessionSelectionKey(sessionFixture({ id: "a:b", source_path: "c" }))).not.toBe(
      sessionSelectionKey(sessionFixture({ id: "a", source_path: "b:c" })),
    );
    expect(sameSession(session, sessionFixture({ title: "Renamed" }))).toBe(true);
    expect(sameSession(null, session)).toBe(false);
    expect(sameSession(session, undefined)).toBe(false);
    expect(sameSession(null, null)).toBe(false);
  });

  it("passes the exact source and latest metadata when loading details", () => {
    expect(
      sessionDetailArgs(
        sessionFixture({ title: "Renamed", source_backend: "sqlite", source_path: "C:/fixture/state.sqlite" }),
      ),
    ).toEqual({
      toolId: "codex",
      sessionId: "shared-id",
      sourcePath: "C:/fixture/state.sqlite",
      sourceKind: "jsonl",
      sourceBackend: "sqlite",
      cwd: "C:/fixture/project",
      title: "Renamed",
      preview: "Fixture conversation",
      createdAt: null,
      updatedAt: null,
      messageCount: 2,
      inputTokens: 10,
      outputTokens: 5,
      tokensUsed: 15,
      canResume: true,
      canDelete: true,
    });
  });

  it("searches session IDs case-insensitively without matching an empty query", () => {
    expect(countSessionHits(sessionFixture(), " SHARED-ID ")).toBe(1);
    expect(countSessionHits(sessionFixture(), "missing")).toBe(0);
    expect(countSessionHits(sessionFixture(), " ")).toBe(0);
  });
});
