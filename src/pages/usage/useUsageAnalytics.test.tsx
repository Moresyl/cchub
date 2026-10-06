import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useUsageAnalytics } from "./useUsageAnalytics";
import type { UsageAnalytics, UsageFilters } from "./types";

const { invoke, listen } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function analytics(requests: number): UsageAnalytics {
  return {
    days: 7,
    start_date: "2026-10-01",
    end_date: "2026-10-07",
    summary: {
      total_requests: requests,
      success_requests: requests,
      success_rate: 100,
      input_tokens: 100,
      output_tokens: 5,
      total_tokens: 105,
      cache_read_tokens: 0,
      cache_creation_tokens: 0,
      total_cost_usd: "0.1",
    },
    trends: [],
    models: [],
    providers: [],
  };
}

const filters: UsageFilters = { days: 7, appId: "", providerName: "", model: "" };
let onRecord: () => void;
let dispose = vi.fn<() => void>();

beforeEach(() => {
  invoke.mockReset().mockResolvedValue(analytics(1));
  dispose = vi.fn<() => void>();
  listen.mockReset().mockImplementation((_event: string, callback: () => void) => {
    onRecord = callback;
    return Promise.resolve(dispose);
  });
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("useUsageAnalytics", () => {
  it("hides the previous filter snapshot and ignores both stale successes and failures", async () => {
    const first = deferred<UsageAnalytics>();
    const second = deferred<UsageAnalytics>();
    const third = deferred<UsageAnalytics>();
    invoke.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise).mockReturnValueOnce(third.promise);
    const { result, rerender } = renderHook((scope) => useUsageAnalytics(scope), { initialProps: filters });
    rerender({ ...filters, days: 30 });
    await act(async () => {
      second.resolve(analytics(30));
    });
    expect(result.current.data?.summary.total_requests).toBe(30);
    rerender({ ...filters, appId: "claude" });
    expect(result.current.data).toBeNull();
    expect(result.current.loading).toBe(true);
    await act(async () => {
      first.resolve(analytics(700));
    });
    expect(result.current.data).toBeNull();
    await act(async () => {
      third.reject("token=private; C:/private/database.sqlite");
    });
    expect(result.current.error).toBe(true);
    expect(result.current.data).toBeNull();
  });

  it("does not let a stale failure stop the active request", async () => {
    const old = deferred<UsageAnalytics>();
    const current = deferred<UsageAnalytics>();
    invoke.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
    const { result, rerender } = renderHook((scope) => useUsageAnalytics(scope), { initialProps: filters });
    rerender({ ...filters, model: "chosen-model" });
    await act(async () => {
      old.reject("stale failure");
    });
    expect(result.current.error).toBe(false);
    expect(result.current.loading).toBe(true);
    await act(async () => {
      current.resolve(analytics(2));
    });
    expect(result.current.data?.summary.total_requests).toBe(2);
    expect(result.current.loading).toBe(false);
  });

  it("retains the same-filter result after a failed refresh and recovers on retry", async () => {
    const { result } = renderHook(() => useUsageAnalytics(filters));
    await waitFor(() => expect(result.current.loading).toBe(false));
    invoke.mockRejectedValueOnce("private detail");
    await act(async () => {
      result.current.refresh();
    });
    expect(result.current.error).toBe(true);
    expect(result.current.data?.summary.total_requests).toBe(1);
    invoke.mockResolvedValueOnce(analytics(3));
    await act(async () => {
      result.current.refresh();
    });
    expect(result.current.error).toBe(false);
    expect(result.current.data?.summary.total_requests).toBe(3);
  });

  it("coalesces bursts and runs only one trailing refresh after events during a slow read", async () => {
    vi.useFakeTimers();
    const slow = deferred<UsageAnalytics>();
    const trailing = deferred<UsageAnalytics>();
    invoke.mockReturnValueOnce(slow.promise).mockReturnValueOnce(trailing.promise);
    const { result } = renderHook(() => useUsageAnalytics(filters));
    act(() => {
      for (let i = 0; i < 100; i++) onRecord();
      result.current.refresh();
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    await act(async () => {
      slow.resolve(analytics(1));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
    expect(invoke).toHaveBeenCalledTimes(2);
    await act(async () => {
      trailing.resolve(analytics(2));
    });
    act(() => {
      for (let i = 0; i < 100; i++) onRecord();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(249);
    });
    expect(invoke).toHaveBeenCalledTimes(2);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(invoke).toHaveBeenCalledTimes(3);
    expect(result.current.loading).toBe(false);
  });

  it("removes late subscriptions and pending event timers after unmount", async () => {
    vi.useFakeTimers();
    const subscription = deferred<() => void>();
    listen.mockImplementationOnce((_event: string, callback: () => void) => {
      onRecord = callback;
      return subscription.promise;
    });
    const { unmount } = renderHook(() => useUsageAnalytics(filters));
    await act(async () => {});
    act(() => onRecord());
    unmount();
    await act(async () => {
      subscription.resolve(dispose);
      await vi.advanceTimersByTimeAsync(500);
    });
    expect(dispose).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("handles listener rejection while keeping manual refresh available", async () => {
    listen.mockRejectedValueOnce(new Error("event bridge unavailable"));
    const { result } = renderHook(() => useUsageAnalytics(filters));
    await waitFor(() => expect(result.current.liveUnavailable).toBe(true));
    expect(result.current.data?.summary.total_requests).toBe(1);
    await act(async () => {
      result.current.refresh();
    });
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(result.current.error).toBe(false);
  });

  it("retains broader filter options without carrying them into a different app", async () => {
    const broad = analytics(2);
    broad.providers = ["alpha", "beta"].map((provider_name) => ({
      provider_name,
      app_id: "claude",
      requests: 1,
      success_rate: 100,
      total_tokens: 1,
      total_cost_usd: "0",
      avg_latency_ms: 1,
    }));
    broad.models = ["model-a", "model-b"].map((model) => ({
      model,
      requests: 1,
      success_rate: 100,
      total_tokens: 1,
      total_cost_usd: "0",
      avg_latency_ms: 1,
    }));
    invoke
      .mockResolvedValueOnce(broad)
      .mockResolvedValueOnce({ ...broad, providers: broad.providers.slice(0, 1), models: broad.models.slice(0, 1) })
      .mockResolvedValueOnce(analytics(0));
    const { result, rerender } = renderHook((scope) => useUsageAnalytics(scope), { initialProps: filters });
    await waitFor(() => expect(result.current.loading).toBe(false));
    rerender({ ...filters, providerName: "alpha" });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.providers).toEqual(["alpha", "beta"]);
    expect(result.current.models).toEqual(["model-a", "model-b"]);
    rerender({ ...filters, appId: "codex" });
    expect(result.current.providers).toEqual([]);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.models).toEqual([]);
    expect(dispose).toHaveBeenCalledTimes(2);
  });
});
