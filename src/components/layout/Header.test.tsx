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
        <Header />
      </MemoryRouter>,
    );

    expect(screen.getByRole("button", { name: /主题.*浅色|Theme.*Light/ })).toBeTruthy();
    act(() => setTheme("light"));
    expect(screen.getByRole("button", { name: /主题.*深色|Theme.*Dark/ })).toBeTruthy();
  });
});
