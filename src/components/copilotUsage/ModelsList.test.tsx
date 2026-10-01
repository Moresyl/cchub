import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import ModelsList from "./ModelsList";
import type { CopilotModel } from "../../lib/copilotAccounts";

vi.mock("../ui/simple-select", () => ({
  SimpleSelect: ({
    value,
    options,
    onValueChange,
    ariaLabel,
  }: {
    value: string;
    options: { value: string; label: string }[];
    onValueChange: (value: string) => void;
    ariaLabel: string;
  }) => (
    <select aria-label={ariaLabel} value={value} onChange={(event) => onValueChange(event.target.value)}>
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));
const text = (zh: string) => zh;
const models: CopilotModel[] = [
  { id: "free", name: "Base", vendor: "Vendor", billing: { kind: "free", multiplier: 0 } },
  { id: "paid", name: "Premium Model", vendor: "Vendor", billing: { kind: "premium", multiplier: 0.33 } },
  { id: "varying", name: "Varying", vendor: "Another", billing: { kind: "premium", multiplier: null } },
  { id: "old", name: "Legacy", vendor: "Vendor" },
];
afterEach(cleanup);
describe("account model billing list", () => {
  it("combines billing filters with name, ID and vendor search", () => {
    render(<ModelsList models={models} id="models" text={text} />);
    expect(screen.getByRole("status").textContent).toBe("显示 4 / 4 个模型");
    const filter = screen.getByRole("combobox", { name: "模型计费筛选" });
    fireEvent.change(filter, { target: { value: "premium" } });
    const list = screen.getByRole("list", { name: "Copilot 模型列表" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    expect(within(list).getByText("高级请求 ×0.33")).toBeTruthy();
    expect(within(list).getByText("高级请求 · 倍率未确定")).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox", { name: "搜索 Copilot 模型" }), { target: { value: " ANOTHER " } });
    expect(screen.getByRole("status").textContent).toBe("显示 1 / 4 个模型");
    expect(screen.getByText("Varying")).toBeTruthy();
    expect(screen.queryByText("Premium Model")).toBeNull();
  });
  it("never promotes legacy or malformed billing to a free model", () => {
    render(
      <ModelsList
        models={[...models, { id: "bad", name: "Bad", vendor: "", billing: { kind: "free", multiplier: -1 } }]}
        id="models"
        text={text}
      />,
    );
    fireEvent.change(screen.getByRole("combobox", { name: "模型计费筛选" }), { target: { value: "unknown" } });
    expect(screen.getByRole("status").textContent).toBe("显示 2 / 5 个模型");
    expect(screen.getByText("Legacy")).toBeTruthy();
    expect(screen.getByText("Bad")).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox", { name: "搜索 Copilot 模型" }), { target: { value: "not-found" } });
    expect(screen.queryByRole("list")).toBeNull();
    expect(screen.getByText("没有符合条件的模型")).toBeTruthy();
  });
});
