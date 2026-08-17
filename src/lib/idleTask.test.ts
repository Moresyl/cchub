import { afterEach, describe, expect, it, vi } from "vitest";
import { scheduleIdleTask } from "./idleTask";

describe("scheduleIdleTask", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("runs delayed work when requestIdleCallback is unavailable", () => {
    vi.useFakeTimers();
    const task = vi.fn();

    scheduleIdleTask(task, { delay: 250 });
    vi.advanceTimersByTime(249);
    expect(task).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(task).toHaveBeenCalledOnce();
  });

  it("cancels delayed work", () => {
    vi.useFakeTimers();
    const task = vi.fn();
    const cancel = scheduleIdleTask(task, { delay: 250 });

    cancel();
    vi.runAllTimers();
    expect(task).not.toHaveBeenCalled();
  });

  it("cancels a queued idle callback", () => {
    vi.useFakeTimers();
    const task = vi.fn();
    const idleCallback = vi.fn(() => 42);
    const cancelIdleCallback = vi.fn();
    vi.stubGlobal("requestIdleCallback", idleCallback);
    vi.stubGlobal("cancelIdleCallback", cancelIdleCallback);

    const cancel = scheduleIdleTask(task, { delay: 0, timeout: 900 });
    vi.runAllTimers();
    expect(idleCallback).toHaveBeenCalledOnce();
    cancel();
    expect(cancelIdleCallback).toHaveBeenCalledWith(42);
    expect(task).not.toHaveBeenCalled();
  });
});
