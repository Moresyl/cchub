import { fireEvent, render, screen } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { parseCodexStructuredConfig, updateCodexStructuredContent } from "../../lib/codexConfig";
import CodexStructuredFields from "./CodexStructuredFields";

const originalScroll = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollIntoView");
beforeAll(() => Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() }));
afterAll(() => {
  if (originalScroll) Object.defineProperty(HTMLElement.prototype, "scrollIntoView", originalScroll);
  else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
});

describe("configuration file reasoning fields", () => {
  it("exposes imported custom effort and explicitly removes it without losing comments", async () => {
    const source = "model = 'keep'\nmodel_reasoning_effort = 'max' # user note\n[mcp_servers.keep]\ncommand = 'node'\n";
    const onPatch = vi.fn();
    render(
      <CodexStructuredFields
        zh
        config={parseCodexStructuredConfig(source)}
        validation={null}
        onPatch={onPatch}
        onAddMcp={vi.fn()}
        onContextWindow1M={vi.fn()}
        apiKey=""
        onApiKeyChange={vi.fn()}
        invalidContextWindow={false}
        invalidCompactLimit={false}
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "推理强度" });
    expect(screen.getByLabelText("推理强度")).toBe(trigger);
    expect(trigger.textContent).toBe("max");
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("option", { name: "使用模型默认值" }));
    expect(onPatch).toHaveBeenCalledExactlyOnceWith({ reasoningEffort: "" });
    const updated = updateCodexStructuredContent(source, onPatch.mock.calls[0][0]);
    expect(updated).not.toContain("model_reasoning_effort");
    expect(updated).toContain("# user note");
    expect(updated).toContain("command = 'node'");
  });
});
