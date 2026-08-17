/* eslint-disable @typescript-eslint/no-explicit-any */
import type { ConfigPreset, StructuredConfigTool, StructuredDraftFields, TemplateValueConfig } from "./types";
import { PRESETS } from "./presets";

export function findTomlValue(content: string, key: string) {
  const pattern = new RegExp(`^\\s*${key}\\s*=\\s*"([^"]*)"`, "m");
  return content.match(pattern)?.[1] || "";
}

export function parseBooleanLike(value: unknown): boolean {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    return ["1", "true", "yes", "on"].includes(value.trim().toLowerCase());
  }
  return false;
}

export function parseNumberLike(value: string): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

export function splitList(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function normalizeEndpointList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const result: string[] = [];
  for (const item of value) {
    const raw = typeof item === "string" ? item : (item as { url?: unknown })?.url;
    if (typeof raw !== "string") continue;
    const normalized = raw.trim().replace(/\/+$/, "");
    if (!normalized || result.includes(normalized)) continue;
    try {
      const parsed = new URL(normalized);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") continue;
    } catch {
      continue;
    }
    result.push(normalized);
  }
  return result;
}

const HTTP_HEADER_NAME_PATTERN = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;
const PROTECTED_REQUEST_HEADERS = new Set([
  "host",
  "connection",
  "keep-alive",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
  "content-length",
  "authorization",
  "x-api-key",
  "x-goog-api-key",
  "chatgpt-account-id",
]);

export function normalizeRequestHeaders(value: unknown): Record<string, string> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  const normalized: Record<string, string> = {};
  for (const [rawName, rawValue] of Object.entries(value as Record<string, unknown>)) {
    const name = rawName.trim();
    if (
      !HTTP_HEADER_NAME_PATTERN.test(name) ||
      name.length > 128 ||
      PROTECTED_REQUEST_HEADERS.has(name.toLowerCase()) ||
      Object.keys(normalized).length >= 64
    )
      continue;
    // eslint-disable-next-line no-control-regex
    if (typeof rawValue !== "string" || rawValue.length > 4096 || /[\x00-\x08\x0a-\x1f\x7f]/.test(rawValue)) continue;
    if (Object.keys(normalized).some((key) => key.toLowerCase() === name.toLowerCase())) continue;
    normalized[name] = rawValue;
  }
  return normalized;
}

export function normalizeCustomUserAgent(value: unknown): string {
  if (typeof value !== "string") return "";
  const trimmed = value.trim();
  // eslint-disable-next-line no-control-regex
  if (!trimmed || trimmed.length > 512 || /[\x00-\x08\x0a-\x1f\x7f]/.test(trimmed)) return "";
  return trimmed;
}

export function stringifyTemplateValues(value: Record<string, TemplateValueConfig> | undefined): string {
  if (!value || Object.keys(value).length === 0) return "";
  return JSON.stringify(value, null, 2);
}

export function parseTemplateValues(value: string): Record<string, TemplateValueConfig> | undefined {
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  try {
    const parsed = JSON.parse(trimmed) as Record<string, TemplateValueConfig>;
    return Object.keys(parsed).length ? parsed : undefined;
  } catch {
    return undefined;
  }
}

export function supportsStructuredConfig(toolId: string): toolId is StructuredConfigTool {
  return (
    toolId === "claude" ||
    toolId === "codex" ||
    toolId === "gemini" ||
    toolId === "grokbuild" ||
    toolId === "openclaw" ||
    toolId === "opencode" ||
    toolId === "hermes" ||
    toolId === "pi"
  );
}

export function getConfigPresets(toolId: string): ConfigPreset[] {
  if (!supportsStructuredConfig(toolId)) return [];
  return PRESETS[toolId];
}

export function getPresetCategories(toolId: string): { category: string; label: string; presets: ConfigPreset[] }[] {
  const presets = getConfigPresets(toolId);
  if (presets.length === 0) return [];
  const grouped = new Map<string, ConfigPreset[]>();
  for (const preset of presets) {
    const category = preset.category || "all";
    if (!grouped.has(category)) {
      grouped.set(category, []);
    }
    grouped.get(category)!.push(preset);
  }
  return [...grouped.entries()].map(([category, groupedPresets]) => ({
    category,
    label: category,
    presets: groupedPresets,
  }));
}

