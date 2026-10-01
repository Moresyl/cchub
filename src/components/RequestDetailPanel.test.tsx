import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import RequestDetailPanel, { type RequestDetailRecord } from "./RequestDetailPanel";

const record: RequestDetailRecord = {
  request_id: "fixture",
  tool_id: "claude",
  profile_id: "p2",
  provider_name: "Recovered provider",
  request_model: "model",
  response_model: "model",
  input_tokens: 11,
  output_tokens: 3,
  cache_read_tokens: 0,
  cache_creation_tokens: 0,
  total_cost_usd: "0.000014",
  latency_ms: 50,
  status_code: 200,
  is_streaming: true,
  error_message: null,
  created_at: "2026-10-02",
  stream_attempts: [
    {
      attempt_id: "a1",
      profile_id: "p1",
      provider_name: "Initial provider",
      model: "model",
      response_model: null,
      input_tokens: 7,
      output_tokens: 0,
      cache_read_tokens: 2,
      cache_creation_tokens: 0,
      total_cost_usd: "0.000009",
      status_code: 429,
    },
  ],
};
const props = { title: "请求明细", closeLabel: "关闭", localeText: (zh: string) => zh, onClose: vi.fn() };

describe("request detail panel", () => {
  it("separates final response metrics from failed attempts and supports collapse and close", () => {
    render(<RequestDetailPanel {...props} record={record} loading={false} />);
    expect(screen.getByText("11 / 3")).toBeTruthy();
    expect(screen.queryByText("7 / 0")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "流式失败尝试" }));
    expect(screen.getByText("7 / 0")).toBeTruthy();
    expect(screen.getByText(/未合并到用量汇总/)).toBeTruthy();
    expect(screen.getByText(/Initial provider · HTTP 429/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "流式失败尝试" }));
    expect(screen.queryByText("7 / 0")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    expect(props.onClose).toHaveBeenCalledOnce();
  });

  it("keeps loading and errors accessible and omits an empty attempt section", () => {
    const retry = vi.fn();
    const view = render(<RequestDetailPanel {...props} record={null} loading />);
    expect(screen.getByRole("status")).toBeTruthy();
    view.rerender(<RequestDetailPanel {...props} record={null} loading={false} error="加载失败" onRetry={retry} />);
    expect(screen.getByRole("alert").textContent).toContain("加载失败");
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(retry).toHaveBeenCalledOnce();
    view.rerender(<RequestDetailPanel {...props} record={{ ...record, stream_attempts: undefined }} loading={false} />);
    expect(screen.queryByRole("button", { name: "流式失败尝试" })).toBeNull();
  });
});
