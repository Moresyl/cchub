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
import {
  createDefaultStructuredFields,
  findTomlValue,
  normalizeCustomUserAgent,
  normalizeEndpointList,
  normalizeRequestHeaders,
  parseBooleanLike,
  stringifyTemplateValues,
} from "./helpers";

function stringifyOverrideObject(value: unknown, headers = false): string {
  if (!value || typeof value !== "object" || Array.isArray(value)) return "";
  const normalized = headers ? normalizeRequestHeaders(value) : { ...(value as Record<string, unknown>) };
  if (!headers) delete normalized.stream;
  return Object.keys(normalized).length ? JSON.stringify(normalized, null, 2) : "";
}

export function parseStructuredConfig(toolId: string, content: string): StructuredDraftFields {
  const defaults = createDefaultStructuredFields(toolId);

  try {
    const parsed = JSON.parse(content) as Record<string, any>;
    const metadata = (parsed.metadata || {}) as Record<string, any>;
    const usageScript =
      metadata.usageScript && typeof metadata.usageScript === "object" && !Array.isArray(metadata.usageScript)
        ? (metadata.usageScript as Record<string, unknown>)
        : undefined;
    const customEndpoints = normalizeEndpointList(
      parsed.customEndpoints ?? parsed.custom_endpoints ?? metadata.customEndpoints,
    );
    const transportFields = {
      customUserAgent: normalizeCustomUserAgent(
        metadata.customUserAgent ?? metadata.custom_user_agent ?? parsed.customUserAgent ?? parsed.custom_user_agent,
      ),
      requestHeaders: normalizeRequestHeaders(
        metadata.requestHeaders ?? metadata.request_headers ?? parsed.requestHeaders ?? parsed.request_headers,
      ),
      requestHeaderOverrides: stringifyOverrideObject(
        (metadata.localProxyRequestOverrides ?? metadata.local_proxy_request_overrides)?.headers,
        true,
      ),
      requestBodyOverrides: stringifyOverrideObject(
        (metadata.localProxyRequestOverrides ?? metadata.local_proxy_request_overrides)?.body,
      ),
    };

    if (toolId === "claude") {
      const env = (parsed.env || {}) as Record<string, string>;
      const authField = env.ANTHROPIC_API_KEY ? "ANTHROPIC_API_KEY" : "ANTHROPIC_AUTH_TOKEN";
      const apiFormat = (env.ANTHROPIC_API_FORMAT as ApiFormat) || "anthropic";
      return {
        ...defaults,
        customEndpoints,
        ...transportFields,
        usageScript,
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
          (["github_copilot", "codex_oauth", "xai_oauth"].includes(metadata.authBinding?.authProvider)
            ? metadata.authBinding?.accountId
            : undefined) ||
          metadata.githubAccountId ||
          "",
      };
    }

    if (toolId === "codex") {
      const auth = (parsed.auth || {}) as Record<string, string>;
      const config = typeof parsed.config === "string" ? parsed.config : "";
      return {
        ...defaults,
        customEndpoints,
        ...transportFields,
        usageScript,
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
        customEndpoints,
        ...transportFields,
        usageScript,
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
        customEndpoints,
        ...transportFields,
        usageScript,
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

    if (toolId === "grokbuild") {
      const config = typeof parsed.config === "string" ? parsed.config : "";
      return {
        ...defaults,
        customEndpoints,
        ...transportFields,
        usageScript,
        apiKey: findTomlValue(config, "api_key"),
        baseUrl: findTomlValue(config, "base_url") || defaults.baseUrl,
        model: findTomlValue(config, "model") || defaults.model,
        modelName: findTomlValue(config, "name") || "Grok",
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

    if (toolId === "pi") {
      const providers = (parsed.providers || {}) as Record<string, Record<string, any>>;
      const firstEntry = Object.entries(providers)[0];
      const provider = firstEntry?.[1] || {};
      const firstModel = (Array.isArray(provider.models) ? provider.models[0] : {}) as Record<string, any>;
      return {
        ...defaults,
        customEndpoints,
        ...transportFields,
        usageScript,
        baseUrl: provider.baseUrl || "",
        apiKey: provider.apiKey || "",
        model: firstModel.id || "",
        modelName: provider.name || firstEntry?.[0] || "",
        apiProtocol: provider.api || defaults.apiProtocol,
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
        customEndpoints,
        ...transportFields,
        usageScript,
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
      customEndpoints,
      ...transportFields,
      usageScript,
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
        (["github_copilot", "codex_oauth", "xai_oauth"].includes(metadata.authBinding?.authProvider)
          ? metadata.authBinding?.accountId
          : undefined) ||
        metadata.githubAccountId ||
        "",
    };
  } catch {
    return defaults;
  }
}