export function createDefaultStructuredFields(toolId: string): StructuredDraftFields {
  const preset = getConfigPresets(toolId)[0];
  const model = preset?.model || "";
  return {
    presetId: preset?.id || "custom",
    baseUrl: preset?.baseUrl || "",
    useFullUrl: false,
    iconUrl: "",
    apiKey: "",
    model,
    reasoningModel: model,
    haikuModel: model,
    sonnetModel: model,
    opusModel: model,
    authField: preset?.authField || "ANTHROPIC_AUTH_TOKEN",
    apiFormat: "anthropic",
    apiProtocol: preset?.apiProtocol || "openai-completions",
    modelName: "",
    npm: preset?.npm || "@ai-sdk/openai-compatible",
    websiteUrl: preset?.websiteUrl || "",
    apiKeyUrl: preset?.apiKeyUrl || "",
    category: preset?.category || "",
    endpointCandidates: (preset?.endpointCandidates || []).join("\n"),
    customEndpoints: [],
    customUserAgent: "",
    requestHeaders: {},
    requestHeaderOverrides: "",
    requestBodyOverrides: "",
    costMultiplier: preset?.costMultiplier || "",
    templateValues: stringifyTemplateValues(preset?.templateValues),
    requiresOAuth: preset?.requiresOAuth || false,
    providerType: preset?.providerType || "",
    oauthAccountId: "",
    hideAttribution: false,
    effortHigh: false,
    enableTeammates: false,
    codexWireApi: preset?.codexWireApi || "responses",
    codexReasoningEffort: preset?.codexReasoningEffort || "high",
    openClawContextWindow: preset?.openClawContextWindow || "",
    openClawCostInput: preset?.openClawCostInput || "",
    openClawCostOutput: preset?.openClawCostOutput || "",
    suggestedPrimaryModel: preset?.suggestedPrimaryModel || "",
    suggestedFallbackModels: preset?.suggestedFallbackModels || "",
    modelCatalogAlias: preset?.modelCatalogAlias || "",
    openCodeContextLimit: preset?.openCodeContextLimit || "",
    openCodeOutputLimit: preset?.openCodeOutputLimit || "",
    openCodeInputModalities: preset?.openCodeInputModalities || "",
    openCodeOutputModalities: preset?.openCodeOutputModalities || "",
    openCodeVariantName: preset?.openCodeVariantName || "",
    openCodeIncludeThoughts: preset?.openCodeIncludeThoughts || false,
    openCodeThinkingBudget: preset?.openCodeThinkingBudget || "",
    openCodeThinkingLevel: preset?.openCodeThinkingLevel || "",
    openCodeReasoningEffort: preset?.openCodeReasoningEffort || "",
    openCodeEffort: preset?.openCodeEffort || "",
    hermesProvider: preset?.hermesProvider || "custom",
    hermesApiKeyEnv: preset?.hermesApiKeyEnv || "",
  };
}

