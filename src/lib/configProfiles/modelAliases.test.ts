import { describe, expect, it } from "vitest";
import {
  buildStructuredConfig,
  createDefaultStructuredFields,
  parseStructuredConfig,
  applyPresetToFields,
} from "./index";
import { parseModelAliases, previewModelAlias, serializeModelAliases, validateModelAliases } from "./modelAliases";
import { mergeDraftFields, mergeSharedDraftFields } from "../../pages/profiles/draftMerge";

const aliases = [
  { model: "*", upstream: "relay/*" },
  { model: "core", upstream: "vendor/*/pro-*" },
];

describe("provider-local model aliases", () => {
  it("prefers exact names and expands every wildcard case-sensitively", () => {
    expect(previewModelAlias(aliases, "core")).toBe("vendor/core/pro-core");
    expect(previewModelAlias(aliases, "Core")).toBe("relay/Core");
    expect(previewModelAlias([], "custom")).toBe("custom");
    expect(previewModelAlias([{ model: "*", upstream: "*".repeat(1024) }], "aa")).toBeNull();
  });

  it.each(["claude", "codex", "gemini", "grokbuild", "openclaw", "opencode", "hermes", "pi"])(
    "round trips %s without replacing its catalog or model name",
    (tool) => {
      const fields = { ...createDefaultStructuredFields(tool), model: "core", localProxyModelAliases: aliases };
      const content = buildStructuredConfig(tool, fields);
      expect(JSON.parse(content).metadata.localProxyModelAliases).toEqual(aliases);
      const parsed = parseStructuredConfig(tool, content);
      expect(parsed.localProxyModelAliases).toEqual(aliases);
      expect(parsed.model).toBe("core");
    },
  );

  it("retains partial rows and malformed raw values instead of silently deleting them", () => {
    expect(serializeModelAliases([{ model: " core ", upstream: " " }])).toEqual([{ model: "core", upstream: "" }]);
    expect(serializeModelAliases([{ model: " ", upstream: " " }])).toEqual([]);
    for (const raw of [null, { secret: "original" }, [null], [{ model: 7, upstream: "a" }]]) {
      const parsed = parseStructuredConfig("claude", JSON.stringify({ metadata: { localProxyModelAliases: raw } }));
      expect(parsed.localProxyModelAliasesRaw).toEqual(raw);
      expect(validateModelAliases([], parsed.localProxyModelAliasesRaw)).toEqual({ kind: "shape" });
      expect(JSON.parse(buildStructuredConfig("claude", parsed)).metadata.localProxyModelAliases).toEqual(raw);
      const repaired = mergeDraftFields(parsed, { localProxyModelAliases: [], localProxyModelAliasesRaw: undefined });
      expect(JSON.parse(buildStructuredConfig("claude", repaired)).metadata.localProxyModelAliases).toEqual([]);
    }
    expect(parseModelAliases(undefined)).toEqual({});
  });

  it("rejects duplicates, partial rows, unsafe characters and byte/count limits", () => {
    expect(
      validateModelAliases([
        { model: "a", upstream: "x" },
        { model: " a ", upstream: "y" },
      ]),
    ).toEqual({ kind: "duplicate", row: 2 });
    for (const row of [
      { model: "a", upstream: "" },
      { model: "a*", upstream: "x" },
      { model: "a", upstream: "x?q" },
      { model: "a", upstream: "中".repeat(342) },
    ]) {
      expect(validateModelAliases([row])).toEqual({ kind: "invalid", row: 1 });
    }
    expect(validateModelAliases(Array.from({ length: 129 }, () => ({ model: "", upstream: "" })))).toEqual({
      kind: "limit",
    });
    expect(validateModelAliases([{ model: "", upstream: "" }])).toBeNull();
  });

  it("preserves aliases through preset and shared edits and supports explicit reset", () => {
    const current = { ...createDefaultStructuredFields("claude"), localProxyModelAliases: aliases };
    expect(applyPresetToFields("claude", "custom", current).localProxyModelAliases).toEqual(aliases);
    const empty = createDefaultStructuredFields("claude");
    expect(mergeSharedDraftFields(current, "claude", empty, true).localProxyModelAliases).toEqual(aliases);
    expect(
      mergeSharedDraftFields(current, "claude", { ...empty, localProxyModelAliases: [] }, true).localProxyModelAliases,
    ).toEqual([]);
    const catalog = { toolId: "claude" as const, fetchedAt: "2026-10-01T00:00:00Z", models: [{ id: "core" }] };
    expect(mergeDraftFields({ ...current, modelCatalog: catalog }, { localProxyModelAliases: [] }).modelCatalog).toBe(
      catalog,
    );
  });
});
