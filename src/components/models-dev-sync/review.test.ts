import { describe, expect, it } from "vitest";
import { mergePricingReview, pricingChanges, type PricingReview, type ReviewChoices } from "./review";
import type { SyncConfig } from "./types";
const base: SyncConfig = {
  autoSyncEnabled: false,
  includeCommonModels: true,
  selectedModelKeys: ["team/keep", "team/remove"],
  excludedCommonModelKeys: [],
  lastSyncAt: null,
  lastSyncError: null,
};
const choices = (): ReviewChoices => ({ models: new Map() });
const review = (local: SyncConfig, saved: SyncConfig, baseline = base): PricingReview => ({
  id: 1,
  baseline,
  draft: local,
  latest: { config: saved, configPath: "C:/demo/pricing.json" },
  loading: false,
  error: false,
});
describe("pricing changes review", () => {
  it("keeps independent additions, removals, flags and latest runtime status", () => {
    const own = { ...base, autoSyncEnabled: true, selectedModelKeys: ["team/keep", "team/own"], lastSyncAt: 1 };
    const latest = {
      ...base,
      includeCommonModels: false,
      selectedModelKeys: [...base.selectedModelKeys, "team/saved"],
      lastSyncAt: 42,
      lastSyncError: "PRICING_SYNC_FAILED",
    };
    const merged = mergePricingReview(review(own, latest), choices());
    expect(merged).toEqual({
      ...latest,
      autoSyncEnabled: true,
      selectedModelKeys: ["team/keep", "team/own", "team/saved"],
    });
    expect(own.selectedModelKeys).toEqual(["team/keep", "team/own"]);
    expect(latest.selectedModelKeys).toContain("team/remove");
    expect(pricingChanges(base, own, latest).map((change) => change.key)).toEqual([
      "team/own",
      "team/remove",
      "team/saved",
    ]);
  });
  it("requires a deliberate choice for competing explicit and common-exclusion changes", () => {
    const own = { ...base, selectedModelKeys: [...base.selectedModelKeys, "team/same"] };
    const latest = { ...base, excludedCommonModelKeys: ["team/same"] };
    const current = review(own, latest);
    expect(pricingChanges(base, own, latest)).toEqual([
      { key: "team/same", baseline: 0, local: 1, saved: 2, merged: 1, conflict: true },
    ]);
    expect(mergePricingReview(current, choices())).toBeNull();
    const local = mergePricingReview(current, { models: new Map([["team/same", "local"]]) });
    expect(local?.selectedModelKeys).toContain("team/same");
    expect(local?.excludedCommonModelKeys).not.toContain("team/same");
    const saved = mergePricingReview(current, { models: new Map([["team/same", "saved"]]) });
    expect(saved?.selectedModelKeys).not.toContain("team/same");
    expect(saved?.excludedCommonModelKeys).toContain("team/same");
  });
  it("lets users override an automatic resolution, including unchanged draft flags", () => {
    const latest = {
      ...base,
      autoSyncEnabled: true,
      includeCommonModels: false,
      selectedModelKeys: [...base.selectedModelKeys, "team/new"],
    };
    expect(
      mergePricingReview(review(base, latest), {
        autoSync: "local",
        common: "local",
        models: new Map([["team/new", "local"]]),
      }),
    ).toEqual({ ...base, lastSyncAt: latest.lastSyncAt });
  });
  it("normalizes whitespace and duplicates and safely handles prototype-like keys", () => {
    const own = { ...base, selectedModelKeys: [...base.selectedModelKeys, " __proto__ ", "", "__proto__"] };
    expect(mergePricingReview(review(own, base), choices())?.selectedModelKeys).toEqual([
      "__proto__",
      "team/keep",
      "team/remove",
    ]);
    expect(pricingChanges(base, own, own)[0]?.conflict).toBe(false);
  });
  it("rejects loading, failed reads and merged selections beyond supported limits", () => {
    const current = review(base, base);
    expect(mergePricingReview({ ...current, loading: true }, choices())).toBeNull();
    expect(mergePricingReview({ ...current, error: true }, choices())).toBeNull();
    expect(mergePricingReview({ ...current, latest: null }, choices())).toBeNull();
    const empty = { ...base, selectedModelKeys: [] };
    const local = { ...empty, selectedModelKeys: Array.from({ length: 16000 }, (_, index) => `team/own-${index}`) };
    const latest = { ...empty, selectedModelKeys: ["team/remote"] };
    expect(mergePricingReview(review(local, latest, empty), choices())).toBeNull();
    expect(
      mergePricingReview(review(local, latest, empty), { models: new Map([["team/remote", "local"]]) })
        ?.selectedModelKeys,
    ).toHaveLength(16000);
  });
});
