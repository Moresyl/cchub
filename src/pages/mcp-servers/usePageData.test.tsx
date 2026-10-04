import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { usePageData } from "./usePageData";
import type { McpServer } from "./helpers";

const client = vi.hoisted(() => ({ getQueryData: vi.fn(), fetchQuery: vi.fn() }));
vi.mock("@tanstack/react-query", () => ({ useQueryClient: () => client }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../hooks/queries", () => ({ queryKeys: { mcpServersPage: ["mcp"] }, fetchMcpServersPageData: vi.fn() }));
beforeEach(() => {
  vi.mocked(invoke).mockImplementation(async (_command, args) =>
    Object.fromEntries(
      (args as { serverIds: string[] }).serverIds.map((id) => [id, { claude: { state: "source", disabled: false } }]),
    ),
  );
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
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
function server(id: string): McpServer {
  return {
    id,
    name: id,
    origin: { native_name: id, revision: "rev", bindings: [] },
    command: "node",
    args: "[]",
    env: "{}",
    transport: "stdio",
    status: "active",
    source: "local",
    config_path: null,
    package_name: null,
    version: null,
  };
}
const page = (id: string) => ({ servers: [server(id)], tools: [] });

it("ignores an older list response after a newer refresh and does not read its statuses", async () => {
  const old = deferred<ReturnType<typeof page>>();
  client.fetchQuery.mockReturnValueOnce(old.promise).mockResolvedValueOnce(page("new"));
  const hook = renderHook(() => usePageData(false));
  await act(async () => {
    await hook.result.current.loadPageData({ force: true });
  });
  await act(async () => {
    old.resolve(page("old"));
  });
  expect(hook.result.current.servers.map((item) => item.id)).toEqual(["new"]);
  expect(hook.result.current.serverAppStatus).toEqual({ new: { claude: true } });
  expect(invoke).toHaveBeenCalledTimes(1);
});

it("keeps the latest status request pending when an older status finishes", async () => {
  const old = deferred<unknown>();
  const fresh = deferred<unknown>();
  client.fetchQuery.mockResolvedValue(page("same"));
  vi.mocked(invoke).mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise);
  const hook = renderHook(() => usePageData(false));
  await act(async () => {});
  let refresh!: Promise<void>;
  await act(async () => {
    refresh = hook.result.current.loadPageData({ force: true });
  });
  await act(async () => {
    old.resolve({ same: { claude: { state: "source", disabled: false } } });
  });
  expect(hook.result.current.appStatusLoading).toBe(true);
  expect(hook.result.current.serverAppStatus).toEqual({});
  await act(async () => {
    fresh.resolve({ same: { claude: { state: "missing", disabled: false } } });
    await refresh;
  });
  expect(hook.result.current.serverAppStatus).toEqual({ same: { claude: false } });
  expect(hook.result.current.appStatusLoading).toBe(false);
});

it("keeps cached content on refresh failure but removes stale known states and private errors", async () => {
  client.getQueryData.mockReturnValue(page("cached"));
  client.fetchQuery.mockResolvedValueOnce(page("cached"));
  const hook = renderHook(() => usePageData(true));
  await act(async () => {});
  expect(hook.result.current.serverAppStatus).toEqual({ cached: { claude: true } });
  client.fetchQuery.mockRejectedValueOnce(new Error("private path and credentials"));
  await act(async () => {
    await hook.result.current.loadPageData({ force: true });
  });
  expect(hook.result.current.servers).toEqual(page("cached").servers);
  expect(hook.result.current.serverAppStatus).toEqual({});
  expect(hook.result.current.loadError).toBe("无法刷新服务列表，请重试。");
  expect(hook.result.current.loading).toBe(false);
});

it.each([false, true])(
  "treats invalid batch statuses as unknown and clears removed selection (rejected=%s)",
  async (rejected) => {
    client.fetchQuery
      .mockResolvedValueOnce(page("removed"))
      .mockResolvedValueOnce({ servers: [server("valid"), server("bad"), server("failed")], tools: [] });
    const hook = renderHook(() => usePageData(false));
    await act(async () => {});
    act(() => {
      hook.result.current.setSelected(server("removed"));
    });
    if (rejected) vi.mocked(invoke).mockRejectedValueOnce(new Error("private"));
    else
      vi.mocked(invoke).mockResolvedValueOnce({
        valid: { claude: { state: "missing", disabled: false } },
        bad: { claude: "true" },
      });
    await act(async () => {
      await hook.result.current.loadPageData();
    });
    expect(hook.result.current.selected).toBeNull();
    expect(hook.result.current.serverAppStatus).toEqual({});
  },
);

it("does not launch status reads after the page unmounts", async () => {
  const request = deferred<ReturnType<typeof page>>();
  client.fetchQuery.mockReturnValueOnce(request.promise);
  const hook = renderHook(() => usePageData(false));
  hook.unmount();
  await act(async () => {
    request.resolve(page("late"));
  });
  expect(invoke).not.toHaveBeenCalled();
});
