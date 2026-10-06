import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import AccountQuota from "./AccountQuota";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const localeText = (zh: string) => zh;
afterEach(cleanup);
beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("account quota states", () => {
  it("retains a same-account result during refresh and after failure, then replaces it on retry", async () => {
    let reject!: (reason: Error) => void;
    vi.mocked(invoke)
      .mockResolvedValueOnce({ success: true, tiers: [{ name: "weekly", utilization: 42 }] })
      .mockReturnValueOnce(
        new Promise((_, fail) => {
          reject = fail;
        }),
      )
      .mockResolvedValueOnce({ success: true, tiers: [{ name: "weekly", utilization: 65 }] });
    const { rerender, container } = render(<AccountQuota accountId="one" localeText={localeText} refreshKey={0} />);
    await screen.findByText("42%");
    const updated = container.querySelector("time")?.dateTime;
    rerender(<AccountQuota accountId="one" localeText={localeText} refreshKey={1} />);
    expect(screen.getByText("正在更新…")).toBeTruthy();
    expect(screen.getByText("42%")).toBeTruthy();
    await act(async () => reject(new Error("offline")));
    expect(screen.getByText("更新失败，以下为上次成功读取的数据。")).toBeTruthy();
    expect(screen.getByText("42%")).toBeTruthy();
    expect(container.querySelector("time")?.dateTime).toBe(updated);
    fireEvent.click(screen.getByRole("button", { name: "重试配额查询" }));
    await screen.findByText("65%");
    expect(screen.queryByText("更新失败，以下为上次成功读取的数据。")).toBeNull();
    expect(screen.queryByText("42%")).toBeNull();
  });

  it("never serves the previous account's result after switching to a failing account", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ success: true, tiers: [{ name: "weekly", utilization: 42 }] })
      .mockRejectedValueOnce(new Error("offline"));
    const { rerender } = render(<AccountQuota accountId="one" localeText={localeText} />);
    await screen.findByText("42%");
    rerender(<AccountQuota accountId="two" localeText={localeText} />);
    expect(screen.queryByText("42%")).toBeNull();
    await screen.findByText("配额查询失败");
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("changing a failure callback does not start duplicate queries", async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true, tiers: [{ name: "weekly", utilization: 10 }] });
    const { rerender } = render(<AccountQuota accountId="one" localeText={localeText} onFailure={() => {}} />);
    await screen.findByText("10%");
    rerender(<AccountQuota accountId="one" localeText={localeText} onFailure={() => {}} />);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("shows valid reset dates and omits invalid date values", async () => {
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      tiers: [
        { name: "five_hour", utilization: 15, resetsAt: "2026-10-07T11:00:00Z" },
        { name: "weekly", utilization: 110, resetsAt: "invalid" },
      ],
    });
    const { container } = render(<AccountQuota accountId="one" localeText={localeText} />);
    await screen.findByText("15%");
    expect(container.querySelector('time[datetime="2026-10-07T11:00:00.000Z"]')).toBeTruthy();
    expect(screen.queryByText(/Invalid Date/)).toBeNull();
    expect(screen.getByRole("progressbar", { name: "7 天额度" }).getAttribute("aria-valuenow")).toBe("100");
  });

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
