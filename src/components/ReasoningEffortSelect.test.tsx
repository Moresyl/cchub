import { fireEvent, render, screen } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import ReasoningEffortSelect from "./ReasoningEffortSelect";

const localeText = (zh: string) => zh;
const originalScroll = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollIntoView");
beforeAll(() => Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() }));
afterAll(() => {
  if (originalScroll) Object.defineProperty(HTMLElement.prototype, "scrollIntoView", originalScroll);
  else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
});

describe("reasoning effort selector", () => {
  it("shows the reported default without writing it into the saved configuration", async () => {
    const onValueChange = vi.fn();
    render(
      <ReasoningEffortSelect
        value="high"
        reportedLevels={["high", "max"]}
        defaultEffort="max"
        onValueChange={onValueChange}
        localeText={localeText}
      />,
    );
    expect(onValueChange).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("option", { name: "使用模型默认值（max）" }));
    expect(onValueChange).toHaveBeenCalledExactlyOnceWith("");
  });
  it("reports no configurable levels without offering the legacy four", () => {
    const onValueChange = vi.fn();
    render(
      <ReasoningEffortSelect value="" reportedLevels={[]} onValueChange={onValueChange} localeText={localeText} />,
    );
    const trigger = screen.getByRole("combobox", { name: "推理强度" });
    expect(trigger).toHaveProperty("disabled", true);
    expect(trigger.textContent).toContain("使用模型默认值");
    expect(screen.getByText(/明确未提供可配置/)).toBeTruthy();
    expect(trigger.getAttribute("aria-describedby")).toBe(screen.getByText(/明确未提供可配置/).id);
    expect(onValueChange).not.toHaveBeenCalled();
  });

  it("keeps an unsupported saved value visible and allows explicit removal", async () => {
    const onValueChange = vi.fn();
    render(
      <ReasoningEffortSelect value="high" reportedLevels={[]} onValueChange={onValueChange} localeText={localeText} />,
    );
    const trigger = screen.getByRole("combobox", { name: "推理强度" });
    expect(trigger.textContent).toContain("high（原配置");
    expect(trigger).toHaveProperty("disabled", false);
    expect(onValueChange).not.toHaveBeenCalled();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const current = await screen.findByRole("option", { name: /high（原配置/ });
    expect(current.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(screen.getByRole("option", { name: "使用模型默认值" }));
    expect(onValueChange).toHaveBeenCalledExactlyOnceWith("");
  });

  it("uses only reported choices and refreshes them without changing the saved value", async () => {
    const onValueChange = vi.fn();
    const view = render(
      <ReasoningEffortSelect
        value="max"
        reportedLevels={["none", "max"]}
        onValueChange={onValueChange}
        localeText={localeText}
      />,
    );
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "ArrowDown" });
    expect((await screen.findAllByRole("option")).map((option) => option.textContent)).toEqual([
      "使用模型默认值",
      "none",
      "max",
    ]);
    fireEvent.click(screen.getByRole("option", { name: "none" }));
    expect(onValueChange).toHaveBeenCalledExactlyOnceWith("none");
    onValueChange.mockClear();
    view.rerender(
      <ReasoningEffortSelect
        value="max"
        reportedLevels={["low"]}
        onValueChange={onValueChange}
        localeText={localeText}
      />,
    );
    expect(screen.getByRole("combobox").textContent).toContain("max（原配置");
    expect(screen.getByText(/未被当前模型报告支持/)).toBeTruthy();
    expect(onValueChange).not.toHaveBeenCalled();
  });

  it("retains custom effort when capabilities are unknown and never confuses none with default", async () => {
    const onValueChange = vi.fn();
    render(<ReasoningEffortSelect value="none" onValueChange={onValueChange} localeText={localeText} />);
    expect(screen.getByRole("combobox").textContent).toBe("none");
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "ArrowDown" });
    expect((await screen.findAllByRole("option")).map((option) => option.textContent)).toEqual([
      "使用模型默认值",
      "low",
      "medium",
      "high",
      "xhigh",
      "none",
    ]);
    fireEvent.click(screen.getByRole("option", { name: "使用模型默认值" }));
    expect(onValueChange).toHaveBeenCalledExactlyOnceWith("");
  });
});
