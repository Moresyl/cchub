/* eslint-disable @typescript-eslint/no-explicit-any */
export type StructuredConfigTool =
  | "claude"
  | "codex"
  | "gemini"
  | "grokbuild"
  | "openclaw"
  | "opencode"
  | "hermes"
  | "pi";
export type ClaudeAuthField = "ANTHROPIC_AUTH_TOKEN" | "ANTHROPIC_API_KEY";
export type ApiFormat = "anthropic" | "openai_chat" | "openai_responses";
export type OpenClawApiProtocol =
  | "openai-completions"
  | "openai-responses"
  | "anthropic-messages"
  | "google-generative-ai"
  | "bedrock-converse-stream";
export type CodexReasoningEffort = "low" | "medium" | "high" | "xhigh";
export type CodexWireApi = "responses" | "chat";
export type OpenCodeNpmPackage =
  | "@ai-sdk/openai"
  | "@ai-sdk/openai-compatible"
  | "@ai-sdk/anthropic"
  | "@ai-sdk/amazon-bedrock"
  | "@ai-sdk/google";
export type OpenCodeThinkingLevel = "minimal" | "low" | "medium" | "high";
export type OpenCodeReasoningEffort = "low" | "medium" | "high" | "xhigh" | "max";
export type PresetProviderType = "github_copilot" | "google_oauth" | "codex_oauth" | "xai_oauth";

export interface TemplateValueConfig {
  label: string;
  placeholder: string;
  defaultValue?: string;
}

export interface ModelCost {
  input?: number;
  output?: number;
}

export interface OpenClawModelCatalogEntry {
  alias?: string;
}

export interface OpenClawSuggestedDefaults {
  primary?: string;
  fallbacks?: string[];
}

export interface ConfigPreset {
  id: string;
  toolId: StructuredConfigTool;
  name: string;
  baseUrl: string;
  model: string;
  authField?: ClaudeAuthField;
  category?: string;
  badge?: string;
  featured?: boolean;
  apiProtocol?: OpenClawApiProtocol;
  npm?: OpenCodeNpmPackage;
  websiteUrl?: string;
  apiKeyUrl?: string;
  endpointCandidates?: string[];
  costMultiplier?: string;
  templateValues?: Record<string, TemplateValueConfig>;
  requiresOAuth?: boolean;
  apiFormat?: ApiFormat;
  providerType?: PresetProviderType;
  codexWireApi?: CodexWireApi;
  codexReasoningEffort?: CodexReasoningEffort;
  openClawContextWindow?: string;
  openClawCostInput?: string;
  openClawCostOutput?: string;
  suggestedPrimaryModel?: string;
  suggestedFallbackModels?: string;
  modelCatalogAlias?: string;
  openCodeContextLimit?: string;
  openCodeOutputLimit?: string;
  openCodeInputModalities?: string;
  openCodeOutputModalities?: string;
  openCodeVariantName?: string;
  openCodeIncludeThoughts?: boolean;
  openCodeThinkingBudget?: string;
  openCodeThinkingLevel?: OpenCodeThinkingLevel | "";
  openCodeReasoningEffort?: OpenCodeReasoningEffort | "";
  openCodeEffort?: OpenCodeReasoningEffort | "";
  hermesProvider?: string;
  hermesApiKeyEnv?: string;
}

export interface StructuredDraftFields {
  presetId: string;
  baseUrl: string;
  useFullUrl: boolean;
  iconUrl: string;
  apiKey: string;
  model: string;
  reasoningModel: string;
  haikuModel: string;
  sonnetModel: string;
  opusModel: string;
  authField: ClaudeAuthField;
  apiFormat: ApiFormat;
  apiProtocol: OpenClawApiProtocol;
  modelName: string;
  npm: OpenCodeNpmPackage;
  websiteUrl: string;
  apiKeyUrl: string;
  category: string;
  endpointCandidates: string;
  customEndpoints: string[];
  customUserAgent: string;
  requestHeaders: Record<string, string>;
  requestHeaderOverrides: string;
  requestBodyOverrides: string;
  costMultiplier: string;
  templateValues: string;
  requiresOAuth: boolean;
  providerType: PresetProviderType | "";
  oauthAccountId: string;
  hideAttribution: boolean;
  effortHigh: boolean;
  enableTeammates: boolean;
  codexWireApi: CodexWireApi;
  codexReasoningEffort: CodexReasoningEffort;
  openClawContextWindow: string;
  openClawCostInput: string;
  openClawCostOutput: string;
  suggestedPrimaryModel: string;
  suggestedFallbackModels: string;
  modelCatalogAlias: string;
  openCodeContextLimit: string;
  openCodeOutputLimit: string;
  openCodeInputModalities: string;
  openCodeOutputModalities: string;
  openCodeVariantName: string;
  openCodeIncludeThoughts: boolean;
  openCodeThinkingBudget: string;
  openCodeThinkingLevel: OpenCodeThinkingLevel | "";
  openCodeReasoningEffort: OpenCodeReasoningEffort | "";
  openCodeEffort: OpenCodeReasoningEffort | "";
  hermesProvider: string;
  hermesApiKeyEnv: string;
  /** Imported usage scripts are preserved while editing structured fields. */
  usageScript?: Record<string, unknown>;
}
