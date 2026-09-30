import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { refreshBackupRestoreState } from "./backupRestoreState";

function client() {
  return new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnMount: false } } });
}

describe("backup restore cache refresh", () => {
  it("cancels a stale read, removes inactive cached libraries and refreshes active views", async () => {
    const cache = client();
    cache.setQueryData(["inactive library"], "old data");
    let finishOld!: (value: string) => void;
    const oldRead = cache.fetchQuery({
      queryKey: ["old read"],
      queryFn: () =>
        new Promise<string>((resolve) => {
          finishOld = resolve;
        }),
    });
    const oldResult = oldRead.catch(() => "cancelled");
    cache.setQueryData(["active library"], "old active data");
    const observer = new QueryObserver(cache, {
      queryKey: ["active library"],
      queryFn: async () => "restored data",
      refetchOnMount: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    const refreshLocal = vi.fn(async () => {});
    expect(await refreshBackupRestoreState(refreshLocal, cache)).toBe(true);
    finishOld("obsolete data");
    expect(await oldResult).toBe("cancelled");
    expect(cache.getQueryData(["old read"])).toBeUndefined();
    expect(cache.getQueryData(["inactive library"])).toBeUndefined();
    expect(observer.getCurrentResult().data).toBe("restored data");
    expect(refreshLocal).toHaveBeenCalledOnce();
    unsubscribe();
    cache.clear();
  });

  it("still refreshes active queries when migration state cannot be read", async () => {
    const cache = client();
    cache.setQueryData(["profiles"], "old profiles");
    const observer = new QueryObserver(cache, { queryKey: ["profiles"], queryFn: async () => "restored profiles" });
    const unsubscribe = observer.subscribe(() => {});
    expect(
      await refreshBackupRestoreState(async () => {
        throw new Error("IPC unavailable");
      }, cache),
    ).toBe(false);
    expect(observer.getCurrentResult().data).toBe("restored profiles");
    unsubscribe();
    cache.clear();
  });

  it("reports refetch failures without rejecting after data was restored", async () => {
    const cache = client();
    cache.setQueryData(["profiles"], "old profiles");
    const observer = new QueryObserver(cache, {
      queryKey: ["profiles"],
      queryFn: async () => {
        throw new Error("IPC unavailable");
      },
    });
    const unsubscribe = observer.subscribe(() => {});
    const refreshLocal = vi.fn(async () => {});
    expect(await refreshBackupRestoreState(refreshLocal, cache)).toBe(false);
    expect(observer.getCurrentResult().isError).toBe(true);
    expect(observer.getCurrentResult().data).toBeUndefined();
    expect(refreshLocal).toHaveBeenCalledOnce();
    unsubscribe();
    cache.clear();
  });
});
