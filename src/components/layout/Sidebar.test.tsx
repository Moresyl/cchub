import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar";

describe("Sidebar", () => {
  beforeEach(() => vi.stubGlobal("__APP_VERSION__", "1.5.0"));
  afterEach(() => vi.unstubAllGlobals());

  it("keeps navigation available when collapsed", () => {
    const onToggle = vi.fn();
    const { container } = render(
      <MemoryRouter>
        <Sidebar collapsed width={264} onResize={vi.fn()} onToggle={onToggle} />
      </MemoryRouter>,
    );

    expect(container.querySelector(".sidebar-shell")?.classList.contains("sidebar-collapsed")).toBe(true);
    expect(screen.getByRole("link", { name: "配置切换" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "设置" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "展开侧栏" }));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });
});
