import html from "../../index.html?raw";
import { afterEach, expect, it, vi } from "vitest";

afterEach(() => {
  localStorage.clear();
  vi.resetModules();
});

const startup = html.match(/<script>([\s\S]*?)<\/script>/)![1];

it.each([
  [null, null, "dark"],
  ["light", "dark", "dark"],
  ["dark", "light", "light"],
  [null, "invalid", "dark"],
  [null, "system", "light"],
  ["light", null, "light"],
])("keeps startup and stored themes consistent (legacy %s, stored %s)", async (legacy, saved, expected) => {
  localStorage.clear();
  vi.resetModules();
  if (legacy) localStorage.setItem("cchub-theme", legacy);
  if (saved) localStorage.setItem("cchub-prefs", JSON.stringify({ state: { theme: saved }, version: 0 }));
  let initial = html.match(/data-theme="([^"]+)"/)![1];
  const runStartup = new Function("localStorage", "matchMedia", "document", startup);
  runStartup(localStorage, () => ({ matches: true }), {
    documentElement: {
      setAttribute: (_key: string, value: string) => {
        initial = value;
      },
    },
  });
  const { getPreferenceTheme } = await import("./preferences");
  const preference = getPreferenceTheme();
  expect(initial).toBe(expected);
  expect(preference === "system" ? "light" : preference).toBe(expected);
});
