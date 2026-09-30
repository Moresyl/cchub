import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import UsageDetailsDialog from "./UsageDetailsDialog";
import type { ConfigProfile } from "../pages/profiles/helpers";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./Toast", () => ({ showToast: vi.fn() }));
vi.mock("./CodeEditor", () => ({
  default: ({ value, language, readOnly }: { value: string; language: string; readOnly: boolean }) => (
    <div data-testid="json-editor" data-language={language} data-readonly={readOnly}>
      {value}
    </div>
  ),
}));

const profile: ConfigProfile = {
  id: "one",
  name: "My provider",
  tool_id: "claude",
  config_snapshot: "{}",
  sort_order: 0,
  created_at: null,
  updated_at: null,
};
beforeEach(() => vi.mocked(invoke).mockReset());

describe("usage and balance dialog", () => {
  it("distinguishes token and credit quotas in the same time window", async () => {
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      data: [
        { planName: "five_hour", metric: "tokens_limit", utilization: 10 },
        { planName: "five_hour", metric: "credit_limit", utilization: 20 },
      ],
    });
    render(<UsageDetailsDialog profile={profile} locale="zh" onClose={vi.fn()} />);
    expect(await screen.findByRole("progressbar", { name: "5 小时额度 · Token" })).toBeTruthy();
    expect(screen.getByRole("progressbar", { name: "5 小时额度 · 积分" })).toBeTruthy();
  });
  it("shows all currencies and a highlighted read-only JSON view", async () => {
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      data: [
        { planName: "USD wallet", remaining: 12.5, unit: "USD" },
        { planName: "CNY wallet", remaining: 25, unit: "CNY" },
      ],
    });
    render(<UsageDetailsDialog profile={profile} locale="zh" onClose={vi.fn()} />);
    expect(await screen.findByText("USD wallet")).toBeTruthy();
    expect(screen.getByText("CNY wallet")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
    fireEvent.click(screen.getByText("查看标准化 JSON"));
    const editor = await screen.findByTestId("json-editor");
    expect(editor.dataset.language).toBe("json");
    expect(editor.dataset.readonly).toBe("true");
  });

  it("marks stale successful data after a failed refresh and permits retry", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ success: true, data: [{ planName: "five_hour", utilization: 75 }] })
      .mockRejectedValueOnce(new Error("Network unavailable"));
    render(<UsageDetailsDialog profile={profile} locale="zh" onClose={vi.fn()} />);
    expect(await screen.findByRole("progressbar", { name: "5 小时额度" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    expect(await screen.findByText("刷新失败，下方为上次成功查询的数据")).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("Network unavailable");
    expect(screen.queryByText("查询成功")).toBeNull();
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("75");
  });

  it("does not show an exhausted or healthy percentage for missing data", async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true, data: [{ planName: "Incomplete", total: 100 }] });
    render(<UsageDetailsDialog profile={profile} locale="zh" onClose={vi.fn()} />);
    expect(await screen.findByText("数据不完整，无法计算剩余或使用比例")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("never leaves previous provider cards visible while the next request loads", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ success: true, data: [{ planName: "Old account", remaining: 5 }] });
    let resolve!: (value: unknown) => void;
    vi.mocked(invoke).mockReturnValueOnce(
      new Promise((yes) => {
        resolve = yes;
      }),
    );
    const view = render(<UsageDetailsDialog profile={profile} locale="zh" onClose={vi.fn()} />);
    await screen.findByText("Old account");
    view.rerender(<UsageDetailsDialog profile={{ ...profile, id: "two" }} locale="zh" onClose={vi.fn()} />);
    expect(screen.queryByText("Old account")).toBeNull();
    await act(async () => resolve({ success: true, data: [{ planName: "New account", remaining: 10 }] }));
    await waitFor(() => expect(screen.getByText("New account")).toBeTruthy());
  });
});
