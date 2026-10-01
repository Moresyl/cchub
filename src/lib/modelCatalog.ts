import { agreeModelBilling, normalizeModelBilling, type ModelBilling } from "./modelBilling";

export interface ModelInfo {
  id: string;
  displayName?: string | null;
  contextWindow?: number | null;
  maxOutputTokens?: number | null;
  inputPrice?: string | null;
  outputPrice?: string | null;
  nativeEndpoints?: string[] | null;
  supportedReasoningLevels?: string[] | null;
  inputModalities?: string[] | null;
  outputModalities?: string[] | null;
  premiumRequestBilling?: ModelBilling | null;
}

export interface SavedModelCatalog {
  toolId: string;
  models: ModelInfo[];
}

export function normalizeModelCatalog(value: unknown): SavedModelCatalog | undefined {
  if (!value || typeof value !== "object") return undefined;
  const catalog = value as Partial<SavedModelCatalog>;
  if (typeof catalog.toolId !== "string" || !Array.isArray(catalog.models) || catalog.models.length > 10_000)
    return undefined;
  const unique = new Map<string, ModelInfo>();
  for (const row of catalog.models) {
    if (!row || typeof row.id !== "string" || !row.id.trim()) continue;
    const model: ModelInfo = { ...unique.get(row.id.trim()), id: row.id.trim() };
    for (const key of ["displayName", "inputPrice", "outputPrice"] as const) {
      if (typeof row[key] === "string" && row[key].trim()) model[key] = row[key].trim();
    }
    for (const key of ["contextWindow", "maxOutputTokens"] as const) {
      if (Number.isSafeInteger(row[key]) && (row[key] ?? 0) > 0) model[key] = row[key];
    }
    for (const key of ["nativeEndpoints", "supportedReasoningLevels", "inputModalities", "outputModalities"] as const) {
      const list = row[key];
      if (Array.isArray(list))
        model[key] = [
          ...new Set(list.filter((item) => typeof item === "string" && item.trim()).map((item) => item.trim())),
        ];
    }
    if (row.premiumRequestBilling !== undefined && row.premiumRequestBilling !== null) {
      model.premiumRequestBilling = model.premiumRequestBilling
        ? agreeModelBilling(model.premiumRequestBilling, row.premiumRequestBilling)
        : normalizeModelBilling(row.premiumRequestBilling);
    }
    unique.set(model.id, model);
  }
  return { toolId: catalog.toolId, models: [...unique.values()] };
}
