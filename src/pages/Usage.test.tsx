import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

afterEach(cleanup);

describe("Usage filters", () => {
  it("keeps filters mounted and focused during a slow filter change", async () => {
    render(<Usage />);
    await screen.findByText("暂无用量记录");
    let resolve!: (value: unknown) => void;
    const held = new Promise((yes) => {
      resolve = yes;
    });
    invoke.mockReturnValueOnce(held);
    const range = screen.getByRole("button", { name: "近 30 天" });
    range.focus();
    fireEvent.click(range);
    expect(document.activeElement).toBe(range);
    expect(screen.getByRole("combobox", { name: "应用" })).toBeTruthy();
    expect(screen.queryByText("暂无用量记录")).toBeNull();
    expect(screen.getByText("正在加载用量分析...")).toBeTruthy();
    await act(async () => {
      resolve(await invoke.mock.results[0].value);
    });
    expect(screen.getByText("暂无用量记录")).toBeTruthy();
    expect(document.activeElement).toBe(range);
  });

  it("shows a safe initial error with usable filters and retry", async () => {
    invoke.mockRejectedValueOnce("token=secret; C:/private/database.sqlite");
    render(<Usage />);
    await screen.findByText("无法读取本地用量，请重试。");
    expect(screen.queryByText(/token=secret/)).toBeNull();
    expect(screen.getByRole("combobox", { name: "应用" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(await screen.findByText("暂无用量记录")).toBeTruthy();
  });

  it("labels retained results after refresh failure and removes the warning after recovery", async () => {
    render(<Usage />);
    await screen.findByText("暂无用量记录");
    invoke.mockRejectedValueOnce("sensitive error");
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      "刷新失败，以下仍是上次成功读取的数据。请重试。",
    );
    expect(screen.getByText("暂无用量记录")).toBeTruthy();
    expect(screen.queryByText(/sensitive error/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("makes rankings beyond the first twelve accessible with independent pagination", async () => {
    const initial = await invoke();
    invoke.mockClear();
    invoke.mockResolvedValue({
      ...initial,
      providers: Array.from({ length: 25 }, (_, index) => ({
        provider_name: `provider-${index + 1}`,
        app_id: "claude",
        requests: index + 1,
        success_rate: 100,
        total_tokens: 1,
        total_cost_usd: "0.1",
        avg_latency_ms: 10,
      })),
    });
    render(<Usage />);
    const table = await screen.findByRole("table", { name: "Provider 排名" });
    const navigation = screen.getByRole("navigation", { name: "Provider 排名分页" });
    expect(within(table).queryByText("provider-13")).toBeNull();
    expect(within(navigation).getByRole("button", { name: "上一页" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(within(navigation).getByRole("button", { name: "下一页" }));
    expect(within(table).getByText("provider-13")).toBeTruthy();
    const compact = screen.getByRole("list", { name: "Provider 排名列表" });
    expect(within(compact).getByRole("heading", { name: "provider-13" })).toBeTruthy();
    expect(within(compact).getAllByText("成功率")).toHaveLength(12);
    expect(screen.getByText("13–24 / 25")).toBeTruthy();
    fireEvent.click(within(navigation).getByRole("button", { name: "下一页" }));
    expect(within(table).getByText("provider-25")).toBeTruthy();
    expect(within(navigation).getByRole("button", { name: "下一页" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(within(navigation).getByRole("button", { name: "上一页" }));
    expect(within(table).getByText("provider-13")).toBeTruthy();
    expect(table.parentElement?.tabIndex).toBe(0);
  });
  it("preserves the legacy category fallback when a backend omits total tokens", async () => {
    invoke.mockResolvedValue({
      days: 7,
      start_date: "2026-09-18",
      end_date: "2026-09-24",
      summary: {
        total_requests: 1,
        success_requests: 1,
        success_rate: 100,
        input_tokens: 100,
        output_tokens: 5,
        cache_read_tokens: 800,
        cache_creation_tokens: 100,
        total_cost_usd: "0.001015",
      },
      trends: [],
      providers: [],
      models: [],
    });
    render(<Usage />);
    expect(await screen.findByText("1,005")).toBeTruthy();
  });
  it("uses the reported total without adding cached input a second time", async () => {
    invoke.mockResolvedValue({
      days: 7,
      start_date: "2026-09-18",
      end_date: "2026-09-24",
      summary: {
        total_requests: 1,
        success_requests: 1,
        success_rate: 100,
        input_tokens: 1000,
        output_tokens: 5,
        cache_read_tokens: 800,
        cache_creation_tokens: 100,
        total_tokens: 1005,
        total_cost_usd: "0.001015",
      },
      trends: [],
      providers: [],
      models: [],
    });
    render(<Usage />);
    expect(await screen.findByText("1,005")).toBeTruthy();
    expect(screen.queryByText("1,905")).toBeNull();
  });
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
