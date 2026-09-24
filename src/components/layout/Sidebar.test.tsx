import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar";

describe("Sidebar", () => {
  beforeEach(() => vi.stubGlobal("__APP_VERSION__", "1.5.0"));
  afterEach(() => vi.unstubAllGlobals());

  it("keeps navigation available when collapsed", () => {
    const { container } = render(
      <MemoryRouter>
        <Sidebar collapsed width={264} onResize={vi.fn()} />
      </MemoryRouter>,
    );

    expect(container.querySelector(".sidebar-shell")?.classList.contains("sidebar-collapsed")).toBe(true);
    expect(screen.getByRole("link", { name: "配置切换" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "设置" })).toBeTruthy();
  });

  it("uses compact primary actions and switches navigation contexts", () => {
    render(
      <MemoryRouter>
        <Sidebar collapsed={false} width={264} onResize={vi.fn()} />
      </MemoryRouter>,
    );

    expect(screen.getByRole("button", { name: "新增配置" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "搜索" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "配置文件" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "技能与插件" })).toBeTruthy();

    const runtimeTab = screen.getByRole("tab", { name: "运行" });
    fireEvent.click(runtimeTab);
    expect(runtimeTab.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("link", { name: "代理增强" })).toBeTruthy();
    expect(screen.queryByRole("link", { name: "MCP 服务" })).toBeNull();
  });
});
