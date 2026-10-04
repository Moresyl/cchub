import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { showToast } from "../../components/Toast";
import { useConfigSave } from "./useConfigSave";

vi.mock("../../components/Toast", () => ({ showToast: vi.fn() }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const draft = { name: "fixture", command: "node", args: '["server.js"]', env: '{"TOKEN":"private"}' };

it("blocks repeated saves before a render and releases the lock after failure for retry", async () => {
  const hook = renderHook(() => useConfigSave(false));
  let reject!: (reason: Error) => void;
  const write = vi.fn(
    () =>
      new Promise<void>((_, no) => {
        reject = no;
      }),
  );
  let first!: Promise<boolean>;
  await act(async () => {
    first = hook.result.current.save(draft, write);
    expect(hook.result.current.isSaving()).toBe(true);
    expect(await hook.result.current.save(draft, write)).toBe(false);
  });
  expect(write).toHaveBeenCalledTimes(1);
  expect(hook.result.current.saving).toBe(true);
  await act(async () => {
    reject(new Error("private credentials and native path"));
    expect(await first).toBe(false);
  });
  expect(hook.result.current.saving).toBe(false);
  expect(showToast).toHaveBeenCalledWith(
    "error",
    "Could not save configuration. Your draft is preserved. Check the file and retry.",
  );
  await act(async () => {
    expect(await hook.result.current.save(draft, vi.fn().mockResolvedValue(undefined))).toBe(true);
  });
});

it.each([
  { args: "{}" },
  { args: "[1]" },
  { args: '"private"' },
  { env: "null" },
  { env: "[]" },
  { env: '{"TOKEN":12}' },
  { env: '{"":"value"}' },
  { command: " " },
  { command: "node\0" },
  { args: '["a\\u0000"]' },
  { env: '{"TOKEN":"a\\u0000"}' },
])("rejects invalid field shapes without invoking native writes: %j", async (invalid) => {
  const hook = renderHook(() => useConfigSave(false));
  const write = vi.fn();
  await act(async () => {
    expect(await hook.result.current.save({ ...draft, ...invalid }, write)).toBe(false);
  });
  expect(write).not.toHaveBeenCalled();
  expect(hook.result.current.isSaving()).toBe(false);
  expect(showToast).toHaveBeenCalledTimes(1);
});
