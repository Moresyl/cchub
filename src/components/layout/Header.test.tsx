import { act, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setTheme } from "../../lib/theme";
import Header from "./Header";

vi.mock("../ProjectProfileSwitcher", () => ({ default: () => null }));
vi.mock("../AppUpdateHost", () => ({
  useAppUpdate: () => ({ updateAvailable: false, latestVersion: null, openUpdateDialog: vi.fn() }),
}));

describe("Header", () => {
  afterEach(() => setTheme("dark"));

  it("tracks theme changes made from another control", () => {
    act(() => setTheme("dark"));
    render(
      <MemoryRouter>
        <Header sidebarCollapsed={false} onToggleSidebar={vi.fn()} />
      </MemoryRouter>,
    );

    expect(screen.getByRole("button", { name: /主题.*浅色|Theme.*Light/ })).toBeTruthy();
    act(() => setTheme("light"));
    expect(screen.getByRole("button", { name: /主题.*深色|Theme.*Dark/ })).toBeTruthy();
  });

  it("keeps window navigation and sidebar controls in the shared titlebar", () => {
    const onToggleSidebar = vi.fn();
    render(
      <MemoryRouter>
        <Header sidebarCollapsed={false} onToggleSidebar={onToggleSidebar} />
      </MemoryRouter>,
    );

    expect(screen.getByRole("button", { name: "后退" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "前进" })).toBeTruthy();
    screen.getByRole("button", { name: "折叠侧栏" }).click();
    expect(onToggleSidebar).toHaveBeenCalledTimes(1);
  });
});
