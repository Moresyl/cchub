import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProxyRequestRow from "./ProxyRequestRow";

describe("proxy request identity", () => {
  it("keeps the requested and served names visible and supports keyboard activation without scrolling", () => {
    const onSelect = vi.fn();
    render(
      <ProxyRequestRow
        item={{ request_id: "request", provider_name: "Provider", error_message: null, status_code: 200 }}
        success
        toolLabel="Claude"
        modelLabel="core"
        responseModelLabel="回复模型: vendor/core"
        costLabel="$2"
        tokenLabel="1M tok"
        latencyLabel="10ms"
        timingLabel="首个输出 2 ms · ~40.0 tok/s"
        createdAtLabel="now"
        onSelect={onSelect}
      />,
    );
    expect(screen.getByText("core")).toBeTruthy();
    expect(screen.getByText("回复模型: vendor/core")).toBeTruthy();
    expect(screen.getByText("首个输出 2 ms · ~40.0 tok/s")).toBeTruthy();
    const row = screen.getByRole("button");
    const event = new KeyboardEvent("keydown", { key: " ", bubbles: true, cancelable: true });
    fireEvent(row, event);
    expect(event.defaultPrevented).toBe(true);
    fireEvent.keyDown(row, { key: "Enter" });
    fireEvent.click(row);
    expect(onSelect).toHaveBeenCalledTimes(3);
  });
});
