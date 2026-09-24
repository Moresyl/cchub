import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Usage from "./Usage";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("../lib/i18n", () => ({ getLocale: () => "zh" }));
vi.mock("../components/ModelsDevSyncPanel", () => ({ default: () => null }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({
    days: 7,
    start_date: "2026-09-18",
    end_date: "2026-09-24",
    summary: {
      total_requests: 0,
      success_requests: 0,
      success_rate: 0,
      input_tokens: 0,
      output_tokens: 0,
      cache_read_tokens: 0,
      cache_creation_tokens: 0,
      total_cost_usd: "0",
    },
    trends: [{ date: "2026-09-18", requests: 0, total_cost_usd: "0" }],
    providers: [],
    models: [],
  });
});

describe("Usage filters", () => {
  it("shows distinct date-range controls and updates the query", async () => {
    render(<Usage />);
    await screen.findByRole("group", { name: "时间范围" });

    expect(screen.getByRole("button", { name: "近 7 天" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "近 30 天" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenLastCalledWith("get_usage_analytics", expect.objectContaining({ days: 30 }));
    });
    expect(screen.getByRole("button", { name: "近 30 天" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("combobox", { name: "应用" })).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Provider" })).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "模型" })).toBeTruthy();
    expect(screen.getByText("暂无用量记录")).toBeTruthy();
    expect(screen.queryByText("09-18")).toBeNull();
  });

  it("keeps trend rows when requests exist", async () => {
    invoke.mockResolvedValue({
      days: 7,
      start_date: "2026-09-18",
      end_date: "2026-09-24",
      summary: {
        total_requests: 2,
        success_rate: 1,
        input_tokens: 10,
        output_tokens: 5,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        total_cost_usd: "0.01",
      },
      trends: [{ date: "2026-09-18", requests: 2, total_cost_usd: "0.01" }],
      providers: [],
      models: [],
    });

    render(<Usage />);
    expect(await screen.findByText("09-18")).toBeTruthy();
    expect(screen.queryByText("暂无用量记录")).toBeNull();
  });
});
