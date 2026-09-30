import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SessionUsageActions from "./SessionUsageActions";
import { compatApi, type SessionSyncResult } from "../lib/api/compat";
import { showToast } from "./Toast";

vi.mock("../lib/api/compat", () => ({ compatApi: { syncSessionUsage: vi.fn(), rebuildCodexUsage: vi.fn() } }));
vi.mock("../lib/i18n", () => ({ getLocale: () => "zh" }));
vi.mock("./Toast", () => ({ showToast: vi.fn() }));
vi.mock("./AppDialogProvider", () => ({ useAppDialog: () => ({ confirm: vi.fn().mockResolvedValue(false) }) }));

function result(overrides: Partial<SessionSyncResult> = {}): SessionSyncResult {
  return {
    imported: 2,
    skipped: 1,
    filesScanned: 3,
    suspectedDuplicates: 1,
    deferredFiles: 0,
    errors: [],
    ...overrides,
  };
}

beforeEach(() => vi.clearAllMocks());

describe("session accounting actions", () => {
  it("reports corrections separately from new imports", async () => {
    vi.mocked(compatApi.syncSessionUsage).mockResolvedValue(result({ updated: 3 }));
    render(<SessionUsageActions />);
    fireEvent.click(screen.getByRole("button", { name: "同步用量" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", expect.stringContaining("3 条更新")));
  });

  it("shows source errors in a reusable styled dialog without a success toast", async () => {
    vi.mocked(compatApi.syncSessionUsage).mockResolvedValue(result({ errors: ["Cannot read native database"] }));
    render(<SessionUsageActions />);
    fireEvent.click(screen.getByRole("button", { name: "同步用量" }));
    expect(await screen.findByRole("dialog", { name: "用量同步结果" })).toBeTruthy();
    expect(screen.getByText("Cannot read native database")).toBeTruthy();
    expect(showToast).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    fireEvent.click(screen.getByRole("button", { name: "同步详情" }));
    expect(screen.getByRole("dialog", { name: "用量同步结果" })).toBeTruthy();
  });

  it("reports unfinished sources as pending instead of successful completion", async () => {
    vi.mocked(compatApi.syncSessionUsage).mockResolvedValue(result({ deferredFiles: 1 }));
    render(<SessionUsageActions />);
    fireEvent.click(screen.getByRole("button", { name: "同步用量" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("info", expect.stringContaining("等待下次同步")));
  });

  it("prevents overlapping imports and re-enables controls after failure", async () => {
    let reject!: (reason: Error) => void;
    vi.mocked(compatApi.syncSessionUsage).mockImplementation(
      () =>
        new Promise((_, rejectRequest) => {
          reject = rejectRequest;
        }),
    );
    render(<SessionUsageActions />);
    const sync = screen.getByRole("button", { name: "同步用量" }) as HTMLButtonElement;
    fireEvent.click(sync);
    expect(sync.disabled).toBe(true);
    expect((screen.getByRole("button", { name: "重建 Codex" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(sync);
    expect(compatApi.syncSessionUsage).toHaveBeenCalledTimes(1);
    reject(new Error("failed"));
    await waitFor(() => expect(sync.disabled).toBe(false));
    expect(showToast).toHaveBeenCalledWith("error", "Error: failed");
  });

  it("keeps the existing accounting when rebuild confirmation is cancelled", async () => {
    render(<SessionUsageActions />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "重建 Codex" }));
    });
    expect(compatApi.rebuildCodexUsage).not.toHaveBeenCalled();
    expect(showToast).not.toHaveBeenCalled();
  });
});
