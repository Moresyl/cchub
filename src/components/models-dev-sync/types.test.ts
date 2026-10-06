import { afterEach, describe, expect, it } from "vitest";
import { isSelected, samePreferences, selectModel, validConfig, type CatalogEntry, type SyncConfig } from "./types";
import { readDraft, rememberDraft } from "./draft";
const config: SyncConfig = {
  autoSyncEnabled: false,
  includeCommonModels: true,
  selectedModelKeys: [],
  excludedCommonModelKeys: [],
  lastSyncAt: null,
  lastSyncError: null,
};
const common: CatalogEntry = {
  key: "demo/recent",
  modelId: "recent",
  modelName: "Recent",
  providerId: "demo",
  providerName: "Demo",
  releaseDate: "",
  isCommon: true,
  input: 1,
  output: 2,
  cacheRead: 0,
  cacheWrite: 0,
};
afterEach(() => {
  rememberDraft(null);
  sessionStorage.clear();
});
describe("pricing selections", () => {
  it("uses the backend common marker and removes explicit selection when deselecting a common model", () => {
    expect(isSelected(common, config)).toBe(true);
    const unselected = selectModel(common, { ...config, selectedModelKeys: [common.key] }, false);
    expect(isSelected(common, unselected)).toBe(false);
    expect(unselected.selectedModelKeys).toEqual([]);
    expect(isSelected(common, selectModel(common, unselected, true))).toBe(true);
    const other = { ...common, key: "demo/older", isCommon: false };
    expect(isSelected(other, config)).toBe(false);
    expect(isSelected(other, selectModel(other, config, true))).toBe(true);
    expect(isSelected(other, selectModel(other, selectModel(other, config, true), false))).toBe(false);
    expect(isSelected(common, { ...config, includeCommonModels: false })).toBe(false);
  });
  it("compares normalized preferences independently of sync metadata", () => {
    expect(
      samePreferences(
        { ...config, selectedModelKeys: [" b ", "a", "a"] },
        { ...config, selectedModelKeys: ["a", "b"], lastSyncAt: 100 },
      ),
    ).toBe(true);
    expect(samePreferences(config, { ...config, excludedCommonModelKeys: [common.key] })).toBe(false);
  });
  it("rejects malformed drafts and only retains actual unsaved preferences", () => {
    expect(validConfig(config)).toBe(true);
    for (const value of [
      null,
      {},
      { ...config, selectedModelKeys: [2] },
      { ...config, lastSyncAt: Infinity },
      { ...config, includeCommonModels: "true" },
    ])
      expect(validConfig(value)).toBe(false);
    sessionStorage.setItem("cchub:pricing-preferences-draft", "{broken");
    expect(readDraft()).toBeNull();
    rememberDraft({ baseline: config, draft: { ...config, autoSyncEnabled: true } });
    expect(readDraft()?.draft.autoSyncEnabled).toBe(true);
    rememberDraft({ baseline: config, draft: { ...config, lastSyncAt: 10 } });
    expect(readDraft()).toBeNull();
  });
});
