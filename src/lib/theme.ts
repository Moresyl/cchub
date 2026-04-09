export type Theme = "dark" | "light";

export function getTheme(): Theme {
  const saved = localStorage.getItem("cchub-theme") as Theme | null;
  return saved === "light" ? "light" : "dark";
}

export function setTheme(theme: Theme) {
  localStorage.setItem("cchub-theme", theme);
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
