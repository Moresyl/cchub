import type { ModelInfo } from "../modelCatalog";
import { buildStructuredConfig } from "./builder";
import { parseStructuredConfig } from "./parser";
import type { StructuredDraftFields } from "./types";

const OPEN_CODE_MODEL_FIELDS = [
  "modelName",
  "openCodeContextLimit",
  "openCodeOutputLimit",
  "openCodeInputModalities",
  "openCodeOutputModalities",
  "openCodeVariantName",
  "openCodeIncludeThoughts",
  "openCodeThinkingBudget",
  "openCodeThinkingLevel",
  "openCodeReasoningEffort",
  "openCodeEffort",
] as const;

/** Save the previous model's edits and restore only the next model's settings. */
export function selectProfileModel(
  tool: string,
  fields: StructuredDraftFields,
  id: string,
): Partial<StructuredDraftFields> {
  if (tool !== "opencode" || id === fields.model) return { model: id };
  const source = JSON.parse(buildStructuredConfig(tool, fields));
  source.metadata.nativeModelId = id;
  const selected = parseStructuredConfig(tool, JSON.stringify(source));
  return {
    model: id,
    openCodeSource: source,
    ...Object.fromEntries(OPEN_CODE_MODEL_FIELDS.map((field) => [field, selected[field]])),
  };
}

/** This explicit action writes only native fields supported by the target tool. */
export function discoveredModelFields(tool: string, model: ModelInfo): Partial<StructuredDraftFields> {
  const result: Partial<StructuredDraftFields> = {};
  if (tool === "opencode") {
    if (model.contextWindow) result.openCodeContextLimit = String(model.contextWindow);
    if (model.maxOutputTokens) result.openCodeOutputLimit = String(model.maxOutputTokens);
    if (model.inputModalities) result.openCodeInputModalities = model.inputModalities.join(",");
    if (model.outputModalities) result.openCodeOutputModalities = model.outputModalities.join(",");
    if (model.displayName) result.modelName = model.displayName;
  } else if (tool === "openclaw") {
    if (model.contextWindow) result.openClawContextWindow = String(model.contextWindow);
    if (model.displayName) result.modelName = model.displayName;
  }
  return result;
}
