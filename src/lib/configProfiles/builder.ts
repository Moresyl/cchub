/* eslint-disable @typescript-eslint/no-explicit-any */
import type { StructuredDraftFields } from "./types";
import { parseNumberLike, parseTemplateValues, splitList } from "./helpers";

export function buildStructuredConfig(toolId: string, fields: StructuredDraftFields): string {
  if (toolId === "claude") {
    const env: Record<string, string | number> = {};
    if (fields.apiKey.trim() && !fields.requiresOAuth) {
      env[fields.authField] = fields.apiKey.trim();
    }
    if (fields.baseUrl.trim()) {
      env.ANTHROPIC_BASE_URL = fields.baseUrl.trim();
    }
    if (fields.model.trim()) {
      env.ANTHROPIC_MODEL = fields.model.trim();
    }
    if (fields.reasoningModel.trim()) {
      env.ANTHROPIC_REASONING_MODEL = fields.reasoningModel.trim();
    }
    if (fields.haikuModel.trim()) {
      env.ANTHROPIC_DEFAULT_HAIKU_MODEL = fields.haikuModel.trim();
    }
    if (fields.sonnetModel.trim()) {
      env.ANTHROPIC_DEFAULT_SONNET_MODEL = fields.sonnetModel.trim();
    }
    if (fields.opusModel.trim()) {
      env.ANTHROPIC_DEFAULT_OPUS_MODEL = fields.opusModel.trim();
    }
    if (fields.apiFormat && fields.apiFormat !== "anthropic") {
      env.ANTHROPIC_API_FORMAT = fields.apiFormat;
    }
    if (fields.enableTeammates) {
      env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS = "1";
    }
    const result: Record<string, any> = {
      env,
      includeCoAuthoredBy: false,
      metadata: {
        category: fields.category,
        websiteUrl: fields.websiteUrl,
        apiKeyUrl: fields.apiKeyUrl,
        endpointCandidates: splitList(fields.endpointCandidates.replace(/\n/g, ",")),
        costMultiplier: fields.costMultiplier.trim() || undefined,
        useFullUrl: fields.useFullUrl || undefined,
        iconUrl: fields.iconUrl.trim() || undefined,
        templateValues: parseTemplateValues(fields.templateValues),
        requiresOAuth: fields.requiresOAuth,
        providerType: fields.providerType || undefined,
        authBinding:
          fields.providerType === "github_copilot"
            ? {
                source: "managed_account",
                authProvider: "github_copilot",
                accountId: fields.oauthAccountId.trim() || undefined,
              }
            : undefined,
        githubAccountId:
          fields.providerType === "github_copilot" && fields.oauthAccountId.trim()
            ? fields.oauthAccountId.trim()
            : undefined,
      },
    };
    if (fields.hideAttribution) {
      result.attribution = { commit: "", pr: "" };
    }
    if (fields.effortHigh) {
      result.effortLevel = "high";
    }
    return JSON.stringify(result, null, 2);
  }

  if (toolId === "codex") {
    const providerName = "custom";
    const config = [
      `model_provider = "${providerName}"`,
      `model = "${fields.model.trim() || "gpt-5.3-codex"}"`,
      `model_reasoning_effort = "${fields.codexReasoningEffort}"`,
      "disable_response_storage = true",
      "",
      `[model_providers.${providerName}]`,
      `name = "${providerName}"`,
      `base_url = "${fields.baseUrl.trim()}"`,
      `wire_api = "${fields.codexWireApi}"`,
      "requires_openai_auth = true",
    ].join("\n");

    return JSON.stringify(
      {
        auth: {
          OPENAI_API_KEY: fields.apiKey.trim(),
        },
        config,
        metadata: {
          category: fields.category,
          websiteUrl: fields.websiteUrl,
          apiKeyUrl: fields.apiKeyUrl,
          endpointCandidates: splitList(fields.endpointCandidates.replace(/\n/g, ",")),
          costMultiplier: fields.costMultiplier.trim() || undefined,
          useFullUrl: fields.useFullUrl || undefined,
          iconUrl: fields.iconUrl.trim() || undefined,
        },
      },
      null,
      2,
    );
  }

  if (toolId === "openclaw") {
    const model: Record<string, unknown> = {
      id: fields.model.trim(),
      name: fields.modelName.trim() || fields.model.trim(),
    };
    const contextWindow = parseNumberLike(fields.openClawContextWindow);
    if (contextWindow !== undefined) {
      model.contextWindow = contextWindow;
    }
    const inputCost = parseNumberLike(fields.openClawCostInput);
    const outputCost = parseNumberLike(fields.openClawCostOutput);
    if (inputCost !== undefined || outputCost !== undefined) {
      model.cost = { input: inputCost, output: outputCost };
    }
    const modelCatalog =
      fields.modelCatalogAlias.trim() && fields.model.trim()
        ? { [fields.model.trim()]: { alias: fields.modelCatalogAlias.trim() } }
        : undefined;
    const suggestedDefaults =
      fields.suggestedPrimaryModel.trim() || fields.suggestedFallbackModels.trim()
        ? {
            primary: fields.suggestedPrimaryModel.trim() || undefined,
            fallbacks: splitList(fields.suggestedFallbackModels),
          }
        : undefined;

    return JSON.stringify(
      {
        baseUrl: fields.baseUrl.trim(),
        apiKey: fields.apiKey.trim(),
        api: fields.apiProtocol || "openai-completions",
        models: fields.model.trim() ? [model] : [],
        modelCatalog,
        suggestedDefaults,
        metadata: {
          category: fields.category,
          websiteUrl: fields.websiteUrl,
          apiKeyUrl: fields.apiKeyUrl,
          endpointCandidates: splitList(fields.endpointCandidates.replace(/\n/g, ",")),
          costMultiplier: fields.costMultiplier.trim() || undefined,
          useFullUrl: fields.useFullUrl || undefined,
          iconUrl: fields.iconUrl.trim() || undefined,
        },
      },
      null,
      2,
    );
  }

  if (toolId === "hermes") {
    const provider = fields.hermesProvider.trim() || "custom";
    const envKey = fields.hermesApiKeyEnv.trim();
    const env: Record<string, string> = {};
    if (envKey && fields.apiKey.trim()) {
      env[envKey] = fields.apiKey.trim();
    }

    return JSON.stringify(
      {
        config: {
          model: {
            provider,
            default: fields.model.trim(),
            base_url: fields.baseUrl.trim(),
          },
        },
        env,
        metadata: {
          category: fields.category,
          websiteUrl: fields.websiteUrl,
          apiKeyUrl: fields.apiKeyUrl,
          endpointCandidates: splitList(fields.endpointCandidates.replace(/\n/g, ",")),
          costMultiplier: fields.costMultiplier.trim() || undefined,
          useFullUrl: fields.useFullUrl || undefined,
          iconUrl: fields.iconUrl.trim() || undefined,
          hermesProvider: provider,
          hermesApiKeyEnv: envKey || undefined,
        },
      },
      null,
      2,
    );
  }

  if (toolId === "opencode") {
    const modelEntry: Record<string, unknown> = {
      name: fields.modelName.trim() || fields.model.trim(),
    };
    const contextLimit = parseNumberLike(fields.openCodeContextLimit);
    if (contextLimit !== undefined) modelEntry.contextLimit = contextLimit;
    const outputLimit = parseNumberLike(fields.openCodeOutputLimit);
    if (outputLimit !== undefined) modelEntry.outputLimit = outputLimit;
    const inputModalities = splitList(fields.openCodeInputModalities);
    const outputModalities = splitList(fields.openCodeOutputModalities);
    if (inputModalities.length || outputModalities.length) {
      modelEntry.modalities = {
        input: inputModalities,
        output: outputModalities,
      };
    }
    if (fields.openCodeVariantName.trim()) {
      const variantConfig: Record<string, unknown> = {};
      if (fields.openCodeIncludeThoughts || fields.openCodeThinkingBudget.trim() || fields.openCodeThinkingLevel) {
        variantConfig.thinkingConfig = {
          ...(fields.openCodeIncludeThoughts ? { includeThoughts: true } : {}),
          ...(fields.openCodeThinkingBudget.trim()
            ? { thinkingBudget: parseNumberLike(fields.openCodeThinkingBudget) ?? fields.openCodeThinkingBudget.trim() }
            : {}),
          ...(fields.openCodeThinkingLevel ? { thinkingLevel: fields.openCodeThinkingLevel } : {}),
        };
      }
      if (fields.openCodeReasoningEffort) {
        variantConfig.reasoningEffort = fields.openCodeReasoningEffort;
      }
      if (fields.openCodeEffort) {
        variantConfig.effort = fields.openCodeEffort;
      }
      if (Object.keys(variantConfig).length > 0) {
        modelEntry.variants = {
          [fields.openCodeVariantName.trim()]: variantConfig,
        };
      }
    }

    return JSON.stringify(
      {
        npm: fields.npm.trim() || "@ai-sdk/openai-compatible",
        name: "custom",
        metadata: {
          category: fields.category,
          websiteUrl: fields.websiteUrl,
          apiKeyUrl: fields.apiKeyUrl,
          endpointCandidates: splitList(fields.endpointCandidates.replace(/\n/g, ",")),
          costMultiplier: fields.costMultiplier.trim() || undefined,
          useFullUrl: fields.useFullUrl || undefined,
          iconUrl: fields.iconUrl.trim() || undefined,
        },
        options: {
          baseURL: fields.baseUrl.trim(),
          apiKey: fields.apiKey.trim(),
        },
        models: fields.model.trim()
          ? {
              [fields.model.trim()]: modelEntry,
            }
          : {},
      },
      null,
      2,
    );
  }

  return JSON.stringify(
    {
      env: {
        GOOGLE_GEMINI_BASE_URL: fields.baseUrl.trim(),
        ...(fields.requiresOAuth ? {} : { GEMINI_API_KEY: fields.apiKey.trim() }),
        GEMINI_MODEL: fields.model.trim() || "gemini-2.5-pro",
      },
      metadata: {
        category: fields.category,
        websiteUrl: fields.websiteUrl,
        apiKeyUrl: fields.apiKeyUrl,
        endpointCandidates: splitList(fields.endpointCandidates.replace(/\n/g, ",")),
        costMultiplier: fields.costMultiplier.trim() || undefined,
        useFullUrl: fields.useFullUrl || undefined,
        iconUrl: fields.iconUrl.trim() || undefined,
        requiresOAuth: fields.requiresOAuth,
        providerType: fields.providerType || undefined,
      },
      config: {},
    },
    null,
    2,
  );
}
