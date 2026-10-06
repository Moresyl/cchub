import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { rememberDraft, readDraft } from "./draft";
import { usePricingSettings } from "./usePricingSettings";
import type { SyncConfig, SyncState } from "./types";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const config: SyncConfig = {
  autoSyncEnabled: false,
  includeCommonModels: true,
  selectedModelKeys: [],
  excludedCommonModelKeys: [],
  lastSyncAt: null,
  lastSyncError: null,
};
const state = (config: SyncConfig): SyncState => ({ config, configPath: "C:/demo/pricing.json" });
const own = { ...config, autoSyncEnabled: true, selectedModelKeys: ["team/own"] };
const latest = { ...config, includeCommonModels: false, selectedModelKeys: ["team/saved"], lastSyncAt: 42 };
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((yes) => {
    resolve = yes;
  });
  return { promise, resolve };
}
beforeEach(() => {
  rememberDraft(null);
  invoke.mockReset().mockResolvedValue(state(config));
});
afterEach(() => {
  cleanup();
  rememberDraft(null);
});
async function conflicted() {
  const hook = renderHook(usePricingSettings);
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
  act(() => hook.result.current.update(() => own));
  invoke.mockRejectedValueOnce("PRICING_SETTINGS_CONFLICT");
  await act(async () => {
    await hook.result.current.save();
  });
  return hook;
}
describe("pricing review operation ownership", () => {
  it("reads fresh settings, merges independent changes and saves with the reviewed revision", async () => {
    const hook = await conflicted();
    invoke.mockResolvedValueOnce(state(latest));
    await act(async () => {
      await hook.result.current.reviewChanges();
    });
    const id = hook.result.current.review!.id;
    act(() => {
      expect(hook.result.current.applyReview(id, { models: new Map() })).toBe(true);
    });
    expect(hook.result.current.draft).toEqual({
      ...latest,
      autoSyncEnabled: true,
      selectedModelKeys: ["team/own", "team/saved"],
    });
    expect(hook.result.current.baseline).toEqual(latest);
    expect(hook.result.current.failure).toBeNull();
    expect(hook.result.current.dirty).toBe(true);
    expect(readDraft()?.baseline).toEqual(latest);
    const count = invoke.mock.calls.length;
    act(() => {
      expect(hook.result.current.applyReview(id, { models: new Map() })).toBe(false);
    });
    expect(invoke.mock.calls).toHaveLength(count);
    invoke.mockResolvedValueOnce(state(hook.result.current.draft!));
    await act(async () => {
      await hook.result.current.save();
    });
    expect(invoke).toHaveBeenLastCalledWith("save_models_dev_sync_config", {
      config: { ...latest, autoSyncEnabled: true, selectedModelKeys: ["team/own", "team/saved"] },
      expectedConfig: latest,
    });
  });
  it("ignores canceled reads and does not let obsolete responses take over a new review", async () => {
    const hook = await conflicted();
    const obsolete = deferred<SyncState>();
    invoke.mockReturnValueOnce(obsolete.promise);
    let read!: Promise<void>;
    act(() => {
      read = hook.result.current.reviewChanges();
    });
    const id = hook.result.current.review!.id;
    act(() => hook.result.current.cancelReview());
    expect(hook.result.current.draft).toEqual(own);
    invoke.mockResolvedValueOnce(state(latest));
    await act(async () => {
      await hook.result.current.reviewChanges();
      obsolete.resolve(state({ ...latest, autoSyncEnabled: false }));
      await read;
    });
    expect(hook.result.current.review?.latest?.config).toEqual(latest);
    act(() => {
      expect(hook.result.current.applyReview(id, { models: new Map() })).toBe(false);
    });
    expect(hook.result.current.draft).toEqual(own);
  });
  it("keeps failed review reads retryable and blocks writes and background edits during review", async () => {
    const hook = await conflicted();
    invoke.mockRejectedValueOnce("token=private; C:/private");
    await act(async () => {
      await hook.result.current.reviewChanges();
    });
    expect(hook.result.current.review?.error).toBe(true);
    expect(JSON.stringify(hook.result.current)).not.toContain("token=private");
    const count = invoke.mock.calls.length;
    await act(async () => {
      hook.result.current.update(() => config);
      await hook.result.current.refresh(true);
      await hook.result.current.sync();
    });
    expect(invoke.mock.calls).toHaveLength(count);
    expect(hook.result.current.draft).toEqual(own);
    act(() => {
      expect(hook.result.current.applyReview(hook.result.current.review!.id, { models: new Map() })).toBe(false);
    });
    invoke.mockResolvedValueOnce(state(latest));
    await act(async () => {
      await hook.result.current.reviewChanges();
    });
    expect(hook.result.current.review?.error).toBe(false);
  });
  it("rejects new external changes after applying and preserves the merged draft on route changes", async () => {
    const hook = await conflicted();
    invoke.mockResolvedValueOnce(state(latest));
    await act(async () => {
      await hook.result.current.reviewChanges();
    });
    act(() => {
      hook.result.current.applyReview(hook.result.current.review!.id, { models: new Map() });
    });
    const merged = hook.result.current.draft;
    invoke.mockRejectedValueOnce("PRICING_SETTINGS_CONFLICT");
    await act(async () => {
      await hook.result.current.sync();
    });
    expect(hook.result.current.failure).toBe("conflict");
    expect(hook.result.current.draft).toEqual(merged);
    expect(invoke.mock.calls.some(([command]) => command === "sync_models_dev_pricing")).toBe(false);
    hook.unmount();
    invoke.mockResolvedValue(state({ ...latest, selectedModelKeys: ["team/newer"] }));
    const restored = renderHook(usePricingSettings);
    await waitFor(() => expect(restored.result.current.loading).toBe(false));
    expect(restored.result.current.draft).toEqual(merged);
    expect(restored.result.current.failure).toBe("conflict");
  });
  it("discards review results after unmount without losing the unsaved draft", async () => {
    const hook = await conflicted();
    const reading = deferred<SyncState>();
    invoke.mockReturnValueOnce(reading.promise);
    let operation!: Promise<void>;
    act(() => {
      operation = hook.result.current.reviewChanges();
    });
    const id = hook.result.current.review!.id;
    hook.unmount();
    await act(async () => {
      reading.resolve(state(latest));
      await operation;
    });
    expect(hook.result.current.applyReview(id, { models: new Map() })).toBe(false);
    expect(readDraft()?.draft).toEqual(own);
  });
});