export function applyPresetToFields(
  toolId: string,
  presetId: string,
  current?: Partial<StructuredDraftFields>,
): StructuredDraftFields {
  const preset = getConfigPresets(toolId).find((item) => item.id === presetId);
  if (!preset) {
    return {
      ...createDefaultStructuredFields(toolId),
      presetId,
      baseUrl: current?.baseUrl || "",
      useFullUrl: current?.useFullUrl || false,
      iconUrl: current?.iconUrl || "",
      apiKey: current?.apiKey || "",
      model: current?.model || "",
      reasoningModel: current?.reasoningModel || "",
      haikuModel: current?.haikuModel || "",
      sonnetModel: current?.sonnetModel || "",
      opusModel: current?.opusModel || "",
      authField: current?.authField || "ANTHROPIC_AUTH_TOKEN",
      apiFormat: current?.apiFormat || "anthropic",
      apiProtocol: current?.apiProtocol || "openai-completions",
      modelName: current?.modelName || "",
      npm: current?.npm || "@ai-sdk/openai-compatible",
      websiteUrl: current?.websiteUrl || "",
      apiKeyUrl: current?.apiKeyUrl || "",
      category: current?.category || "",
      endpointCandidates: current?.endpointCandidates || "",
      customEndpoints: current?.customEndpoints || [],
      customUserAgent: current?.customUserAgent || "",
      requestHeaders: current?.requestHeaders || {},
      requestHeaderOverrides: current?.requestHeaderOverrides || "",
      requestBodyOverrides: current?.requestBodyOverrides || "",
      costMultiplier: current?.costMultiplier || "",
      templateValues: current?.templateValues || "",
      requiresOAuth: current?.requiresOAuth || false,
      providerType: current?.providerType || "",
      oauthAccountId: current?.oauthAccountId || "",
      hideAttribution: current?.hideAttribution || false,
      effortHigh: current?.effortHigh || false,
      enableTeammates: current?.enableTeammates || false,
      codexWireApi: current?.codexWireApi || "responses",
      codexReasoningEffort: current?.codexReasoningEffort || "high",
      openClawContextWindow: current?.openClawContextWindow || "",
      openClawCostInput: current?.openClawCostInput || "",
      openClawCostOutput: current?.openClawCostOutput || "",
      suggestedPrimaryModel: current?.suggestedPrimaryModel || "",
      suggestedFallbackModels: current?.suggestedFallbackModels || "",
      modelCatalogAlias: current?.modelCatalogAlias || "",
      openCodeContextLimit: current?.openCodeContextLimit || "",
      openCodeOutputLimit: current?.openCodeOutputLimit || "",
      openCodeInputModalities: current?.openCodeInputModalities || "",
      openCodeOutputModalities: current?.openCodeOutputModalities || "",
      openCodeVariantName: current?.openCodeVariantName || "",
      openCodeIncludeThoughts: current?.openCodeIncludeThoughts || false,
      openCodeThinkingBudget: current?.openCodeThinkingBudget || "",
      openCodeThinkingLevel: current?.openCodeThinkingLevel || "",
      openCodeReasoningEffort: current?.openCodeReasoningEffort || "",
      openCodeEffort: current?.openCodeEffort || "",
      hermesProvider: current?.hermesProvider || "custom",
      hermesApiKeyEnv: current?.hermesApiKeyEnv || "",
    };
  }

  const defaults = createDefaultStructuredFields(toolId);
  return {
    ...defaults,
    ...current,
    presetId,
    baseUrl: preset.baseUrl,
    useFullUrl: current?.useFullUrl || defaults.useFullUrl,
    iconUrl: current?.iconUrl || defaults.iconUrl,
    model: preset.model,
    reasoningModel: preset.model || current?.reasoningModel || defaults.reasoningModel,
    haikuModel: preset.model || current?.haikuModel || defaults.haikuModel,
    sonnetModel: preset.model || current?.sonnetModel || defaults.sonnetModel,
    opusModel: preset.model || current?.opusModel || defaults.opusModel,
    authField: preset.authField || current?.authField || defaults.authField,
    apiProtocol: preset.apiProtocol || current?.apiProtocol || defaults.apiProtocol,
    npm: preset.npm || current?.npm || defaults.npm,
    websiteUrl: preset.websiteUrl || current?.websiteUrl || "",
    apiKeyUrl: preset.apiKeyUrl || current?.apiKeyUrl || "",
    category: preset.category || current?.category || "",
    endpointCandidates: (preset.endpointCandidates || []).join("\n") || current?.endpointCandidates || "",
    customEndpoints: current?.customEndpoints || [],
    customUserAgent: current?.customUserAgent || "",
    requestHeaders: current?.requestHeaders || {},
    requestHeaderOverrides: current?.requestHeaderOverrides || "",
    requestBodyOverrides: current?.requestBodyOverrides || "",
    costMultiplier: preset.costMultiplier || current?.costMultiplier || "",
    templateValues: stringifyTemplateValues(preset.templateValues) || current?.templateValues || "",
    requiresOAuth: preset.requiresOAuth || false,
    providerType: preset.providerType || current?.providerType || "",
    oauthAccountId: current?.oauthAccountId || "",
    hideAttribution: current?.hideAttribution || false,
    effortHigh: current?.effortHigh || false,
    enableTeammates: current?.enableTeammates || false,
    codexWireApi: preset.codexWireApi || current?.codexWireApi || defaults.codexWireApi,
    codexReasoningEffort: preset.codexReasoningEffort || current?.codexReasoningEffort || defaults.codexReasoningEffort,
    openClawContextWindow: preset.openClawContextWindow || current?.openClawContextWindow || "",
    openClawCostInput: preset.openClawCostInput || current?.openClawCostInput || "",
    openClawCostOutput: preset.openClawCostOutput || current?.openClawCostOutput || "",
    suggestedPrimaryModel: preset.suggestedPrimaryModel || current?.suggestedPrimaryModel || "",
    suggestedFallbackModels: preset.suggestedFallbackModels || current?.suggestedFallbackModels || "",
    modelCatalogAlias: preset.modelCatalogAlias || current?.modelCatalogAlias || "",
    openCodeContextLimit: preset.openCodeContextLimit || current?.openCodeContextLimit || "",
    openCodeOutputLimit: preset.openCodeOutputLimit || current?.openCodeOutputLimit || "",
    openCodeInputModalities: preset.openCodeInputModalities || current?.openCodeInputModalities || "",
    openCodeOutputModalities: preset.openCodeOutputModalities || current?.openCodeOutputModalities || "",
    openCodeVariantName: preset.openCodeVariantName || current?.openCodeVariantName || "",
    openCodeIncludeThoughts: preset.openCodeIncludeThoughts || current?.openCodeIncludeThoughts || false,
    openCodeThinkingBudget: preset.openCodeThinkingBudget || current?.openCodeThinkingBudget || "",
    openCodeThinkingLevel: preset.openCodeThinkingLevel || current?.openCodeThinkingLevel || "",
    openCodeReasoningEffort: preset.openCodeReasoningEffort || current?.openCodeReasoningEffort || "",
    openCodeEffort: preset.openCodeEffort || current?.openCodeEffort || "",
    hermesProvider: preset.hermesProvider || current?.hermesProvider || "custom",
    hermesApiKeyEnv: preset.hermesApiKeyEnv || current?.hermesApiKeyEnv || "",
  };
}
