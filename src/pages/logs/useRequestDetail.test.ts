import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useRequestDetail } from "./useRequestDetail";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const text = (zh: string) => zh;
function deferred() {
  let resolve!: (value: unknown) => void;
  const promise = new Promise<unknown>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
beforeEach(() => invoke.mockReset());

describe("request detail ownership", () => {
  it("ignores an older response and a response arriving after close", async () => {
    const pending = deferred();
    invoke.mockReturnValueOnce(pending.promise).mockResolvedValueOnce({ request_id: "new" });
    const { result } = renderHook(() => useRequestDetail(text));
    act(() => {
      void result.current.load("old");
    });
    await act(async () => {
      await result.current.load("new");
    });
    expect(result.current.record?.request_id).toBe("new");
    await act(async () => pending.resolve({ request_id: "old" }));
    expect(result.current.record?.request_id).toBe("new");
    const late = deferred();
    invoke.mockReturnValueOnce(late.promise);
    act(() => {
      void result.current.load("late");
      result.current.close();
    });
    await act(async () => late.resolve({ request_id: "late" }));
    expect(result.current.record).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it("shows an actionable masked failure, retries the same request and reports a removed row", async () => {
    invoke
      .mockRejectedValueOnce(new Error("private secret"))
      .mockResolvedValueOnce({ request_id: "one" })
      .mockResolvedValueOnce(null);
    const { result } = renderHook(() => useRequestDetail(text));
    await act(async () => result.current.load("one"));
    expect(result.current.error).toContain("加载失败");
    expect(result.current.error).not.toContain("private secret");
    act(() => result.current.retry());
    await waitFor(() => expect(result.current.record?.request_id).toBe("one"));
    expect(invoke).toHaveBeenLastCalledWith("get_request_detail", { requestId: "one" });
    await act(async () => result.current.load("removed"));
    expect(result.current.record).toBeNull();
    expect(result.current.error).toContain("已被清理");
  });
});
