import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import AccountQuota from "./AccountQuota";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const localeText = (zh: string) => zh;
beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("account quota states", () => {
  it("finishes failed queries and allows retry instead of loading forever", async () => {
    vi.mocked(invoke)
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce({ success: true, tiers: [{ name: "five_hour", utilization: 0 }] });
    const onFailure = vi.fn();
    render(<AccountQuota accountId="one" localeText={localeText} onFailure={onFailure} />);
    expect(await screen.findByText("配额查询失败")).toBeTruthy();
    expect(screen.queryByText("配额读取中…")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "重试配额查询" }));
    expect((await screen.findByRole("progressbar", { name: "5 小时额度" })).getAttribute("aria-valuenow")).toBe("0");
    expect(onFailure).toHaveBeenCalledTimes(1);
  });

  it("ignores old account failures after switching", async () => {
    let reject!: (reason: Error) => void;
    vi.mocked(invoke)
      .mockReturnValueOnce(
        new Promise((_, fail) => {
          reject = fail;
        }),
      )
      .mockResolvedValueOnce({ success: true, tiers: [{ name: "weekly", utilization: 42 }] });
    const onFailure = vi.fn();
    const { rerender } = render(<AccountQuota accountId="one" localeText={localeText} onFailure={onFailure} />);
    rerender(<AccountQuota accountId="two" localeText={localeText} onFailure={onFailure} />);
    expect(await screen.findByText("42%")).toBeTruthy();
    await act(async () => reject(new Error("old failure")));
    expect(screen.getByText("42%")).toBeTruthy();
    expect(onFailure).not.toHaveBeenCalled();
  });

  it("does not render non-finite or missing usage as a zero meter", async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true, tiers: [{ name: "unknown", utilization: NaN }] });
    render(<AccountQuota accountId="one" localeText={localeText} />);
    expect(await screen.findByText("供应商未返回配额数据")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("failed responses and invalid shapes remain retryable", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ success: false, tiers: [] }).mockResolvedValueOnce({ success: true });
    render(<AccountQuota accountId="one" localeText={localeText} />);
    await screen.findByText("配额查询失败");
    fireEvent.click(screen.getByRole("button", { name: "重试配额查询" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("配额查询失败")).toBeTruthy();
  });
});
