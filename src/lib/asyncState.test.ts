import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useAsyncResource } from "./asyncState";

describe("useAsyncResource", () => {
  it("loads data and exposes reload", async () => {
    const loader = vi.fn().mockResolvedValueOnce("first").mockResolvedValueOnce("second");
    const { result } = renderHook(() => useAsyncResource(loader));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.data).toBe("first");
    expect(result.current.error).toBeNull();

    act(() => {
      result.current.reload();
    });

    await waitFor(() => expect(result.current.data).toBe("second"));
    expect(loader).toHaveBeenCalledTimes(2);
  });

  it("keeps existing data when reload fails", async () => {
    const loader = vi.fn().mockResolvedValueOnce("cached").mockRejectedValueOnce(new Error("failed"));
    const { result } = renderHook(() => useAsyncResource(loader));

    await waitFor(() => expect(result.current.data).toBe("cached"));

    act(() => {
      result.current.reload();
    });

    await waitFor(() => expect(result.current.error).toBe("failed"));
    expect(result.current.data).toBe("cached");
    expect(result.current.loading).toBe(false);
  });
});
