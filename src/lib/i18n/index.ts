import { getPreferenceLocale, setPreferenceLocale } from "../../stores/preferences";
import type { Locale } from "../../types/preferences";
import { zh, type I18n } from "./zh";
import { en } from "./en";
import { ja } from "./ja";

export type { I18n, Locale };

const locales: Record<Locale, I18n> = { zh, en, ja };

export function setLocale(locale: Locale) {
  setPreferenceLocale(locale);
}

export function getLocale(): Locale {
  return getPreferenceLocale();
}

export function t(): I18n {
  return locales[getLocale()];
}

export function tReplace(str: string, params: Record<string, string | number>): string {
  let result = str;
  for (const [key, value] of Object.entries(params)) {
    result = result.replace(`{${key}}`, String(value));
  }
  return result;
}
