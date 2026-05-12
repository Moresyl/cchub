/* eslint-disable @typescript-eslint/no-explicit-any */
import type {
  ApiFormat,
  CodexReasoningEffort,
  CodexWireApi,
  ModelCost,
  OpenClawApiProtocol,
  OpenClawModelCatalogEntry,
  OpenClawSuggestedDefaults,
  OpenCodeNpmPackage,
  OpenCodeReasoningEffort,
  OpenCodeThinkingLevel,
  StructuredDraftFields,
} from "./types";
import { createDefaultStructuredFields, findTomlValue, parseBooleanLike, stringifyTemplateValues } from "./helpers";

export function parseStructuredConfig(toolId: string, content: string): StructuredDraftFields {
  const defaults = createDefaultStructuredFields(toolId);

  try {
    const parsed = JSON.parse(content) as Record<string, any>;
    const metadata = (parsed.metadata || {}) as Record<string, any>;

    if (toolId === "claude") {
      const env = (parsed.env || {}) as Record<string, string>;
      const authField = env.ANTHROPIC_API_KEY ? "ANTHROPIC_API_KEY" : "ANTHROPIC_AUTH_TOKEN";
      const apiFormat = (env.ANTHROPIC_API_FORMAT as ApiFormat) || "anthropic";
      return {
        ...defaults,
        baseUrl: env.ANTHROPIC_BASE_URL || defaults.baseUrl,
        apiKey: env[authField] || "",
        model: env.ANTHROPIC_MODEL || defaults.model,
        reasoningModel: env.ANTHROPIC_REASONING_MODEL || env.ANTHROPIC_MODEL || defaults.model,
        haikuModel: env.ANTHROPIC_DEFAULT_HAIKU_MODEL || env.ANTHROPIC_MODEL || defaults.model,
        sonnetModel: env.ANTHROPIC_DEFAULT_SONNET_MODEL || env.ANTHROPIC_MODEL || defaults.model,
        opusModel: env.ANTHROPIC_DEFAULT_OPUS_MODEL || env.ANTHROPIC_MODEL || defaults.model,
        authField,
        apiFormat,
        hideAttribution: parsed.attribution?.commit === "" && parsed.attribution?.pr === "",
        effortHigh: parsed.effortLevel === "high",
        enableTeammates:
          env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS === "1" || (env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS as any) === 1,
        websiteUrl: metadata.websiteUrl || defaults.websiteUrl,
        apiKeyUrl: metadata.apiKeyUrl || defaults.apiKeyUrl,
        category: metadata.category || defaults.category,
        endpointCandidates: Array.isArray(metadata.endpointCandidates)
          ? metadata.endpointCandidates.join("\n")
          : defaults.endpointCandidates,
        costMultiplier:
          metadata.costMultiplier !== undefined ? String(metadata.costMultiplier) : defaults.costMultiplier,
        useFullUrl: parseBooleanLike(metadata.useFullUrl),
        iconUrl: metadata.iconUrl || defaults.iconUrl,
        templateValues: stringifyTemplateValues(metadata.templateValues),
        requiresOAuth: Boolean(metadata.requiresOAuth),
        providerType: metadata.providerType || defaults.providerType,
        oauthAccountId:
          (metadata.authBinding?.authProvider === "github_copilot" ? metadata.authBinding?.accountId : undefined) ||
          metadata.githubAccountId ||
          "",
      };
    }

    if (toolId === "codex") {
      const auth = (parsed.auth || {}) as Record<string, string>;
      const config = typeof parsed.config === "string" ? parsed.config : "";
      return {
        ...defaults,
        apiKey: auth.OPENAI_API_KEY || "",
        baseUrl: findTomlValue(config, "base_url") || defaults.baseUrl,
        model: findTomlValue(config, "model") || defaults.model,
        codexReasoningEffort:
          (findTomlValue(config, "model_reasoning_effort") as CodexReasoningEffort) || defaults.codexReasoningEffort,
        codexWireApi: (findTomlValue(config, "wire_api") as CodexWireApi) || defaults.codexWireApi,
        websiteUrl: metadata.websiteUrl || defaults.websiteUrl,
        apiKeyUrl: metadata.apiKeyUrl || defaults.apiKeyUrl,
        category: metadata.category || defaults.category,
        endpointCandidates: Array.isArray(metadata.endpointCandidates)
          ? metadata.endpointCandidates.join("\n")
          : defaults.endpointCandidates,
        costMultiplier:
          metadata.costMultiplier !== undefined ? String(metadata.costMultiplier) : defaults.costMultiplier,
        useFullUrl: parseBooleanLike(metadata.useFullUrl),
        iconUrl: metadata.iconUrl || defaults.iconUrl,
      };
    }

    if (toolId === "openclaw") {
      const baseUrl = (parsed.baseUrl as string) || "";
      const apiKey = (parsed.apiKey as string) || "";
      const api = (parsed.api as OpenClawApiProtocol) || "openai-completions";
      const models = Array.isArray(parsed.models) ? parsed.models : [];
      const firstModel = models[0] as
        | { id?: string; name?: string; contextWindow?: number; cost?: ModelCost }
        | undefined;
      const modelCatalog = (parsed.modelCatalog || {}) as Record<string, OpenClawModelCatalogEntry>;
      const suggestedDefaults = (parsed.suggestedDefaults || {}) as OpenClawSuggestedDefaults;
      return {
        ...defaults,
        baseUrl,
        apiKey,
        model: firstModel?.id || "",
        modelName: firstModel?.name || "",
        apiProtocol: api,
        websiteUrl: metadata.websiteUrl || defaults.websiteUrl,
        apiKeyUrl: metadata.apiKeyUrl || defaults.apiKeyUrl,
        category: metadata.category || defaults.category,
        endpointCandidates: Array.isArray(metadata.endpointCandidates)
          ? metadata.endpointCandidates.join("\n")
          : defaults.endpointCandidates,
        costMultiplier:
          metadata.costMultiplier !== undefined ? String(metadata.costMultiplier) : defaults.costMultiplier,
        useFullUrl: parseBooleanLike(metadata.useFullUrl),
        iconUrl: metadata.iconUrl || defaults.iconUrl,
        openClawContextWindow: firstModel?.contextWindow ? String(firstModel.contextWindow) : "",
        openClawCostInput: firstModel?.cost?.input !== undefined ? String(firstModel.cost.input) : "",
        openClawCostOutput: firstModel?.cost?.output !== undefined ? String(firstModel.cost.output) : "",
        suggestedPrimaryModel: suggestedDefaults.primary || "",
        suggestedFallbackModels: Array.isArray(suggestedDefaults.fallbacks)
          ? suggestedDefaults.fallbacks.join(", ")
          : "",
        modelCatalogAlias: (firstModel?.id && modelCatalog[firstModel.id]?.alias) || "",
      };
    }

    if (toolId === "hermes") {
      const config = (parsed.config || {}) as Record<string, any>;
      const modelConfig = (config.model || {}) as Record<string, string>;
      const env = (parsed.env || {}) as Record<string, string>;
      const hermesApiKeyEnv = (parsed.metadata?.hermesApiKeyEnv as string) || Object.keys(env)[0] || "";
      return {
        ...defaults,
        baseUrl: modelConfig.base_url || defaults.baseUrl,
        apiKey: hermesApiKeyEnv ? env[hermesApiKeyEnv] || "" : "",
        model: modelConfig.default || defaults.model,
        websiteUrl: metadata.websiteUrl || defaults.websiteUrl,
        apiKeyUrl: metadata.apiKeyUrl || defaults.apiKeyUrl,
        category: metadata.category || defaults.category,
        endpointCandidates: Array.isArray(metadata.endpointCandidates)
          ? metadata.endpointCandidates.join("\n")
          : defaults.endpointCandidates,
        costMultiplier:
          metadata.costMultiplier !== undefined ? String(metadata.costMultiplier) : defaults.costMultiplier,
        useFullUrl: parseBooleanLike(metadata.useFullUrl),
        iconUrl: metadata.iconUrl || defaults.iconUrl,
        hermesProvider: modelConfig.provider || metadata.hermesProvider || defaults.hermesProvider,
        hermesApiKeyEnv,
      };
    }

    if (toolId === "opencode") {
      const npm = (parsed.npm as OpenCodeNpmPackage) || "@ai-sdk/openai-compatible";
      const options = (parsed.options || {}) as Record<string, string>;
      const modelsObj = (parsed.models || {}) as Record<string, Record<string, any>>;
      const modelEntries = Object.entries(modelsObj);
      const firstEntry = modelEntries[0];
      const firstModel = firstEntry?.[1] || {};
      const variants = (firstModel.variants || {}) as Record<string, Record<string, any>>;
      const firstVariantName = Object.keys(variants)[0] || "";
      const firstVariant = variants[firstVariantName] || {};
      const thinkingConfig = (firstVariant.thinkingConfig || firstVariant.thinking || {}) as Record<string, any>;
      const modalities = (firstModel.modalities || {}) as { input?: string[]; output?: string[] };
      return {
        ...defaults,
        npm,
        baseUrl: options.baseURL || "",
        apiKey: options.apiKey || "",
        model: firstEntry?.[0] || "",
        modelName: firstModel.name || "",
        websiteUrl: metadata.websiteUrl || defaults.websiteUrl,
        apiKeyUrl: metadata.apiKeyUrl || defaults.apiKeyUrl,
        category: metadata.category || defaults.category,
        endpointCandidates: Array.isArray(metadata.endpointCandidates)
          ? metadata.endpointCandidates.join("\n")
          : defaults.endpointCandidates,
        costMultiplier:
          metadata.costMultiplier !== undefined ? String(metadata.costMultiplier) : defaults.costMultiplier,
        useFullUrl: parseBooleanLike(metadata.useFullUrl),
        iconUrl: metadata.iconUrl || defaults.iconUrl,
        openCodeContextLimit: firstModel.contextLimit !== undefined ? String(firstModel.contextLimit) : "",
        openCodeOutputLimit: firstModel.outputLimit !== undefined ? String(firstModel.outputLimit) : "",
        openCodeInputModalities: Array.isArray(modalities.input) ? modalities.input.join(",") : "",
        openCodeOutputModalities: Array.isArray(modalities.output) ? modalities.output.join(",") : "",
        openCodeVariantName: firstVariantName,
        openCodeIncludeThoughts: parseBooleanLike(thinkingConfig.includeThoughts),
        openCodeThinkingBudget:
          thinkingConfig.thinkingBudget !== undefined
            ? String(thinkingConfig.thinkingBudget)
            : thinkingConfig.budgetTokens !== undefined
              ? String(thinkingConfig.budgetTokens)
              : "",
        openCodeThinkingLevel: (thinkingConfig.thinkingLevel as OpenCodeThinkingLevel) || "",
        openCodeReasoningEffort: (firstVariant.reasoningEffort as OpenCodeReasoningEffort) || "",
        openCodeEffort: (firstVariant.effort as OpenCodeReasoningEffort) || "",
      };
    }

    const env = (parsed.env || {}) as Record<string, string>;
    return {
      ...defaults,
      baseUrl: env.GOOGLE_GEMINI_BASE_URL || defaults.baseUrl,
      apiKey: env.GEMINI_API_KEY || "",
      model: env.GEMINI_MODEL || defaults.model,
      websiteUrl: metadata.websiteUrl || defaults.websiteUrl,
      apiKeyUrl: metadata.apiKeyUrl || defaults.apiKeyUrl,
      category: metadata.category || defaults.category,
      endpointCandidates: Array.isArray(metadata.endpointCandidates)
        ? metadata.endpointCandidates.join("\n")
        : defaults.endpointCandidates,
      costMultiplier: metadata.costMultiplier !== undefined ? String(metadata.costMultiplier) : defaults.costMultiplier,
      useFullUrl: parseBooleanLike(metadata.useFullUrl),
      iconUrl: metadata.iconUrl || defaults.iconUrl,
      requiresOAuth: Boolean(metadata.requiresOAuth),
      providerType: metadata.providerType || defaults.providerType,
      oauthAccountId:
        (metadata.authBinding?.authProvider === "github_copilot" ? metadata.authBinding?.accountId : undefined) ||
        metadata.githubAccountId ||
        "",
    };
  } catch {
    return defaults;
  }
}
