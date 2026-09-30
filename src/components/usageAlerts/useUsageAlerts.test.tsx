import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useUsageAlerts } from "./useUsageAlerts";
import type { AlertOverview } from "./types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
const empty: AlertOverview = { rules: [], events: [], polling: false };
beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset().mockResolvedValue(vi.fn());
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

describe("usage alert request lifecycle", () => {
  it("does not hide a save failure behind a newer successful reload", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(empty);
    const hook = renderHook(useUsageAlerts);
    await waitFor(() => expect(hook.result.current.data).toEqual(empty));
    const failed = deferred<AlertOverview>();
    vi.mocked(invoke).mockReturnValueOnce(failed.promise).mockResolvedValueOnce(empty);
    let pending!: Promise<boolean>;
    act(() => {
      pending = hook.result.current.action("set_usage_alert_rule");
    });
    await act(async () => {
      await hook.result.current.refresh();
    });
    await act(async () => {
      failed.reject(new Error("Configuration changed"));
      await pending;
    });
    expect(hook.result.current.error).toContain("Configuration changed");
    expect(hook.result.current.data).toEqual(empty);
  });
  it("ignores late initial results after a newer reload", async () => {
    const old = deferred<AlertOverview>();
    vi.mocked(invoke)
      .mockReturnValueOnce(old.promise)
      .mockResolvedValueOnce({ ...empty, polling: true });
    const hook = renderHook(useUsageAlerts);
    await act(async () => {
      await hook.result.current.refresh();
    });
    expect(hook.result.current.data?.polling).toBe(true);
    await act(async () => old.resolve(empty));
    expect(hook.result.current.data?.polling).toBe(true);
  });

  it("blocks duplicate actions and preserves last data on a failure", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(empty);
    const hook = renderHook(useUsageAlerts);
    await waitFor(() => expect(hook.result.current.data).toEqual(empty));
    const next = deferred<AlertOverview>();
    vi.mocked(invoke).mockReturnValueOnce(next.promise);
    let pending!: Promise<boolean>;
    act(() => {
      pending = hook.result.current.action("check_usage_alerts");
    });
    await act(async () => expect(await hook.result.current.action("check_usage_alerts")).toBe(false));
    expect(invoke).toHaveBeenCalledTimes(2);
    await act(async () => {
      next.resolve(empty);
      await pending;
    });
    expect(hook.result.current.busy).toBe(false);
    vi.mocked(invoke).mockRejectedValueOnce(new Error("Unavailable"));
    await act(async () => {
      await hook.result.current.refresh();
    });
    expect(hook.result.current.data).toEqual(empty);
    expect(hook.result.current.error).toContain("Unavailable");
  });

  it("removes a listener whose registration completes after unmount", async () => {
    const registration = deferred<() => void>();
    const stop = vi.fn();
    vi.mocked(listen).mockReturnValueOnce(registration.promise);
    vi.mocked(invoke).mockResolvedValue(empty);
    const hook = renderHook(useUsageAlerts);
    hook.unmount();
    await act(async () => registration.resolve(stop));
    expect(stop).toHaveBeenCalledOnce();
  });
});
