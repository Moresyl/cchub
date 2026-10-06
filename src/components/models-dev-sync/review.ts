import { validConfig, type SyncConfig, type SyncState } from "./types";

export type ReviewChoice = "local" | "saved";
export interface ReviewChoices {
  autoSync?: ReviewChoice;
  common?: ReviewChoice;
  models: ReadonlyMap<string, ReviewChoice>;
}
export interface PricingReview {
  id: number;
  baseline: SyncConfig;
  draft: SyncConfig;
  latest: SyncState | null;
  loading: boolean;
  error: boolean;
}
export interface ModelChange {
  key: string;
  baseline: number;
  local: number;
  saved: number;
  merged: number;
  conflict: boolean;
}
const normalized = (values: string[]) => new Set(values.map((key) => key.trim()).filter(Boolean));
function selections(config: SyncConfig) {
  const selected = normalized(config.selectedModelKeys);
  const excluded = normalized(config.excludedCommonModelKeys);
  return {
    keys: new Set([...selected, ...excluded]),
    value: (key: string) => Number(selected.has(key)) + 2 * Number(excluded.has(key)),
  };
}
function mergeValue<T>(baseline: T, local: T, saved: T) {
  return local !== baseline ? local : saved;
}
export function pricingChanges(baseline: SyncConfig, local: SyncConfig, saved: SyncConfig): ModelChange[] {
  const before = selections(baseline);
  const own = selections(local);
  const latest = selections(saved);
  return [...new Set([...before.keys, ...own.keys, ...latest.keys])].sort().flatMap((key) => {
    const baseline = before.value(key);
    const local = own.value(key);
    const saved = latest.value(key);
    if (baseline === local && baseline === saved) return [];
    return [
      {
        key,
        baseline,
        local,
        saved,
        merged: mergeValue(baseline, local, saved),
        conflict: local !== baseline && saved !== baseline && local !== saved,
      },
    ];
  });
}
export function mergePricingReview(review: PricingReview, choices: ReviewChoices): SyncConfig | null {
  if (review.loading || review.error || !review.latest) return null;
  const saved = review.latest.config;
  const local = review.draft;
  const baseline = review.baseline;
  const selected = normalized(saved.selectedModelKeys);
  const excluded = normalized(saved.excludedCommonModelKeys);
  for (const change of pricingChanges(baseline, local, saved)) {
    const choice = choices.models.get(change.key);
    if (change.conflict && !choice) return null;
    const value = choice === "local" ? change.local : choice === "saved" ? change.saved : change.merged;
    if (value & 1) selected.add(change.key);
    else selected.delete(change.key);
    if (value & 2) excluded.add(change.key);
    else excluded.delete(change.key);
  }
  const next: SyncConfig = {
    ...saved,
    autoSyncEnabled:
      choices.autoSync === "local"
        ? local.autoSyncEnabled
        : choices.autoSync === "saved"
          ? saved.autoSyncEnabled
          : mergeValue(baseline.autoSyncEnabled, local.autoSyncEnabled, saved.autoSyncEnabled),
    includeCommonModels:
      choices.common === "local"
        ? local.includeCommonModels
        : choices.common === "saved"
          ? saved.includeCommonModels
          : mergeValue(baseline.includeCommonModels, local.includeCommonModels, saved.includeCommonModels),
    selectedModelKeys: [...selected].sort(),
    excludedCommonModelKeys: [...excluded].sort(),
  };
  return validConfig(next) ? next : null;
}
