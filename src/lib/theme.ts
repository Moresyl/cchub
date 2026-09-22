import { getPreferenceTheme, setPreferenceTheme } from "../stores/preferences";
import type { Theme } from "../types/preferences";

export type { Theme };

export function getTheme(): Theme {
  return getPreferenceTheme();
}

export function setTheme(theme: Theme) {
  setPreferenceTheme(theme);
  applyTheme(theme);
}

function resolveTheme(theme: Theme): "dark" | "light" {
  if (theme !== "system") return theme;
  return window.matchMedia?.("(prefers-color-scheme: light)")?.matches ? "light" : "dark";
}

export function getResolvedTheme(): "dark" | "light" {
  return resolveTheme(getTheme());
}

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const resolved = resolveTheme(theme);
  const apply = () => {
    root.setAttribute("data-theme", resolved);
  };

  const startViewTransition = (
    document as Document & {
      startViewTransition?: (callback: () => void) => void;
    }
  ).startViewTransition?.bind(document);
  if (!startViewTransition || root.getAttribute("data-theme") === resolved) {
    apply();
    return;
  }

  startViewTransition(() => {
    apply();
  });
}

export function initTheme() {
  applyTheme(getTheme());
  window.matchMedia?.("(prefers-color-scheme: light)")?.addEventListener?.("change", () => {
    if (getTheme() === "system") applyTheme("system");
  });
}
