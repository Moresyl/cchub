import { fireEvent, render, screen } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import CodexTab from "./CodexTab";

const originalScroll = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollIntoView");
beforeAll(() => Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() }));
afterAll(() => {
  if (originalScroll) Object.defineProperty(HTMLElement.prototype, "scrollIntoView", originalScroll);
  else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
});

describe("tool settings reasoning", () => {
  it("preserves an unknown effort and sends an empty value when selecting the model default", async () => {
    const onSelect = vi.fn();
    render(
      <CodexTab
        uiText={(zh) => zh}
        codexApproval="suggest"
        codexApprovalOptions={[]}
        handleSelectCodexApproval={vi.fn()}
        codexReasoning="future-effort"
        handleSelectCodexReasoning={onSelect}
        codexDisableStorage={false}
        handleToggleCodexDisableStorage={vi.fn()}
        codexContextWindow1M={false}
        handleToggleCodexContextWindow1M={vi.fn()}
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "推理强度" });
    expect(trigger.textContent).toBe("future-effort");
    expect(onSelect).not.toHaveBeenCalled();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("option", { name: "使用模型默认值" }));
    expect(onSelect).toHaveBeenCalledExactlyOnceWith("");
  });
});
