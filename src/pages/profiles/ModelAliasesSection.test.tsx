import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { createDefaultStructuredFields, type StructuredDraftFields } from "../../lib/configProfiles";
import { mergeDraftFields } from "./draftMerge";
import { ModelAliasesSection } from "./ModelAliasesSection";

function Harness({ initial }: { initial?: Partial<StructuredDraftFields> }) {
  const [fields, setFields] = useState({ ...createDefaultStructuredFields("claude"), model: "core", ...initial });
  return (
    <ModelAliasesSection
      fields={fields}
      localeText={(zh) => zh}
      onChange={(next) => setFields((current) => mergeDraftFields(current, next))}
    />
  );
}

describe("model alias editor", () => {
  it("focuses a new row, validates while typing, previews and removes it", () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "添加别名" }));
    const model = screen.getByLabelText("请求模型 1");
    expect(document.activeElement).toBe(model);
    fireEvent.change(model, { target: { value: "*" } });
    expect(screen.getByRole("alert").textContent).toContain("完整填写第 1 条");
    fireEvent.change(screen.getByLabelText("供应商模型 1"), { target: { value: "vendor/*" } });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText("core → vendor/core")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "删除别名 1" }));
    expect(screen.queryByLabelText("请求模型 1")).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "添加别名" }));
  });

  it("keeps malformed data until the explicit clear action", () => {
    render(<Harness initial={{ localProxyModelAliasesRaw: { bad: true } }} />);
    expect(screen.getByRole("alert").textContent).toContain("已保留原始数据");
    expect(screen.queryByRole("button", { name: "添加别名" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "清空损坏的别名" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("button", { name: "添加别名" })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "添加别名" }));
  });

  it("marks the duplicate row and retains entered values", () => {
    render(
      <Harness
        initial={{
          localProxyModelAliases: [
            { model: "*", upstream: "a/*" },
            { model: " * ", upstream: "b/*" },
          ],
        }}
      />,
    );
    expect(screen.getByRole("alert").textContent).toContain("第 2 条请求模型重复");
    expect(screen.getByLabelText("请求模型 2").getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByLabelText("供应商模型 2")).toHaveProperty("value", "b/*");
  });
});
