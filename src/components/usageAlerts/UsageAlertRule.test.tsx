import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import UsageAlertRule from "./UsageAlertRule";
import { defaultSettings } from "./types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
const data = {
  rules: [
    {
      profileId: "one",
      queryIdentity: "hash",
      settings: defaultSettings,
      paused: false,
      status: "off",
      checkedAt: null,
    },
  ],
  events: [],
  polling: false,
};
beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue(data);
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe = vi.fn();
      unobserve = vi.fn();
      disconnect = vi.fn();
    },
  );
});
afterEach(() => vi.unstubAllGlobals());

async function open() {
  render(<UsageAlertRule profileId="one" locale="zh" />);
  fireEvent.click(screen.getByRole("button", { name: /余额与额度提醒/ }));
  await screen.findByRole("form", { name: "提醒设置" });
}

describe("usage alert settings", () => {
  it("explains unavailable query configuration and disables saving without an account identity", async () => {
    vi.mocked(invoke).mockResolvedValue({ ...data, rules: [{ ...data.rules[0], queryIdentity: null }] });
    await open();
    expect(screen.getByText(/当前配置无法用于用量提醒/)).toBeTruthy();
    expect((screen.getByRole("button", { name: "保存提醒设置" }) as HTMLButtonElement).disabled).toBe(true);
    expect(invoke).toHaveBeenCalledTimes(1);
  });
  it("defaults off and saves only after an explicit action", async () => {
    await open();
    expect(screen.getByRole("switch", { name: "自动检查并提醒" }).getAttribute("aria-checked")).toBe("false");
    fireEvent.click(screen.getByRole("switch", { name: "自动检查并提醒" }));
    expect((screen.getByRole("spinbutton", { name: "额度百分比" }) as HTMLInputElement).value).toBe("80");
    expect(invoke).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "保存提醒设置" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("set_usage_alert_rule", {
        profileId: "one",
        expectedIdentity: "hash",
        settings: { ...defaultSettings, enabled: true, quotaPercent: 80 },
      }),
    );
    await screen.findByText("已保存");
  });

  it("adding or removing a threshold never submits the form and duplicate units prevent saving", async () => {
    await open();
    fireEvent.click(screen.getByRole("button", { name: "添加阈值" }));
    expect(invoke).toHaveBeenCalledTimes(1);
    expect((screen.getByRole("button", { name: "保存提醒设置" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "余额单位 1" }), { target: { value: "USD" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "余额阈值 1" }), { target: { value: "0" } });
    expect((screen.getByRole("button", { name: "保存提醒设置" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "添加阈值" }));
    fireEvent.change(screen.getByRole("textbox", { name: "余额单位 2" }), { target: { value: "usd" } });
    expect((screen.getByRole("button", { name: "保存提醒设置" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "移除阈值 2" }));
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("explains paused monitoring before rebinding an account", async () => {
    vi.mocked(invoke).mockResolvedValue({
      ...data,
      rules: [{ ...data.rules[0], paused: true, settings: { ...defaultSettings, enabled: true, quotaPercent: 80 } }],
    });
    await open();
    expect(screen.getByText(/后台检查已暂停/)).toBeTruthy();
  });
});
