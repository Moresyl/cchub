import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { showToast } from "../../components/Toast";
import { invoke } from "@tauri-apps/api/core";
import { useConfigCopy } from "./useConfigCopy";
import type { McpServer } from "./helpers";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../components/Toast", () => ({ showToast: vi.fn() }));
const server: McpServer = {
  id: "one",
  name: "One",
  command: "node",
  args: '["server.js"]',
  env: '{"TOKEN":"private"}',
  transport: "stdio",
  status: "active",
  source: "local",
  package_name: null,
  version: null,
  config_path: null,
};
const write = vi.fn();
beforeEach(() => {
  vi.mocked(invoke)
    .mockReset()
    .mockResolvedValue(
      JSON.stringify({ command: "node", args: ["server.js"], env: { TOKEN: "private" }, timeout: 45, disabled: true }),
    );
  write.mockReset().mockResolvedValue(undefined);
  vi.stubGlobal("navigator", { clipboard: { writeText: write } });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

it("copies the full native configuration by origin ID, including extension fields", async () => {
  const hook = renderHook(() => useConfigCopy(server, false));
  await act(async () => {
    await hook.result.current.copy();
  });
  expect(invoke).toHaveBeenCalledWith("export_mcp_server_config", { serverId: "one" });
  expect(JSON.parse(write.mock.calls[0][0])).toEqual({
    command: "node",
    args: ["server.js"],
    env: { TOKEN: "private" },
    timeout: 45,
    disabled: true,
  });
  expect(hook.result.current.copied).toBe(true);
});

it("waits for clipboard success, blocks duplicate requests and ignores completion after selection changes", async () => {
  let resolve!: () => void;
  write.mockImplementationOnce(
    () =>
      new Promise<void>((yes) => {
        resolve = yes;
      }),
  );
  const hook = renderHook(({ selected }) => useConfigCopy(selected, false), { initialProps: { selected: server } });
  let pending!: Promise<void>;
  await act(async () => {
    pending = hook.result.current.copy();
    await hook.result.current.copy();
  });
  expect(write).toHaveBeenCalledTimes(1);
  expect(hook.result.current.copied).toBe(false);
  expect(hook.result.current.copying).toBe(true);
  hook.rerender({ selected: { ...server, id: "two" } });
  await act(async () => {
    resolve();
    await pending;
  });
  expect(hook.result.current.copied).toBe(false);
  expect(hook.result.current.copying).toBe(false);
});

it("reports clipboard failure without exposing native errors and permits retry", async () => {
  write.mockRejectedValueOnce(new Error("private clipboard details"));
  const hook = renderHook(() => useConfigCopy(server, false));
  await act(async () => {
    await hook.result.current.copy();
  });
  expect(hook.result.current.copied).toBe(false);
  expect(showToast).toHaveBeenCalledWith(
    "error",
    "Could not copy configuration. Check the configuration and clipboard permissions, then retry.",
  );
  await act(async () => {
    await hook.result.current.copy();
  });
  expect(hook.result.current.copied).toBe(true);
});

it.each(["null", "[]", "{", '"text"'])("refuses malformed native exports: %s", async (invalid) => {
  vi.mocked(invoke).mockResolvedValueOnce(invalid);
  const hook = renderHook(() => useConfigCopy(server, false));
  await act(async () => {
    await hook.result.current.copy();
  });
  expect(write).not.toHaveBeenCalled();
  expect(hook.result.current.copied).toBe(false);
});

it("does not copy stale connection fields when native export fails", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("private path"));
  const hook = renderHook(() => useConfigCopy(server, false));
  await act(async () => {
    await hook.result.current.copy();
  });
  expect(write).not.toHaveBeenCalled();
  expect(showToast).toHaveBeenCalled();
});
