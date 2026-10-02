import { act, createEvent, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import ModelSelector, { type ModelInfo } from "./ModelSelector";
import { setLocale } from "../lib/i18n";
import { Dialog, DialogBody, DialogContent, DialogHeader, DialogTitle } from "./ui/dialog";

beforeEach(() => setLocale("zh"));

const models: ModelInfo[] = [
  { id: "model-alpha", displayName: "Alpha", contextWindow: 200_000 },
  { id: "model-beta", displayName: "Beta", maxOutputTokens: 16_000 },
];

beforeAll(() => {
  Object.defineProperty(Element.prototype, "hasPointerCapture", {
    configurable: true,
    value: () => false,
  });
  Object.defineProperty(Element.prototype, "setPointerCapture", {
    configurable: true,
    value: () => undefined,
  });
  Object.defineProperty(Element.prototype, "releasePointerCapture", {
    configurable: true,
    value: () => undefined,
  });
  Object.defineProperty(Element.prototype, "scrollIntoView", {
    configurable: true,
    value: () => undefined,
  });
  Object.defineProperty(globalThis, "ResizeObserver", {
    configurable: true,
    value: class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  });
});

describe("ModelSelector", () => {
  it.each([
    { top: 1_000, bottom: 1_040, height: 40, initialScroll: 0, expected: 724 },
    { top: 110, bottom: 150, height: 40, initialScroll: 0, expected: 0 },
    { top: 60, bottom: 100, height: 40, initialScroll: 900, expected: 860 },
    { top: 1_000, bottom: 1_300, height: 300, initialScroll: 0, expected: 900 },
  ])(
    "aligns a measured selection at $top within the list",
    async ({ top, bottom, height, initialScroll, expected }) => {
      const callbacks: (() => void)[] = [];
      vi.stubGlobal(
        "ResizeObserver",
        class {
          constructor(callback: ResizeObserverCallback) {
            callbacks.push(() => callback([], this as unknown as ResizeObserver));
          }
          observe() {}
          unobserve() {}
          disconnect() {}
        },
      );
      try {
        render(<ModelSelector value="model-beta" models={models} onChange={vi.fn()} />);
        fireEvent.click(screen.getByRole("combobox", { name: "选择模型" }));
        const list = await screen.findByRole("listbox", { name: "选择模型" });
        const selected = screen.getByRole("option", { name: /model-beta/ });
        const listRect = { top: 100, bottom: 316, height: 216 } as DOMRect;
        const itemRect = { top, bottom, height } as DOMRect;
        list.scrollTop = initialScroll;
        vi.spyOn(list, "getBoundingClientRect").mockReturnValue(listRect);
        vi.spyOn(selected, "getBoundingClientRect").mockReturnValue(itemRect);
        act(() => callbacks.forEach((callback) => callback()));
        expect(list.scrollTop).toBe(initialScroll);
        Object.defineProperty(list, "clientHeight", { value: 216 });
        act(() => callbacks.forEach((callback) => callback()));
        expect(list.scrollTop).toBe(expected);
      } finally {
        vi.unstubAllGlobals();
      }
    },
  );

  it.each(["touch", "pen"])("lets %s users browse without focusing the search input", async (pointerType) => {
    const onChange = vi.fn();
    render(<ModelSelector value="model-beta" models={models} onChange={onChange} />);
    const trigger = screen.getByRole("combobox", { name: "选择模型" });
    const pointer = createEvent.pointerDown(trigger);
    Object.defineProperty(pointer, "pointerType", { value: pointerType });
    fireEvent(trigger, pointer);
    fireEvent.click(trigger, { detail: 1 });
    const input = await screen.findByRole("combobox", { name: "搜索模型" });
    const list = screen.getByRole("listbox", { name: "选择模型" });
    await waitFor(() => expect(document.activeElement).toBe(list));
    const focusSearch = vi.spyOn(input, "focus");
    fireEvent.pointerMove(screen.getByRole("option", { name: /model-alpha/ }));
    fireEvent.keyDown(list, { key: "ArrowUp" });
    await waitFor(() =>
      expect(screen.getByRole("option", { name: /model-alpha/ }).getAttribute("data-selected")).toBe("true"),
    );
    fireEvent.click(screen.getByRole("option", { name: /model-alpha/ }));
    expect(focusSearch).not.toHaveBeenCalled();
    expect(onChange).toHaveBeenCalledWith("model-alpha");
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });

  it("focuses search for a mouse and returns focus on Escape", async () => {
    render(<ModelSelector value="model-beta" models={models} onChange={vi.fn()} />);
    const trigger = screen.getByRole("combobox", { name: "选择模型" });
    const pointer = createEvent.pointerDown(trigger);
    Object.defineProperty(pointer, "pointerType", { value: "mouse" });
    fireEvent(trigger, pointer);
    fireEvent.click(trigger, { detail: 1 });
    const input = await screen.findByRole("combobox", { name: "搜索模型" });
    await waitFor(() => expect(document.activeElement).toBe(input));
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(trigger));
    expect(screen.queryByRole("combobox", { name: "搜索模型" })).toBeNull();
  });

  it.each(["ArrowDown", "ArrowUp"])("opens with %s and preserves the current model", async (key) => {
    const onChange = vi.fn();
    render(<ModelSelector value="model-beta" models={models} onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("combobox", { name: "选择模型" }), { key });
    const input = await screen.findByRole("combobox", { name: "搜索模型" });
    await waitFor(() => expect(document.activeElement).toBe(input));
    await waitFor(() =>
      expect(screen.getByRole("option", { name: /model-beta/ }).getAttribute("data-selected")).toBe("true"),
    );
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("model-beta");
  });

  it("switches back to keyboard search after browsing with touch", async () => {
    render(<ModelSelector value="model-beta" models={models} onChange={vi.fn()} />);
    const trigger = screen.getByRole("combobox", { name: "选择模型" });
    const pointer = createEvent.pointerDown(trigger);
    Object.defineProperty(pointer, "pointerType", { value: "touch" });
    fireEvent(trigger, pointer);
    fireEvent.click(trigger, { detail: 1 });
    await screen.findByRole("combobox", { name: "搜索模型" });
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("listbox", { name: "选择模型" })));
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("combobox", { name: "搜索模型" })).toBeNull());
    fireEvent.click(trigger, { detail: 0 });
    const reopened = await screen.findByRole("combobox", { name: "搜索模型" });
    await waitFor(() => expect(document.activeElement).toBe(reopened));
  });

  it("dismisses only the model popup inside a dialog and returns focus to the field", async () => {
    render(
      <Dialog open>
        <DialogContent aria-describedby={undefined}>
          <DialogHeader>
            <DialogTitle>编辑配置</DialogTitle>
          </DialogHeader>
          <DialogBody>
            <ModelSelector value="model-beta" models={models} onChange={vi.fn()} />
          </DialogBody>
        </DialogContent>
      </Dialog>,
    );
    const trigger = screen.getByRole("combobox", { name: "选择模型" });
    fireEvent.click(trigger);
    const input = await screen.findByRole("combobox", { name: "搜索模型" });
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(trigger));
    expect(screen.queryByRole("combobox", { name: "搜索模型" })).toBeNull();
    expect(screen.getByRole("dialog", { name: "编辑配置" })).toBeTruthy();
  });

  it("shows reported request units while selecting the unchanged model ID", async () => {
    const onChange = vi.fn();
    render(
      <ModelSelector
        value="model-alpha"
        models={[
          { ...models[0], premiumRequestBilling: { kind: "free", multiplier: 0 } },
          { ...models[1], premiumRequestBilling: { kind: "premium", multiplier: 0.33 } },
        ]}
        onChange={onChange}
      />,
    );
    fireEvent.click(screen.getByRole("combobox", { name: "选择模型" }));
    await screen.findByText("高级请求 ×0.33");
    expect(screen.getByText("不消耗高级额度")).toBeTruthy();
    fireEvent.click(screen.getByRole("option", { name: /model-beta/ }));
    expect(onChange).toHaveBeenCalledWith("model-beta");
  });
  it("localizes search and custom-model controls in English", async () => {
    setLocale("en");
    render(<ModelSelector value="model-alpha" models={models} onChange={vi.fn()} />);
    fireEvent.click(screen.getByRole("combobox", { name: "Choose model" }));
    const input = await screen.findByRole("combobox", { name: "Search models" });
    fireEvent.change(input, { target: { value: "custom" } });
    expect(screen.getByText("Custom model ID")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Clear model" })).toBeTruthy();
  });
  it("starts keyboard navigation at the current model", async () => {
    const onChange = vi.fn();
    render(<ModelSelector value="model-beta" models={models} onChange={onChange} />);
    fireEvent.click(screen.getByRole("combobox", { name: "选择模型" }));
    const search = await screen.findByRole("combobox", { name: "搜索模型" });
    await waitFor(() =>
      expect(screen.getByRole("option", { name: /model-beta/ }).getAttribute("data-selected")).toBe("true"),
    );
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("model-beta");
  });

  it("opens, filters, and selects a model", async () => {
    const onChange = vi.fn();
    render(<ModelSelector value="model-alpha" models={models} onChange={onChange} />);

    fireEvent.click(screen.getByRole("combobox", { name: "选择模型" }));
    const search = await screen.findByRole("combobox", { name: "搜索模型" });
    fireEvent.change(search, { target: { value: "beta" } });

    expect(screen.queryByRole("option", { name: /model-alpha/ })).toBeNull();
    fireEvent.click(screen.getByRole("option", { name: /model-beta/ }));

    expect(onChange).toHaveBeenCalledWith("model-beta");
    await waitFor(() => expect(screen.queryByRole("combobox", { name: "搜索模型" })).toBeNull());
  });

  it("accepts a custom model id with Enter", async () => {
    const onChange = vi.fn();
    render(<ModelSelector value="" models={models} onChange={onChange} placeholder="选择" />);

    fireEvent.click(screen.getByRole("combobox", { name: "选择" }));
    const search = await screen.findByRole("combobox", { name: "搜索模型" });
    fireEvent.change(search, { target: { value: "custom-model" } });
    fireEvent.keyDown(search, { key: "Enter" });

    expect(onChange).toHaveBeenCalledWith("custom-model");
  });

  it("uses a matching catalog model when Enter follows a partial name", async () => {
    const onChange = vi.fn();
    render(<ModelSelector value="" models={models} onChange={onChange} />);
    fireEvent.click(screen.getByRole("combobox", { name: "选择模型" }));
    const search = await screen.findByRole("combobox", { name: "搜索模型" });
    fireEvent.change(search, { target: { value: "beta" } });
    const beta = screen.getByRole("option", { name: /model-beta/ });
    await waitFor(() => expect(beta.getAttribute("data-selected")).toBe("true"));
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("model-beta");
    expect(onChange).not.toHaveBeenCalledWith("beta");
  });

  it("does not commit a model while an IME composition is confirmed", async () => {
    const onChange = vi.fn();
    render(<ModelSelector value="" models={models} onChange={onChange} />);
    fireEvent.click(screen.getByRole("combobox", { name: "选择模型" }));
    const search = await screen.findByRole("combobox", { name: "搜索模型" });
    fireEvent.change(search, { target: { value: "custom" } });
    fireEvent.keyDown(search, { key: "Enter", isComposing: true, keyCode: 229 });
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("combobox", { name: "搜索模型" })).toBeTruthy();
  });

  it("clears the current model", () => {
    const onChange = vi.fn();
    render(<ModelSelector value="model-alpha" models={models} onChange={onChange} />);

    fireEvent.click(screen.getByRole("button", { name: "清除模型" }));
    expect(onChange).toHaveBeenCalledWith("");
  });

  it("falls back to a text input when no catalog is available", () => {
    const onChange = vi.fn();
    render(<ModelSelector value="custom" models={[]} onChange={onChange} placeholder="模型 ID" />);

    fireEvent.change(screen.getByPlaceholderText("模型 ID"), { target: { value: "custom-next" } });
    expect(onChange).toHaveBeenCalledWith("custom-next");
  });
});
