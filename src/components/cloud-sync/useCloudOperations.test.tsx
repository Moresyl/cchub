import { renderHook } from "@testing-library/react";
import { StrictMode } from "react";
import { describe, expect, it, vi } from "vitest";
import { useCloudCallback, useCloudOperations } from "./useCloudOperations";

describe("cloud operation ownership", () => {
  it("keeps subscribed callbacks stable, uses the latest committed handler and silences late events after unmount", () => {
    const first = vi.fn();
    const next = vi.fn();
    const { result, rerender, unmount } = renderHook(({ handler }) => useCloudCallback(handler), {
      initialProps: { handler: first },
      wrapper: StrictMode,
    });
    const subscribed = result.current;
    subscribed("first");
    expect(first).toHaveBeenCalledWith("first");
    rerender({ handler: next });
    expect(result.current).toBe(subscribed);
    subscribed("latest");
    expect(next).toHaveBeenCalledWith("latest");
    unmount();
    subscribed("late");
    expect(next).toHaveBeenCalledOnce();
  });
  it("coalesces reads and lets a write supersede a background read synchronously", () => {
    const { result } = renderHook(useCloudOperations);
    const read = result.current.beginRead()!;
    expect(read.current()).toBe(true);
    expect(result.current.beginRead()).toBeNull();
    const write = result.current.beginAction()!;
    expect(read.current()).toBe(false);
    expect(write.current()).toBe(true);
    expect(result.current.beginAction()).toBeNull();
    expect(result.current.beginRead()).toBeNull();
    read.finish();
    expect(write.current()).toBe(true);
    write.finish();
    expect(write.current()).toBe(false);
    expect(result.current.beginRead()?.current()).toBe(true);
  });

  it("editing invalidates reads without releasing an action's ownership", () => {
    const { result } = renderHook(useCloudOperations);
    const read = result.current.beginRead()!;
    result.current.invalidateReads();
    expect(read.current()).toBe(false);
    const action = result.current.beginAction()!;
    result.current.invalidateReads();
    expect(action.current()).toBe(true);
    action.finish();
    expect(result.current.beginAction()?.current()).toBe(true);
  });

  it("keeps a stable guard across rerenders and revokes leases on unmount in StrictMode", () => {
    const { result, rerender, unmount } = renderHook(useCloudOperations, { wrapper: StrictMode });
    const guard = result.current;
    const action = guard.beginAction()!;
    rerender();
    expect(result.current).toBe(guard);
    expect(action.current()).toBe(true);
    unmount();
    expect(action.current()).toBe(false);
    expect(guard.beginRead()).toBeNull();
    expect(guard.beginAction()).toBeNull();
    action.finish();
  });
});
