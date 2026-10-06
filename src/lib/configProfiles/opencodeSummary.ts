function object(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

export function openCodeSummary(profile: Record<string, unknown>) {
  const metadata = object(profile.metadata);
  const native =
    metadata.nativeFormat === "providers" || ["settings", "package", "canonical"].some((key) => key in profile);
  const models = object(profile.models);
  const selected = typeof metadata.nativeModelId === "string" ? metadata.nativeModelId : Object.keys(models)[0];
  const [id, variantId] = (selected || "").split("#");
  const model = object(models[id]);
  const variants = Array.isArray(model.variants) ? model.variants : [];
  const variant = object(variants.find((item) => object(item).id === variantId));
  const settings = native
    ? { ...object(profile.settings), ...object(model.settings), ...object(variant.settings) }
    : object(profile.options);
  return {
    baseUrl: typeof settings.baseURL === "string" ? settings.baseURL : undefined,
    model: selected || undefined,
    iconUrl: typeof metadata.iconUrl === "string" ? metadata.iconUrl : undefined,
  };
}
