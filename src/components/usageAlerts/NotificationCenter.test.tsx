import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import NotificationCenter from "./NotificationCenter";
import { defaultSettings, type AlertEvent } from "./types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
const event: AlertEvent = {
  id: "alert",
  profileId: "one",
  profileName: "Wallet",
  toolId: "claude",
  kind: "balance",
  label: "USD",
  value: 0,
  threshold: 5,
  unit: "USD",
  resetAt: null,
  createdAt: 1780272000,
  read: false,
  systemStatus: "accepted",
};
const data = {
  rules: [{ profileId: "one", settings: defaultSettings, paused: false, status: "off", checkedAt: null }],
  events: [event],
  polling: false,
};
beforeEach(() => vi.mocked(invoke).mockReset().mockResolvedValue(data));

describe("notification center", () => {
  it("shows unread history and accepted system submission without claiming delivery", async () => {
    render(<NotificationCenter locale="zh" />);
    fireEvent.click(await screen.findByRole("button", { name: "通知中心 · 1" }));
    expect(await screen.findByText("余额 0 USD，不高于 5")).toBeTruthy();
    expect(screen.getByText("已交给系统")).toBeTruthy();
    expect(screen.queryByText("已送达")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "标为已读" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("mark_usage_alerts_read", { eventId: "alert" }));
  });

  it("offers retries only for failed system notifications", async () => {
    vi.mocked(invoke).mockResolvedValue({ ...data, events: [{ ...event, systemStatus: "failed" }] });
    render(<NotificationCenter locale="en" />);
    fireEvent.click(await screen.findByRole("button", { name: "Notification center · 1" }));
    fireEvent.click(await screen.findByRole("button", { name: "Retry system notification" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("retry_usage_alert", { eventId: "alert" }));
  });
});
