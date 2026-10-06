import { samePreferences, validConfig, type SyncConfig } from "./types";

const STORAGE_KEY = "cchub:pricing-preferences-draft";
export interface PricingDraft {
  baseline: SyncConfig;
  draft: SyncConfig;
}
let memory: PricingDraft | null = null;
export function readDraft(): PricingDraft | null {
  try {
    const raw = window.sessionStorage.getItem(STORAGE_KEY);
    if (raw) {
      const value: PricingDraft = JSON.parse(raw);
      if (validConfig(value.baseline) && validConfig(value.draft)) return value;
    }
  } catch {
    /* The in-memory draft remains available when storage is unavailable. */
  }
  return memory;
}
export function rememberDraft(value: PricingDraft | null) {
  memory = value && !samePreferences(value.baseline, value.draft) ? value : null;
  try {
    if (memory) window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(memory));
    else window.sessionStorage.removeItem(STORAGE_KEY);
  } catch {
    /* Do not turn a storage restriction into a failed settings write. */
  }
}
