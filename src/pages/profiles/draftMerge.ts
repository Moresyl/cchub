/* eslint-disable @typescript-eslint/no-explicit-any */
import { type StructuredDraftFields } from "../../lib/configProfiles";

export type DraftFieldsStateUpdate =
  | StructuredDraftFields
  | ((current: StructuredDraftFields) => StructuredDraftFields);

export function mergeSharedDraftFields(
  current: StructuredDraftFields,
  toolId: string,
  parsed: StructuredDraftFields,
  includeCommon: boolean,
  includeToolSpecific = true,
): StructuredDraftFields {
  const next = { ...current };

  if (includeCommon) {
    next.baseUrl = parsed.baseUrl || next.baseUrl;
    next.useFullUrl = parsed.useFullUrl;
    next.apiKey = parsed.apiKey || next.apiKey;
    next.model = parsed.model || next.model;
    next.websiteUrl = parsed.websiteUrl || next.websiteUrl;
    next.apiKeyUrl = parsed.apiKeyUrl || next.apiKeyUrl;
    next.category = parsed.category || next.category;
    next.endpointCandidates = parsed.endpointCandidates || next.endpointCandidates;
    next.customEndpoints = parsed.customEndpoints.length ? parsed.customEndpoints : next.customEndpoints;
    next.customUserAgent = parsed.customUserAgent || next.customUserAgent;
    next.requestHeaders = Object.keys(parsed.requestHeaders).length ? parsed.requestHeaders : next.requestHeaders;
    next.requestHeaderOverrides = parsed.requestHeaderOverrides || next.requestHeaderOverrides;
    next.requestBodyOverrides = parsed.requestBodyOverrides || next.requestBodyOverrides;
    next.costMultiplier = parsed.costMultiplier || next.costMultiplier;
    next.requiresOAuth = parsed.requiresOAuth || next.requiresOAuth;
    next.providerType = parsed.providerType || next.providerType;
    next.oauthAccountId = parsed.oauthAccountId || next.oauthAccountId;
  }

  if (!includeToolSpecific) {
    return next;
  }

  if (toolId === "claude") {
    next.reasoningModel = parsed.reasoningModel;
    next.haikuModel = parsed.haikuModel;
    next.sonnetModel = parsed.sonnetModel;
    next.opusModel = parsed.opusModel;
    next.authField = parsed.authField;
    next.apiFormat = parsed.apiFormat;
    next.hideAttribution = parsed.hideAttribution;
    next.effortHigh = parsed.effortHigh;
    next.enableTeammates = parsed.enableTeammates;
  } else if (toolId === "codex") {
    next.codexWireApi = parsed.codexWireApi;
    next.codexReasoningEffort = parsed.codexReasoningEffort;
  } else if (toolId === "openclaw") {
    next.apiProtocol = parsed.apiProtocol;
    next.modelName = parsed.modelName;
    next.openClawContextWindow = parsed.openClawContextWindow;
    next.openClawCostInput = parsed.openClawCostInput;
    next.openClawCostOutput = parsed.openClawCostOutput;
    next.suggestedPrimaryModel = parsed.suggestedPrimaryModel;
    next.suggestedFallbackModels = parsed.suggestedFallbackModels;
    next.modelCatalogAlias = parsed.modelCatalogAlias;
  } else if (toolId === "opencode") {
    next.npm = parsed.npm;
    next.modelName = parsed.modelName || next.modelName;
    next.openCodeContextLimit = parsed.openCodeContextLimit;
    next.openCodeOutputLimit = parsed.openCodeOutputLimit;
    next.openCodeInputModalities = parsed.openCodeInputModalities;
    next.openCodeOutputModalities = parsed.openCodeOutputModalities;
    next.openCodeVariantName = parsed.openCodeVariantName;
    next.openCodeIncludeThoughts = parsed.openCodeIncludeThoughts;
    next.openCodeThinkingBudget = parsed.openCodeThinkingBudget;
    next.openCodeThinkingLevel = parsed.openCodeThinkingLevel;
    next.openCodeReasoningEffort = parsed.openCodeReasoningEffort;
    next.openCodeEffort = parsed.openCodeEffort;
  }

  return next;
}

export function mergeDraftFields(
  current: StructuredDraftFields,
  next: Partial<StructuredDraftFields> = {},
): StructuredDraftFields {
  const merged = { ...current } as StructuredDraftFields;
  for (const [key, value] of Object.entries(next) as [
    keyof StructuredDraftFields,
    StructuredDraftFields[keyof StructuredDraftFields] | undefined,
  ][]) {
    if (value !== undefined) {
      (merged as unknown as Record<string, unknown>)[key] = value;
    }
  }
  return merged;
}
