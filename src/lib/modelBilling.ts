export interface ModelBilling {
  kind: "free" | "premium" | "unknown";
  multiplier: number | null;
}

export function normalizeModelBilling(value: unknown): ModelBilling {
  const unknown: ModelBilling = { kind: "unknown", multiplier: null };
  if (!value || typeof value !== "object") return unknown;
  const billing = value as Partial<ModelBilling>;
  const multiplier = billing.multiplier;
  if (billing.kind === "premium" && multiplier === null) return { kind: "premium", multiplier: null };
  if (typeof multiplier !== "number" || !Number.isFinite(multiplier)) return unknown;
  if (billing.kind === "free" && multiplier === 0) return { kind: "free", multiplier: 0 };
  if (billing.kind === "premium" && multiplier > 0) return { kind: "premium", multiplier };
  return unknown;
}

export function agreeModelBilling(left: unknown, right: unknown): ModelBilling {
  const previous = normalizeModelBilling(left);
  const next = normalizeModelBilling(right);
  if (previous.kind === next.kind && previous.multiplier === next.multiplier) return previous;
  if (previous.kind === "premium" && next.kind === "premium") return { kind: "premium", multiplier: null };
  return { kind: "unknown", multiplier: null };
}
