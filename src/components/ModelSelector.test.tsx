import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import ModelSelector, { type ModelInfo } from "./ModelSelector";

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
