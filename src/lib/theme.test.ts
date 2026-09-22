import { afterEach, describe, expect, it, vi } from "vitest";
import { getResolvedTheme, initTheme, setTheme } from "./theme";

describe("theme", () => {
  afterEach(() => {
    setTheme("dark");
    vi.unstubAllGlobals();
  });

  it("follows system changes only while system theme is selected", () => {
    const listeners: Array<() => void> = [];
    let prefersLight = true;
    vi.stubGlobal("matchMedia", () => ({
      get matches() {
        return prefersLight;
      },
      addEventListener: (_event: string, listener: () => void) => listeners.push(listener),
    }));

    setTheme("system");
    initTheme();
    expect(getResolvedTheme()).toBe("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");

    prefersLight = false;
    listeners.forEach((listener) => listener());
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");

    setTheme("light");
    listeners.forEach((listener) => listener());
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });
});
