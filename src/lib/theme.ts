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

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const apply = () => {
    root.setAttribute("data-theme", theme);
  };

  const startViewTransition = (
    document as Document & {
      startViewTransition?: (callback: () => void) => void;
    }
  ).startViewTransition?.bind(document);
  if (!startViewTransition || root.getAttribute("data-theme") === theme) {
    apply();
    return;
  }

  startViewTransition(() => {
    apply();
  });
}

export function initTheme() {
  applyTheme(getTheme());
}
