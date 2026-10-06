export interface SyncConfig {
  autoSyncEnabled: boolean;
  includeCommonModels: boolean;
  selectedModelKeys: string[];
  excludedCommonModelKeys: string[];
  lastSyncAt: number | null;
  lastSyncError: string | null;
}
export interface SyncState {
  config: SyncConfig;
  configPath: string;
}
export interface CatalogEntry {
  key: string;
  providerId: string;
  providerName: string;
  modelId: string;
  modelName: string;
  releaseDate: string;
  isCommon: boolean;
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
}
export interface SyncResult {
  skipped: boolean;
  selected: number;
  imported: number;
  changed: number;
  syncedAt: number | null;
}
export type UiText = (zh: string, en: string, ja?: string) => string;

const keys = (values: string[]) => [...new Set(values.map((value) => value.trim()).filter(Boolean))].sort();
export function samePreferences(left: SyncConfig, right: SyncConfig) {
  return (
    left.autoSyncEnabled === right.autoSyncEnabled &&
    left.includeCommonModels === right.includeCommonModels &&
    JSON.stringify(keys(left.selectedModelKeys)) === JSON.stringify(keys(right.selectedModelKeys)) &&
    JSON.stringify(keys(left.excludedCommonModelKeys)) === JSON.stringify(keys(right.excludedCommonModelKeys))
  );
}
export function isSelected(entry: CatalogEntry, config: SyncConfig) {
  return (
    config.selectedModelKeys.includes(entry.key) ||
    (config.includeCommonModels && entry.isCommon && !config.excludedCommonModelKeys.includes(entry.key))
  );
}
export function selectModel(entry: CatalogEntry, config: SyncConfig, checked: boolean): SyncConfig {
  const selected = new Set(config.selectedModelKeys);
  const excluded = new Set(config.excludedCommonModelKeys);
  if (entry.isCommon && config.includeCommonModels) {
    selected.delete(entry.key);
    if (checked) excluded.delete(entry.key);
    else excluded.add(entry.key);
  } else if (checked) selected.add(entry.key);
  else selected.delete(entry.key);
  return { ...config, selectedModelKeys: [...selected].sort(), excludedCommonModelKeys: [...excluded].sort() };
}
export function validConfig(value: unknown): value is SyncConfig {
  if (!value || typeof value !== "object") return false;
  const config = value as SyncConfig;
  const validKeys = (values: unknown) =>
    Array.isArray(values) &&
    values.length <= 16000 &&
    values.every((key) => typeof key === "string" && key.length <= 4096);
  return (
    typeof config.autoSyncEnabled === "boolean" &&
    typeof config.includeCommonModels === "boolean" &&
    validKeys(config.selectedModelKeys) &&
    validKeys(config.excludedCommonModelKeys) &&
    (config.lastSyncAt === null || (typeof config.lastSyncAt === "number" && Number.isFinite(config.lastSyncAt))) &&
    (config.lastSyncError === null || typeof config.lastSyncError === "string")
  );
}
