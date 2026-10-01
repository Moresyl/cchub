import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PromptApp } from "./types";
import { usePromptLibrary } from "./usePromptLibrary";
import { deferred, snapshot } from "./testFixtures";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockResolvedValue(snapshot());
});
afterEach(cleanup);

describe("prompt library ownership", () => {
  it("retains the previous snapshot after failed refresh and blocks writes until reloaded", async () => {
    const { result } = renderHook(() => usePromptLibrary("claude"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(invoke).mockRejectedValueOnce(new Error("read failed"));
    await act(async () => {
      await result.current.refresh();
    });
    expect(result.current.snapshot?.libraryRevision).toBe("library-1");
    expect(result.current.needsReload).toBe(true);
    const count = vi.mocked(invoke).mock.calls.length;
    await act(async () => {
      expect(await result.current.mutate("delete_prompt", { id: "one" })).toBe(false);
    });
    expect(invoke).toHaveBeenCalledTimes(count);
    await act(async () => {
      await result.current.refresh();
    });
    expect(result.current.needsReload).toBe(false);
  });

  it("rejects malformed snapshots without replacing readable state", async () => {
    const { result } = renderHook(() => usePromptLibrary("claude"));
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());
    vi.mocked(invoke).mockResolvedValueOnce({ prompts: {} });
    await act(async () => {
      await result.current.refresh();
    });
    expect(result.current.snapshot?.libraryRevision).toBe("library-1");
    expect(result.current.error).toContain("reload");
  });

  it("ignores a late old-app load even after changing away and back", async () => {
    const held = deferred<ReturnType<typeof snapshot>>();
    vi.mocked(invoke).mockReturnValueOnce(held.promise);
    const { result, rerender } = renderHook(({ app }: { app: PromptApp }) => usePromptLibrary(app), {
      initialProps: { app: "claude" as PromptApp },
    });
    rerender({ app: "codex" });
    await waitFor(() => expect(result.current.loading).toBe(false));
    rerender({ app: "claude" });
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(async () => {
      held.resolve(snapshot({ libraryRevision: "stale" }));
      await held.promise;
    });
    expect(result.current.snapshot?.libraryRevision).toBe("library-1");
  });

  it("serializes same-tick mutations and uses the owned revisions and app", async () => {
    const { result } = renderHook(() => usePromptLibrary("claude"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const held = deferred<null>();
    vi.mocked(invoke).mockReturnValueOnce(held.promise);
    let first!: Promise<boolean>;
    await act(async () => {
      first = result.current.mutate("enable_prompt", { id: "one", app: "codex" });
      expect(await result.current.mutate("enable_prompt", { id: "one" })).toBe(false);
    });
    expect(invoke).toHaveBeenLastCalledWith(
      "enable_prompt",
      expect.objectContaining({
        app: "claude",
        expectedLibraryRevision: "library-1",
        expectedLiveRevision: "file-1",
      }),
    );
    await act(async () => {
      held.resolve(null);
      expect(await first).toBe(true);
    });
    expect(result.current.writing).toBe(false);
  });

  it("does not retry a committed write whose readback failed", async () => {
    const { result } = renderHook(() => usePromptLibrary("claude"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(invoke).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error("readback failed"));
    await act(async () => {
      expect(await result.current.mutate("upsert_prompt")).toBe(false);
    });
    expect(result.current.needsReload).toBe(true);
    expect(result.current.error).toContain("saved");
    const count = vi.mocked(invoke).mock.calls.length;
    await act(async () => {
      await result.current.mutate("upsert_prompt");
    });
    expect(invoke).toHaveBeenCalledTimes(count);
  });

  it("blocks stale callbacks synchronously after a rejected write", async () => {
    const { result } = renderHook(() => usePromptLibrary("claude"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const mutate = result.current.mutate;
    vi.mocked(invoke).mockRejectedValueOnce(new Error("write rejected"));
    await act(async () => {
      await mutate("upsert_prompt");
      await mutate("upsert_prompt");
    });
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "upsert_prompt")).toHaveLength(1);
  });

  it("ignores a late mutation after app switch and releases its global busy state", async () => {
    const { result, rerender } = renderHook(({ app }: { app: PromptApp }) => usePromptLibrary(app), {
      initialProps: { app: "claude" as PromptApp },
    });
    await waitFor(() => expect(result.current.loading).toBe(false));
    const held = deferred<null>();
    vi.mocked(invoke).mockReturnValueOnce(held.promise);
    let pending!: Promise<boolean>;
    await act(async () => {
      pending = result.current.mutate("enable_prompt");
    });
    rerender({ app: "codex" });
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());
    await act(async () => {
      held.resolve(null);
      expect(await pending).toBe(false);
    });
    expect(result.current.writing).toBe(false);
    expect(result.current.snapshot?.libraryRevision).toBe("library-1");
  });

  it("prevents an earlier pending refresh from resurrecting a deleted record", async () => {
    const { result } = renderHook(() => usePromptLibrary("claude"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const held = deferred<ReturnType<typeof snapshot>>();
    vi.mocked(invoke).mockReturnValueOnce(held.promise);
    let refresh!: Promise<unknown>;
    await act(async () => {
      refresh = result.current.refresh();
    });
    const count = vi.mocked(invoke).mock.calls.length;
    await act(async () => {
      expect(await result.current.mutate("delete_prompt")).toBe(false);
    });
    expect(invoke).toHaveBeenCalledTimes(count);
    await act(async () => {
      held.resolve(snapshot());
      await refresh;
    });
    vi.mocked(invoke)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(snapshot({ prompts: {}, libraryRevision: "new" }));
    await act(async () => {
      expect(await result.current.mutate("delete_prompt", { id: "one" })).toBe(true);
    });
    expect(result.current.snapshot?.prompts).toEqual({});
  });
});
