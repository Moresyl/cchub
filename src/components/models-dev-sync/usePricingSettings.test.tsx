import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
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
const state = (value = config): SyncState => ({ config: structuredClone(value), configPath: "C:/demo/pricing.json" });
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
beforeEach(() => {
  rememberDraft(null);
  invoke.mockReset().mockResolvedValue(state());
});
afterEach(() => {
  cleanup();
  rememberDraft(null);
  vi.restoreAllMocks();
});
async function loaded() {
  const hook = renderHook(usePricingSettings);
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
  return hook;
}
const edited = (draft: SyncConfig) => ({ ...draft, autoSyncEnabled: true, selectedModelKeys: ["demo/chosen"] });

describe("pricing preferences ownership", () => {
  it("keeps all edits across refreshes, route remounts and unavailable session storage", async () => {
    const first = await loaded();
    act(() => first.result.current.update(edited));
    await act(async () => {
      await first.result.current.refresh();
    });
    expect(first.result.current.draft).toMatchObject(edited(config));
    expect(first.result.current.dirty).toBe(true);
    expect(invoke.mock.calls.every(([command]) => command === "get_models_dev_sync_config")).toBe(true);
    first.unmount();
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("unavailable");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("unavailable");
    });
    const restored = await loaded();
    expect(restored.result.current.draft?.selectedModelKeys).toEqual(["demo/chosen"]);
    expect(restored.result.current.dirty).toBe(true);
  });

  it("saves the entire draft before syncing and coalesces actions synchronously", async () => {
    const hook = await loaded();
    act(() => hook.result.current.update(edited));
    const save = deferred<SyncState>();
    invoke.mockImplementation((command, args) =>
      command === "save_models_dev_sync_config"
        ? save.promise
        : command === "sync_models_dev_pricing"
          ? Promise.resolve({ skipped: false, selected: 7, imported: 6, changed: 2, syncedAt: 42 })
          : Promise.resolve(state(args?.config)),
    );
    let operation!: Promise<void>;
    act(() => {
      operation = hook.result.current.sync();
      void hook.result.current.sync();
      void hook.result.current.save();
      hook.result.current.update((draft) => ({ ...draft, autoSyncEnabled: false }));
    });
    expect(invoke.mock.calls.filter(([command]) => command === "save_models_dev_sync_config")).toHaveLength(1);
    expect(invoke.mock.calls.some(([command]) => command === "sync_models_dev_pricing")).toBe(false);
    expect(invoke).toHaveBeenCalledWith("save_models_dev_sync_config", {
      config: edited(config),
      expectedConfig: config,
    });
    await act(async () => {
      save.resolve(state({ ...edited(config), lastSyncAt: 21 }));
      await operation;
    });
    expect(invoke).toHaveBeenCalledWith("sync_models_dev_pricing", {
      force: true,
      expectedConfig: { ...edited(config), lastSyncAt: 21 },
    });
    expect(hook.result.current.result?.imported).toBe(6);
    expect(hook.result.current.state?.config.lastSyncAt).toBe(42);
    expect(hook.result.current.dirty).toBe(false);
    expect(readDraft()).toBeNull();
  });

  it("keeps a failed save retryable without issuing a sync or exposing its error", async () => {
    const hook = await loaded();
    act(() => hook.result.current.update(edited));
    invoke.mockRejectedValueOnce("token=private; C:/private/database");
    await act(async () => {
      await hook.result.current.sync();
    });
    expect(hook.result.current.failure).toBe("save");
    expect(hook.result.current.dirty).toBe(true);
    expect(JSON.stringify(hook.result.current)).not.toContain("token=private");
    expect(invoke.mock.calls.some(([command]) => command === "sync_models_dev_pricing")).toBe(false);
    invoke.mockResolvedValueOnce(state(edited(config)));
    await act(async () => {
      await hook.result.current.save();
    });
    expect(hook.result.current.failure).toBeNull();
    expect(hook.result.current.dirty).toBe(false);
  });

  it("keeps saved choices after sync failure and retries only the sync", async () => {
    const hook = await loaded();
    act(() => hook.result.current.update(edited));
    invoke.mockResolvedValueOnce(state(edited(config))).mockRejectedValueOnce("network failure");
    await act(async () => {
      await hook.result.current.sync();
    });
    expect(hook.result.current.failure).toBe("sync");
    expect(hook.result.current.dirty).toBe(false);
    invoke.mockResolvedValueOnce({ skipped: false, imported: 1, changed: 1, selected: 1, syncedAt: 10 });
    await act(async () => {
      await hook.result.current.sync();
    });
    expect(invoke.mock.calls.filter(([command]) => command === "save_models_dev_sync_config")).toHaveLength(1);
    expect(hook.result.current.failure).toBeNull();
  });

  it("detects changed settings without replacing the draft and reloads only after explicit discard", async () => {
    const hook = await loaded();
    act(() => hook.result.current.update(edited));
    const latest = { ...config, includeCommonModels: false };
    invoke.mockResolvedValue(state(latest));
    await act(async () => {
      await hook.result.current.refresh();
    });
    expect(hook.result.current.failure).toBe("conflict");
    expect(hook.result.current.draft).toMatchObject(edited(config));
    const count = invoke.mock.calls.length;
    await act(async () => {
      await hook.result.current.sync();
    });
    expect(invoke.mock.calls).toHaveLength(count);
    invoke.mockRejectedValueOnce("failed read");
    await act(async () => {
      await hook.result.current.refresh(true);
    });
    expect(hook.result.current.dirty).toBe(true);
    expect(hook.result.current.failure).toBe("read");
    await act(async () => {
      await hook.result.current.refresh(true);
    });
    expect(hook.result.current.draft).toEqual(latest);
    expect(hook.result.current.dirty).toBe(false);
  });

  it("handles backend save conflicts and disables repeated writes", async () => {
    const hook = await loaded();
    act(() => hook.result.current.update(edited));
    invoke.mockRejectedValueOnce("PRICING_SETTINGS_CONFLICT");
    await act(async () => {
      await hook.result.current.save();
    });
    expect(hook.result.current.failure).toBe("conflict");
    expect(hook.result.current.dirty).toBe(true);
    expect(hook.result.current.blocked).toBe(true);
  });

  it("shows initial read failure and recovers without writing defaults", async () => {
    invoke.mockRejectedValueOnce("invalid stored config");
    const hook = await loaded();
    expect(hook.result.current.failure).toBe("read");
    expect(hook.result.current.draft).toBeNull();
    await act(async () => {
      await hook.result.current.save();
      await hook.result.current.refresh();
    });
    expect(hook.result.current.draft).toEqual(config);
    expect(invoke.mock.calls.every(([command]) => command === "get_models_dev_sync_config")).toBe(true);
  });

  it("rejects obsolete StrictMode reads and does not run follow-up writes after unmount", async () => {
    const obsolete = deferred<SyncState>();
    const active = deferred<SyncState>();
    invoke.mockReturnValueOnce(obsolete.promise).mockReturnValueOnce(active.promise);
    const hook = renderHook(usePricingSettings, { wrapper: StrictMode });
    await act(async () => {
      active.resolve(state());
    });
    await act(async () => {
      obsolete.resolve(state({ ...config, autoSyncEnabled: true }));
    });
    expect(hook.result.current.draft?.autoSyncEnabled).toBe(false);
    act(() => hook.result.current.update(edited));
    const save = deferred<SyncState>();
    invoke.mockReturnValueOnce(save.promise);
    let operation!: Promise<void>;
    act(() => {
      operation = hook.result.current.sync();
    });
    hook.unmount();
    await act(async () => {
      save.resolve(state(edited(config)));
      await operation;
    });
    expect(invoke.mock.calls.some(([command]) => command === "sync_models_dev_pricing")).toBe(false);
    expect(readDraft()?.draft.selectedModelKeys).toEqual(["demo/chosen"]);
  });
});
