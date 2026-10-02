import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import CopilotUsagePanel from "./CopilotUsagePanel";
import type { CopilotAccountResources } from "../lib/copilotAccounts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
const account = { id: "1", login: "test-account", revision: "login-one", avatar_url: null, authenticated_at: 1 };
const auth = {
  accounts: [account],
  default_account_id: "1",
  authenticated: true,
  username: account.login,
  expires_at: null,
};
const data: CopilotAccountResources = {
  account,
  fetched_at: "2026-10-01T08:00:00Z",
  usage: {
    copilot_plan: "individual_pro",
    quota_reset_date: "2026-11-01",
    quota_snapshots: {
      premium_interactions: { entitlement: 100, remaining: 40, percent_remaining: 40, unlimited: false },
      chat: { entitlement: null, remaining: null, percent_remaining: null, unlimited: true },
      completions: null,
    },
  },
  models: [{ id: "test-model", name: "Test Model", vendor: "fixture" }],
  usage_error: null,
  models_error: null,
};
beforeEach(() => vi.mocked(invoke).mockReset().mockResolvedValueOnce(auth));
afterEach(cleanup);
const text = (zh: string) => zh;
describe("Copilot quota and model panel", () => {
  it("keeps quota visible when models fail, with an accessible retry and account selector", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ ...data, models: null, models_error: "rate_limited" });
    render(<CopilotUsagePanel localeText={text} />);
    await waitFor(() => expect(screen.getByText("40 / 100")).toBeTruthy());
    expect(screen.getByRole("alert").textContent).toContain("模型: 查询过于频繁");
    expect(screen.getByRole("combobox", { name: "查询 Copilot 账号" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "刷新 Copilot 配额与模型" }).getAttribute("type")).toBe("button");
    expect(screen.getByText("不限量")).toBeTruthy();
    expect(screen.getByText("未提供")).toBeTruthy();
    expect(screen.getAllByRole("progressbar")).toHaveLength(1);
    expect(screen.getByRole("progressbar", { name: "Premium 请求" }).getAttribute("aria-valuenow")).toBe("40");
  });
  it("retains an expandable model list when quota fails", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ ...data, usage: null, usage_error: "sign_in_required" });
    render(<CopilotUsagePanel localeText={text} />);
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("重新登录此账号"));
    const button = screen.getByRole("button", { name: "可用模型 1 个" });
    fireEvent.click(button);
    expect(button.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText("Test Model")).toBeTruthy();
    expect(screen.getByText("test-model · fixture")).toBeTruthy();
    fireEvent.click(button);
    expect(screen.queryByText("Test Model")).toBeNull();
  });
  it("clears a failed refresh and never displays raw request errors", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(data);
    render(<CopilotUsagePanel localeText={text} />);
    await waitFor(() => expect(screen.getByText("40 / 100")).toBeTruthy());
    vi.mocked(invoke).mockRejectedValueOnce(new Error("https://secret.test?token=private"));
    fireEvent.click(screen.getByRole("button", { name: "刷新 Copilot 配额与模型" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("刷新重试"));
    expect(screen.queryByText("40 / 100")).toBeNull();
    expect(document.body.textContent).not.toContain("private");
    expect(document.body.textContent).not.toContain("secret");
  });
});
