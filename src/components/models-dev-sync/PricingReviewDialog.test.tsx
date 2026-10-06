import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PricingReviewDialog from "./PricingReviewDialog";
import type { PricingReview } from "./review";
import type { SyncConfig } from "./types";
const base: SyncConfig = {
  autoSyncEnabled: false,
  includeCommonModels: true,
  selectedModelKeys: [],
  excludedCommonModelKeys: [],
  lastSyncAt: null,
  lastSyncError: null,
};
const current = (changes: Partial<PricingReview> = {}): PricingReview => ({
  id: 1,
  baseline: base,
  draft: base,
  latest: { config: base, configPath: "C:/demo/pricing.json" },
  loading: false,
  error: false,
  ...changes,
});
const props = () => ({
  text: (zh: string) => zh,
  onCancel: vi.fn(),
  onRetry: vi.fn(),
  onApply: vi.fn(() => true),
  onCloseAutoFocus: vi.fn(),
});
beforeEach(() => {
  if (!Element.prototype.scrollIntoView)
    Object.defineProperty(Element.prototype, "scrollIntoView", { value: vi.fn(), configurable: true });
});
afterEach(cleanup);
describe("pricing review dialog", () => {
  it("pages through every changed key, searches long identifiers and names comparison values", () => {
    const keys = Array.from({ length: 50 }, (_, index) => `team/model-${String(index).padStart(2, "0")}`);
    keys[49] += `-${"long".repeat(100)}`;
    const own = { ...base, selectedModelKeys: keys };
    render(<PricingReviewDialog review={current({ draft: own })} {...props()} />);
    const section = screen.getByRole("region", { name: "模型更改对照" });
    expect(within(section).getAllByRole("article")).toHaveLength(24);
    fireEvent.click(screen.getByRole("button", { name: "下一页更改" }));
    fireEvent.click(screen.getByRole("button", { name: "下一页更改" }));
    expect(within(section).getByRole("heading", { name: keys[49] })).toBeDefined();
    expect(within(section).getAllByRole("article")).toHaveLength(2);
    fireEvent.change(screen.getByRole("textbox", { name: "搜索模型更改" }), { target: { value: "model-00" } });
    expect(within(section).getAllByRole("article")).toHaveLength(1);
    expect(within(section).getByText("最初载入")).toBeDefined();
    expect(within(section).getByText("核对后的结果")).toBeDefined();
  });
  it("requires a keyboard choice for conflicts and applies the selected resolution", async () => {
    const own = { ...base, selectedModelKeys: ["team/same"] };
    const saved = { ...base, excludedCommonModelKeys: ["team/same"] };
    const callbacks = props();
    render(
      <PricingReviewDialog
        review={current({ draft: own, latest: { config: saved, configPath: "C:/demo/pricing.json" } })}
        {...callbacks}
      />,
    );
    const apply = screen.getByRole("button", { name: "应用核对结果" }) as HTMLButtonElement;
    expect(apply.disabled).toBe(true);
    fireEvent.keyDown(screen.getByRole("combobox", { name: "显示范围" }), { key: "ArrowDown" });
    fireEvent.keyDown(await screen.findByRole("option", { name: "冲突更改" }), { key: "Enter" });
    fireEvent.keyDown(screen.getByRole("combobox", { name: "team/same 保留方式" }), { key: "ArrowDown" });
    const option = await screen.findByRole("option", { name: "保留我的草稿" });
    fireEvent.keyDown(option, { key: "Enter" });
    expect(apply.disabled).toBe(false);
    expect(screen.getByRole("combobox", { name: "team/same 保留方式" })).toBeDefined();
    expect(screen.getByRole("status").textContent).toContain("0 个冲突待选择");
    fireEvent.click(apply);
    expect(callbacks.onApply).toHaveBeenCalledWith(1, { models: new Map([["team/same", "local"]]) });
  });
  it("blocks incomplete reads, exposes safe retry and keeps cancellation available", () => {
    const callbacks = props();
    const view = render(<PricingReviewDialog review={current({ latest: null, loading: true })} {...callbacks} />);
    expect((screen.getByRole("button", { name: "应用核对结果" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "返回编辑" }));
    expect(callbacks.onCancel).toHaveBeenCalledOnce();
    view.rerender(<PricingReviewDialog review={current({ latest: null, error: true })} {...callbacks} />);
    expect(screen.getByRole("alert")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "重试读取" }));
    expect(callbacks.onRetry).toHaveBeenCalledOnce();
    expect((screen.getByRole("button", { name: "应用核对结果" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
