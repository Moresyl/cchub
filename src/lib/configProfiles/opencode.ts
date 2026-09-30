import type { OpenCodeNpmPackage, StructuredDraftFields } from "./types";

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

export function defaultOpenCodeNpm(source?: Record<string, unknown>): OpenCodeNpmPackage {
  const id = record(source?.metadata).nativeProviderId;
  if (id === "anthropic") return "@ai-sdk/anthropic";
  if (id === "google") return "@ai-sdk/google";
  if (id === "openai") return "@ai-sdk/openai";
  return "@ai-sdk/openai-compatible";
}

/** Preserve imported SDK options, model metadata and provider extension fields. */
export function buildOpenCodeProvider(
  fields: StructuredDraftFields,
  modelEntry: Record<string, unknown>,
  metadata: Record<string, unknown>,
  customEndpoints: string[],
): Record<string, unknown> {
  const source = fields.openCodeSource;
  const options = record(source?.options);
  const models = record(source?.models);
  const model = fields.model.trim();
  const npm = fields.npm.trim() || "@ai-sdk/openai-compatible";
  const existingModel = record(models[model]);
  const selectedModel = {
    ...existingModel,
    ...modelEntry,
    ...(modelEntry.variants ? { variants: { ...record(existingModel.variants), ...record(modelEntry.variants) } } : {}),
  };
  return {
    ...source,
    // A key-only built-in provider relies on the SDK bundled with the tool.
    npm: source && !source.npm && npm === defaultOpenCodeNpm(source) ? undefined : npm,
    name: source ? source.name : "custom",
    customEndpoints,
    metadata: {
      ...record(source?.metadata),
      ...metadata,
      nativeProviderId: fields.openCodeNativeProviderId,
      nativeModelId: model,
    },
    options: {
      ...options,
      ...(fields.baseUrl.trim() || "baseURL" in options || !source ? { baseURL: fields.baseUrl.trim() } : {}),
      ...(fields.apiKey.trim() || "apiKey" in options || !source ? { apiKey: fields.apiKey.trim() } : {}),
    },
    models: model ? { ...models, [model]: selectedModel } : models,
  };
}
