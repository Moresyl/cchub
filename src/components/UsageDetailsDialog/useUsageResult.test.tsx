import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigProfile } from "../../pages/profiles/helpers";
import { useUsageResult, type UsageResult } from "./useUsageResult";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function profile(id: string, config_snapshot = "{}", tool_id = "claude"): ConfigProfile {
  return { id, name: id, tool_id, config_snapshot, sort_order: 0, created_at: null, updated_at: null };
}
function deferred() {
  let resolve!: (result: UsageResult) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<UsageResult>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
const success: UsageResult = { success: true, data: [{ remaining: 12, unit: "USD" }] };

beforeEach(() => vi.mocked(invoke).mockReset());

describe("provider usage request ownership", () => {
  it("ignores an old success after switching providers", async () => {
    const old = deferred();
    const next = deferred();
    vi.mocked(invoke).mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
    const hook = renderHook(({ selected }) => useUsageResult(selected), {
      initialProps: { selected: profile("first") },
    });
    hook.rerender({ selected: profile("second") });
    expect(hook.result.current.result).toBeNull();
    expect(hook.result.current.loading).toBe(true);
    await act(async () => old.resolve(success));
    expect(hook.result.current.result).toBeNull();
    expect(hook.result.current.loading).toBe(true);
    const value = { success: true, data: [{ remaining: 25 }] };
    await act(async () => next.resolve(value));
    expect(hook.result.current.result).toEqual(value);
    expect(invoke).toHaveBeenLastCalledWith("queryProviderUsage", { providerId: "second", app: "claude" });
  });

  it("invalidates data when credentials or the tool change, even with the same profile ID", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(success);
    const next = deferred();
    vi.mocked(invoke).mockReturnValueOnce(next.promise);
    const hook = renderHook(({ selected }) => useUsageResult(selected), { initialProps: { selected: profile("one") } });
    await waitFor(() => expect(hook.result.current.result).toEqual(success));
    hook.rerender({ selected: profile("one", '{"key":"new"}', "codex") });
    expect(hook.result.current.result).toBeNull();
    expect(hook.result.current.updatedAt).toBeNull();
    await act(async () => next.resolve({ success: true, data: [{ remaining: 1 }] }));
    expect(hook.result.current.result?.data).toEqual([{ remaining: 1 }]);
  });

  it("retains only the same configuration's last good data after transient failure", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(success).mockRejectedValueOnce(new Error("Network unavailable"));
    const hook = renderHook(() => useUsageResult(profile("one")));
    await waitFor(() => expect(hook.result.current.result).toEqual(success));
    const updated = hook.result.current.updatedAt;
    await act(async () => hook.result.current.refresh());
    expect(hook.result.current.result).toEqual(success);
    expect(hook.result.current.error).toContain("Network unavailable");
    expect(hook.result.current.updatedAt).toBe(updated);
    expect(hook.result.current.loading).toBe(false);
  });

  it("replaces last good data on a deterministic credential failure", async () => {
    const failure = { success: false, data: [], error: "HTTP 401" };
    vi.mocked(invoke).mockResolvedValueOnce(success).mockResolvedValueOnce(failure);
    const hook = renderHook(() => useUsageResult(profile("one")));
    await waitFor(() => expect(hook.result.current.result).toEqual(success));
    await act(async () => hook.result.current.refresh());
    expect(hook.result.current.result).toEqual(failure);
    expect(hook.result.current.updatedAt).toBeNull();
  });

  it("does not overlap refreshes and can retry after rejection", async () => {
    const pending = deferred();
    vi.mocked(invoke).mockReturnValueOnce(pending.promise).mockResolvedValueOnce(success);
    const hook = renderHook(() => useUsageResult(profile("one")));
    await act(async () => {
      void hook.result.current.refresh();
      void hook.result.current.refresh();
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    await act(async () => pending.reject(new Error("Timeout")));
    await act(async () => hook.result.current.refresh());
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(hook.result.current.error).toBeNull();
  });

  it("ignores old failures after closing and reopening the dialog", async () => {
    const old = deferred();
    const next = deferred();
    vi.mocked(invoke).mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
    const hook = renderHook(({ selected }: { selected: ConfigProfile | null }) => useUsageResult(selected), {
      initialProps: { selected: profile("one") as ConfigProfile | null },
    });
    hook.rerender({ selected: null });
    expect(hook.result.current.result).toBeNull();
    hook.rerender({ selected: profile("one") });
    await act(async () => old.reject(new Error("Old timeout")));
    expect(hook.result.current.error).toBeNull();
    expect(hook.result.current.loading).toBe(true);
    await act(async () => next.resolve(success));
    expect(hook.result.current.result).toEqual(success);
  });
});
