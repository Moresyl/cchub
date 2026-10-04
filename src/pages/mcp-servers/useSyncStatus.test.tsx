import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useSyncStatus } from "./useSyncStatus";
import type { McpServer } from "./helpers";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../components/Toast", () => ({ showToast: vi.fn() }));
const one: McpServer = {
  id: "one",
  name: "One",
  command: "node",
  args: "[]",
  env: "{}",
  transport: "stdio",
  status: "active",
  source: "local",
  package_name: null,
  version: null,
  config_path: null,
};
const two = { ...one, id: "two", name: "Two" };
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
beforeEach(() => vi.mocked(invoke).mockReset());
afterEach(cleanup);

it.each([false, true])("ignores superseded status results and errors (failure=%s)", async (failure) => {
  const old = deferred<Record<string, boolean>>();
  vi.mocked(invoke).mockReturnValueOnce(old.promise).mockResolvedValueOnce({ claude: false });
  const hook = renderHook(({ selected }) => useSyncStatus(selected, false), { initialProps: { selected: one } });
  hook.rerender({ selected: two });
  await waitFor(() => expect(hook.result.current.status).toEqual({ claude: false }));
  await act(async () => {
    if (failure) old.reject(new Error("private"));
    else old.resolve({ claude: true });
  });
  expect(hook.result.current.status).toEqual({ claude: false });
  expect(hook.result.current.error).toBe(false);
});

it("refuses unknown state and allows explicit retry after a failed read", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("private"));
  const hook = renderHook(() => useSyncStatus(one, false));
  await waitFor(() => expect(hook.result.current.error).toBe(true));
  await act(async () => {
    expect(await hook.result.current.toggle("claude")).toBeNull();
  });
  expect(invoke).toHaveBeenCalledTimes(1);
  vi.mocked(invoke).mockResolvedValueOnce({ claude: true });
  await act(async () => {
    await hook.result.current.refresh();
  });
  expect(hook.result.current.status).toEqual({ claude: true });
  expect(hook.result.current.error).toBe(false);
});

it("locks repeated toggles and keeps a completed old operation out of the newly selected service", async () => {
  const write = deferred<void>();
  vi.mocked(invoke)
    .mockResolvedValueOnce({ claude: false })
    .mockReturnValueOnce(write.promise)
    .mockResolvedValueOnce({ claude: false });
  const hook = renderHook(({ selected }) => useSyncStatus(selected, false), { initialProps: { selected: one } });
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
  let pending!: ReturnType<typeof hook.result.current.toggle>;
  await act(async () => {
    pending = hook.result.current.toggle("claude");
    expect(await hook.result.current.toggle("claude")).toBeNull();
  });
  expect(invoke).toHaveBeenCalledTimes(2);
  hook.rerender({ selected: two });
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
  await act(async () => {
    write.resolve();
    expect(await pending).toEqual({ serverId: "one", toolId: "claude", enabled: true });
  });
  expect(hook.result.current.status).toEqual({ claude: false });
  expect(invoke).toHaveBeenCalledTimes(3);
});
