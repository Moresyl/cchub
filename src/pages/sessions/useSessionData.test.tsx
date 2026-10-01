import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchSessionsPageData, queryKeys } from "../../hooks/queries";
import type { ManagedAppId } from "../../lib/appPreferences";
import type { SessionDetail, SessionSummary } from "./helpers";
import { deferred, detailFixture, sessionFixture } from "./testFixtures";
import { useSessionData } from "./useSessionData";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../hooks/queries", () => ({
  fetchSessionsPageData: vi.fn(),
  queryKeys: { sessions: (tool: string | null) => ["sessions", tool ?? "all"] },
}));

const clients: QueryClient[] = [];
const first = sessionFixture();
const other = sessionFixture({ title: "Other source", source_path: "C:/fixture/two.jsonl" });

function mount(filter: ManagedAppId | "all" = "all", cached?: SessionSummary[]) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } });
  clients.push(client);
  if (cached) client.setQueryData(queryKeys.sessions(filter === "all" ? null : filter), cached);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, ...renderHook(({ tool }) => useSessionData(tool), { wrapper, initialProps: { tool: filter } }) };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(fetchSessionsPageData).mockResolvedValue([first, other]);
  vi.mocked(invoke).mockResolvedValue(detailFixture(first));
});
afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
});

describe("session data ownership", () => {
  it("loads details from the exact selected source and reconciles a rename", async () => {
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(invoke).mockResolvedValue(detailFixture(other));
    await act(async () => {
      await result.current.openSession(other);
    });
    expect(invoke).toHaveBeenCalledWith(
      "get_session_detail",
      expect.objectContaining({ sourcePath: other.source_path, sourceBackend: "file" }),
    );
    const renamed = { ...other, title: "New name", tokens_used: 30 };
    vi.mocked(fetchSessionsPageData).mockResolvedValue([first, renamed]);
    await act(async () => {
      await result.current.loadSessions(false);
    });
    expect(result.current.selectedSession).toEqual(renamed);
    expect(result.current.detail?.session).toEqual(renamed);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("does not restart pending details or let their old name overwrite a refresh", async () => {
    const pending = deferred<SessionDetail>();
    vi.mocked(invoke).mockReturnValue(pending.promise);
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => {
      void result.current.openSession(first);
    });
    const renamed = { ...first, title: "Renamed during load" };
    vi.mocked(fetchSessionsPageData).mockResolvedValue([renamed]);
    await act(async () => {
      await result.current.loadSessions(false);
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    await act(async () => {
      pending.resolve(detailFixture(first));
      await pending.promise;
    });
    expect(result.current.detail?.session.title).toBe(renamed.title);
    expect(result.current.detailLoading).toBe(false);
  });

  it("checks current pending ownership when a list request started before details", async () => {
    const list = deferred<SessionSummary[]>();
    const details = deferred<SessionDetail>();
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(fetchSessionsPageData).mockReturnValue(list.promise);
    act(() => {
      void result.current.loadSessions(false);
    });
    vi.mocked(invoke).mockReturnValue(details.promise);
    act(() => {
      void result.current.openSession(first);
    });
    await act(async () => {
      list.resolve([{ ...first, title: "Latest name" }]);
      await list.promise;
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    await act(async () => {
      details.resolve(detailFixture(first));
      await details.promise;
    });
    expect(result.current.detail?.session.title).toBe("Latest name");
  });

  it("ignores an old detail response when another same-ID source is selected", async () => {
    const pending = deferred<SessionDetail>();
    vi.mocked(invoke).mockReturnValueOnce(pending.promise).mockResolvedValueOnce(detailFixture(other));
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => {
      void result.current.openSession(first);
    });
    await act(async () => {
      await result.current.openSession(other);
    });
    await act(async () => {
      pending.resolve(detailFixture(first));
      await pending.promise;
    });
    expect(result.current.detail?.session.source_path).toBe(other.source_path);
    expect(result.current.detailLoading).toBe(false);
  });

  it.each(["resolve", "reject"] as const)("keeps a closed detail closed after a late %s", async (outcome) => {
    const pending = deferred<SessionDetail>();
    vi.mocked(invoke).mockReturnValue(pending.promise);
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => {
      void result.current.openSession(first);
      result.current.closeSession();
    });
    await act(async () => {
      if (outcome === "resolve") pending.resolve(detailFixture(first));
      else pending.reject(new Error("old failure"));
      await pending.promise.catch(() => {});
    });
    expect(result.current.selectedSession).toBeNull();
    expect(result.current.detail).toBeNull();
    expect(result.current.detailError).toBeNull();
    expect(result.current.detailLoading).toBe(false);
  });

  it("retains list and detail after a failed refresh, then retries", async () => {
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(async () => {
      await result.current.openSession(first);
    });
    vi.mocked(fetchSessionsPageData).mockRejectedValueOnce(new Error("read failed"));
    await act(async () => {
      await result.current.loadSessions(false);
    });
    expect(result.current.loadError).toContain("read failed");
    expect(result.current.allSessions).toEqual([first, other]);
    expect(result.current.detail?.session).toEqual(first);
    await act(async () => {
      await result.current.loadSessions(false);
    });
    expect(result.current.loadError).toBeNull();
  });

  it("allows retry after initial list and detail errors", async () => {
    vi.mocked(fetchSessionsPageData).mockRejectedValueOnce(new Error("initial failure"));
    const { result } = mount();
    await waitFor(() => expect(result.current.loadError).toContain("initial failure"));
    expect(result.current.loading).toBe(false);
    await act(async () => {
      await result.current.loadSessions(true);
    });
    vi.mocked(invoke).mockRejectedValueOnce(new Error("detail failure"));
    await act(async () => {
      await result.current.openSession(first);
    });
    expect(result.current.detailError).toContain("detail failure");
    await act(async () => {
      await result.current.openSession(first, false);
    });
    expect(result.current.detailError).toBeNull();
    expect(result.current.detail?.session).toEqual(first);
  });

  it("rejects details that belong to a different backend or path", async () => {
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(invoke).mockResolvedValue(detailFixture(other));
    await act(async () => {
      await result.current.openSession(first);
    });
    expect(result.current.detail).toBeNull();
    expect(result.current.detailError).toContain("did not match its source");
  });

  it("closes a vanished source rather than choosing another record with the same ID", async () => {
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(async () => {
      await result.current.openSession(first);
    });
    vi.mocked(fetchSessionsPageData).mockResolvedValue([other]);
    await act(async () => {
      await result.current.loadSessions(false);
    });
    expect(result.current.selectedSession).toBeNull();
    expect(result.current.detail).toBeNull();
  });

  it("removes confirmed deletions without closing a newly selected source", async () => {
    const { result, client } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(invoke).mockResolvedValue(detailFixture(other));
    await act(async () => {
      await result.current.openSession(other);
    });
    act(() => result.current.removeSessions([first]));
    expect(result.current.allSessions).toEqual([other]);
    expect(client.getQueryData(queryKeys.sessions(null))).toEqual([other]);
    expect(result.current.detail?.session).toEqual(other);
    act(() => result.current.removeSessions([other]));
    expect(result.current.selectedSession).toBeNull();
  });

  it("cancels a pre-deletion scan so a late list cannot resurrect the deleted source", async () => {
    const pending = deferred<SessionSummary[]>();
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(fetchSessionsPageData).mockReturnValue(pending.promise);
    act(() => {
      void result.current.loadSessions(false);
    });
    act(() => result.current.removeSessions([first]));
    await act(async () => {
      pending.resolve([first, other]);
      await pending.promise;
    });
    expect(result.current.allSessions).toEqual([other]);
    expect(result.current.refreshing).toBe(false);
  });

  it("ignores an old tool's list and details after changing the filter", async () => {
    const pendingList = deferred<SessionSummary[]>();
    const pendingDetail = deferred<SessionDetail>();
    const claude = sessionFixture({ tool_id: "claude", tool_name: "Claude" });
    const { result, rerender } = mount("codex", [first]);
    vi.mocked(fetchSessionsPageData).mockImplementation((tool) =>
      tool === "claude" ? Promise.resolve([claude]) : pendingList.promise,
    );
    vi.mocked(invoke).mockReturnValue(pendingDetail.promise);
    act(() => {
      void result.current.openSession(first);
      void result.current.loadSessions(false);
    });
    await waitFor(() => expect(fetchSessionsPageData).toHaveBeenCalledWith("codex"));
    rerender({ tool: "claude" });
    await waitFor(() => expect(result.current.allSessions).toEqual([claude]));
    await act(async () => {
      pendingList.resolve([first]);
      pendingDetail.reject(new Error("old tool failed"));
      await Promise.all([pendingList.promise, pendingDetail.promise.catch(() => {})]);
    });
    expect(result.current.allSessions).toEqual([claude]);
    expect(result.current.detailError).toBeNull();
    expect(result.current.selectedSession).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it("does not retry a previously selected source after a new selection", async () => {
    const { result } = mount();
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(invoke).mockResolvedValue(detailFixture(other));
    await act(async () => {
      await result.current.openSession(other);
      await result.current.openSession(first, false);
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(result.current.selectedSession).toEqual(other);
  });
});
