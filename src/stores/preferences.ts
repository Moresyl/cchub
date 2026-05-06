import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { Locale, Theme } from "../types/preferences";

export interface PreferencesState {
  locale: Locale;
  theme: Theme;
  setLocale: (locale: Locale) => void;
  setTheme: (theme: Theme) => void;
}

function readLegacyLocale(): Locale {
  if (typeof localStorage === "undefined") return "zh";
  const saved = localStorage.getItem("cchub-locale");
  return saved === "en" || saved === "ja" || saved === "zh" ? saved : "zh";
}

function readLegacyTheme(): Theme {
  if (typeof localStorage === "undefined") return "dark";
  return localStorage.getItem("cchub-theme") === "light" ? "light" : "dark";
}

export const usePreferences = create<PreferencesState>()(
  persist(
    (set) => ({
      locale: readLegacyLocale(),
      theme: readLegacyTheme(),
      setLocale: (locale) => {
        localStorage.setItem("cchub-locale", locale);
        set({ locale });
      },
      setTheme: (theme) => {
        localStorage.setItem("cchub-theme", theme);
        set({ theme });
      },
    }),
    {
      name: "cchub-prefs",
      partialize: (state) => ({ locale: state.locale, theme: state.theme }),
    },
  ),
);

export function getPreferenceLocale(): Locale {
  return usePreferences.getState().locale;
}

export function setPreferenceLocale(locale: Locale) {
  usePreferences.getState().setLocale(locale);
}

export function getPreferenceTheme(): Theme {
  return usePreferences.getState().theme;
}

export function setPreferenceTheme(theme: Theme) {
  usePreferences.getState().setTheme(theme);
}
