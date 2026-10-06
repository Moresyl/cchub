import { act, cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import MarkdownPreview from "./MarkdownPreview";
import { setLocale } from "../lib/i18n";
import "./markdown-preview/MarkdownPreviewImpl";

beforeEach(() => setLocale("zh"));
afterEach(cleanup);

describe("Markdown preview", () => {
  it("renders nested lists, disabled labelled tasks and an accessible table", async () => {
    render(
      <MarkdownPreview
        content={
          "# Instructions\n\n- ordinary\n  - nested\n\n1. first\n2. second\n\n- [x] 保留 **配置**\n  - 子项\n- [ ] 检查结果\n\n| Field | Value |\n|---|---|\n| key | retained |"
        }
      />,
    );
    await act(() => import("./markdown-preview/MarkdownPreviewImpl"));
    expect(await screen.findByRole("heading", { name: "Instructions" })).toBeTruthy();
    const done = screen.getByRole("checkbox", { name: "保留 配置" });
    expect(done.getAttribute("aria-checked")).toBe("true");
    expect(done.hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("checkbox", { name: "检查结果" }).getAttribute("aria-checked")).toBe("false");
    const region = screen.getByRole("region", { name: "表格，可横向滚动" });
    expect(region.tabIndex).toBe(0);
    expect(within(region).getByRole("table")).toBeTruthy();
    expect(screen.getAllByRole("list")).toHaveLength(5);
  });

  it("preserves caller link handling and table overrides", async () => {
    render(
      <MarkdownPreview
        content={"[Guide](https://example.test)\n\n| Field | Value |\n|---|---|\n| key | retained |"}
        components={{
          a: ({ children }) => <span>{children}</span>,
          table: ({ children }) => <table aria-label="Custom table">{children}</table>,
        }}
      />,
    );
    await act(() => import("./markdown-preview/MarkdownPreviewImpl"));
    expect(await screen.findByText("Guide")).toBeTruthy();
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.getByRole("table", { name: "Custom table" })).toBeTruthy();
  });

  it("keeps code literal and ignores executable HTML and unsafe link targets", async () => {
    const { container } = render(
      <MarkdownPreview
        content={'```json\n{"retained": true}\n```\n\n<script>alert(1)</script>\n\n[Unsafe](javascript:alert%281%29)'}
      />,
    );
    await act(() => import("./markdown-preview/MarkdownPreviewImpl"));
    expect(await screen.findByText('{"retained": true}')).toBeTruthy();
    expect(container.querySelector("pre code")?.textContent).toContain('"retained"');
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector('a[href^="javascript:"]')).toBeNull();
  });
});
