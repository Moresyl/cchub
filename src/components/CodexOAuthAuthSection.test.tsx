import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import CodexOAuthAuthSection from "./CodexOAuthAuthSection";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-shell", () => ({ open: vi.fn() }));
vi.mock("./AppDialogProvider", () => ({ useAppDialog: () => ({ confirm: vi.fn() }) }));
const localeText = (zh: string) => zh;
const account = { id: "one", login: "saved@example.com", authenticatedAt: 1, requiresReauth: true };
const status = { accounts: [account], defaultAccountId: "one", authenticated: true };
afterEach(cleanup);
beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("OAuth account health", () => {
  it("retains quota when refreshing the same login through the account panel", async () => {
    let quotaReads = 0;
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "codex_oauth_get_status") return { ...status, accounts: [{ ...account, requiresReauth: false }] };
      if (command === "get_codex_oauth_quota") {
        if (quotaReads++ === 0) return { success: true, tiers: [{ name: "weekly", utilization: 42 }] };
        throw new Error("offline");
      }
      throw new Error(`Unexpected request: ${command}`);
    });
    render(<CodexOAuthAuthSection localeText={localeText} />);
    await screen.findByText("42%");
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    await screen.findByText("更新失败，以下为上次成功读取的数据。");
    expect(screen.getByText("42%")).toBeTruthy();
    expect(quotaReads).toBe(2);
  });

  it("drops quota from an earlier login when the same account is reauthorized", async () => {
    let statusReads = 0;
    let quotaReads = 0;
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "codex_oauth_get_status")
        return {
          ...status,
          accounts: [{ ...account, requiresReauth: false, authenticatedAt: statusReads++ === 0 ? 1 : 2 }],
        };
      if (command === "get_codex_oauth_quota") {
        if (quotaReads++ === 0) return { success: true, tiers: [{ name: "weekly", utilization: 42 }] };
        throw new Error("offline");
      }
      throw new Error(`Unexpected request: ${command}`);
    });
    render(<CodexOAuthAuthSection localeText={localeText} />);
    await screen.findByText("42%");
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    await screen.findByText("配额查询失败");
    expect(screen.queryByText("42%")).toBeNull();
    expect(quotaReads).toBe(2);
  });

  it("keeps expired accounts visible with a direct reauthorization action", async () => {
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "codex_oauth_get_status") return status;
      if (command === "codex_oauth_start_device_flow")
        return {
          deviceCode: "device",
          userCode: "ABCD",
          verificationUri: "https://example.test",
          expiresIn: 300,
          interval: 5,
        };
      if (command === "codex_oauth_cancel_device_flow") return null;
      throw new Error(`Unexpected request: ${command}`);
    });
    render(<CodexOAuthAuthSection localeText={localeText} />);
    expect(await screen.findByText("授权已失效，请重新登录此账号。")).toBeTruthy();
    expect(screen.getByText("saved@example.com")).toBeTruthy();
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "get_codex_oauth_quota")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "重新登录" }));
    expect(await screen.findByText("ABCD")).toBeTruthy();
    expect(screen.getByRole("button", { name: "取消" }).getAttribute("data-slot")).toBe("button");
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(invoke).toHaveBeenCalledWith("codex_oauth_cancel_device_flow", { deviceCode: "device" });
  });

  it("reloads health after a rejected quota without querying an expired account again", async () => {
    let reads = 0;
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "codex_oauth_get_status")
        return reads++ === 0 ? { ...status, accounts: [{ ...account, requiresReauth: false }] } : status;
      if (command === "get_codex_oauth_quota") return { success: false, tiers: [] };
      throw new Error(`Unexpected request: ${command}`);
    });
    render(<CodexOAuthAuthSection localeText={localeText} />);
    expect(await screen.findByText("授权已失效，请重新登录此账号。")).toBeTruthy();
    await waitFor(() =>
      expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "get_codex_oauth_quota")).toHaveLength(1),
    );
    expect(reads).toBe(2);
  });
});
