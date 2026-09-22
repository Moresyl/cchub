import { afterEach, describe, expect, it } from "vitest";
import { clampSidebarWidth, DEFAULT_SIDEBAR_WIDTH, readSidebarWidth } from "./sidebarWidth";

describe("sidebar width", () => {
  afterEach(() => window.localStorage.removeItem("cchub:sidebar-width"));

  it("clamps dragged and keyboard-adjusted widths", () => {
    expect(clampSidebarWidth(200)).toBe(264);
    expect(clampSidebarWidth(280.6)).toBe(281);
    expect(clampSidebarWidth(500)).toBe(360);
    expect(clampSidebarWidth(Number.NaN)).toBe(DEFAULT_SIDEBAR_WIDTH);
  });

  it("restores a saved width and ignores invalid values", () => {
    expect(readSidebarWidth()).toBe(DEFAULT_SIDEBAR_WIDTH);
    window.localStorage.setItem("cchub:sidebar-width", "300");
    expect(readSidebarWidth()).toBe(300);
    window.localStorage.setItem("cchub:sidebar-width", "invalid");
    expect(readSidebarWidth()).toBe(DEFAULT_SIDEBAR_WIDTH);
  });
});
