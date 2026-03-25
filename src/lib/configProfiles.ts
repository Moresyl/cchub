export type StructuredConfigTool = "claude" | "codex" | "gemini" | "openclaw" | "opencode";
export type ClaudeAuthField = "ANTHROPIC_AUTH_TOKEN" | "ANTHROPIC_API_KEY";
export type ApiFormat = "anthropic" | "openai_chat" | "openai_responses";
export type OpenClawApiProtocol = "openai-completions" | "openai-responses" | "anthropic-messages" | "google-generative-ai" | "bedrock-converse-stream";
export type CodexReasoningEffort = "low" | "medium" | "high" | "xhigh";
export type CodexWireApi = "responses" | "chat";
export type OpenCodeNpmPackage = "@ai-sdk/openai" | "@ai-sdk/openai-compatible" | "@ai-sdk/anthropic" | "@ai-sdk/amazon-bedrock" | "@ai-sdk/google";
export type OpenCodeThinkingLevel = "minimal" | "low" | "medium" | "high";
export type OpenCodeReasoningEffort = "low" | "medium" | "high" | "xhigh" | "max";
export type PresetProviderType = "github_copilot" | "google_oauth";

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
}

export interface StructuredDraftFields {
  presetId: string;
  baseUrl: string;
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
  templateValues: string;
  requiresOAuth: boolean;
  providerType: PresetProviderType | "";
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
}

const PRESETS: Record<StructuredConfigTool, ConfigPreset[]> = {
  claude: [
    // === 官方 ===
    { id: "claude-official", toolId: "claude", name: "Claude Official", websiteUrl: "https://www.anthropic.com/claude-code", category: "official", badge: "官方", featured: true, baseUrl: "", model: "", authField: "ANTHROPIC_AUTH_TOKEN" },
    // === 国产官方 ===
    { id: "claude-deepseek", toolId: "claude", name: "DeepSeek", websiteUrl: "https://platform.deepseek.com", category: "cn_official", badge: "国产", featured: true, baseUrl: "https://api.deepseek.com/anthropic", model: "DeepSeek-V3.2", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-zhipu", toolId: "claude", name: "Zhipu GLM", websiteUrl: "https://open.bigmodel.cn", apiKeyUrl: "https://www.bigmodel.cn/claude-code", category: "cn_official", badge: "国产", baseUrl: "https://open.bigmodel.cn/api/anthropic", model: "glm-5", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-zhipu-en", toolId: "claude", name: "Zhipu GLM en", websiteUrl: "https://z.ai", apiKeyUrl: "https://z.ai/subscribe", category: "cn_official", badge: "国产", baseUrl: "https://api.z.ai/api/anthropic", model: "glm-5", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-bailian", toolId: "claude", name: "Bailian", websiteUrl: "https://bailian.console.aliyun.com", category: "cn_official", badge: "国产", baseUrl: "https://dashscope.aliyuncs.com/apps/anthropic", model: "", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-bailian-coding", toolId: "claude", name: "Bailian For Coding", websiteUrl: "https://bailian.console.aliyun.com", category: "cn_official", badge: "国产", baseUrl: "https://coding.dashscope.aliyuncs.com/apps/anthropic", model: "", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-kimi", toolId: "claude", name: "Kimi", websiteUrl: "https://platform.moonshot.cn/console", category: "cn_official", badge: "国产", baseUrl: "https://api.moonshot.cn/anthropic", model: "kimi-k2.5", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-kimi-coding", toolId: "claude", name: "Kimi For Coding", websiteUrl: "https://www.kimi.com/coding/docs/", category: "cn_official", badge: "国产", baseUrl: "https://api.kimi.com/coding/", model: "", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-stepfun", toolId: "claude", name: "StepFun", websiteUrl: "https://platform.stepfun.ai", apiKeyUrl: "https://platform.stepfun.ai/interface-key", category: "cn_official", badge: "国产", baseUrl: "https://api.stepfun.ai/v1", model: "step-3.5-flash", authField: "ANTHROPIC_AUTH_TOKEN", apiFormat: "openai_chat", endpointCandidates: ["https://api.stepfun.ai/v1"] },
    { id: "claude-modelscope", toolId: "claude", name: "ModelScope", websiteUrl: "https://modelscope.cn", category: "aggregator", badge: "聚合", baseUrl: "https://api-inference.modelscope.cn", model: "ZhipuAI/GLM-5", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-minimax", toolId: "claude", name: "MiniMax", websiteUrl: "https://platform.minimaxi.com", apiKeyUrl: "https://platform.minimaxi.com/subscribe/coding-plan", category: "cn_official", badge: "国产", baseUrl: "https://api.minimaxi.com/anthropic", model: "MiniMax-M2.7", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-minimax-en", toolId: "claude", name: "MiniMax en", websiteUrl: "https://platform.minimax.io", apiKeyUrl: "https://platform.minimax.io/subscribe/coding-plan", category: "cn_official", badge: "国产", baseUrl: "https://api.minimax.io/anthropic", model: "MiniMax-M2.7", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-doubaoseed", toolId: "claude", name: "DouBaoSeed", websiteUrl: "https://www.volcengine.com/product/doubao", apiKeyUrl: "https://www.volcengine.com/product/doubao", category: "cn_official", badge: "国产", baseUrl: "https://ark.cn-beijing.volces.com/api/coding", model: "doubao-seed-2-0-code-preview-latest", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-bailing", toolId: "claude", name: "BaiLing", websiteUrl: "https://alipaytbox.yuque.com/sxs0ba/ling/get_started", category: "cn_official", badge: "国产", baseUrl: "https://api.tbox.cn/api/anthropic", model: "Ling-2.5-1T", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-longcat", toolId: "claude", name: "Longcat", websiteUrl: "https://longcat.chat/platform", apiKeyUrl: "https://longcat.chat/platform/api_keys", category: "cn_official", badge: "国产", baseUrl: "https://api.longcat.chat/anthropic", model: "LongCat-Flash-Chat", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-xiaomi-mimo", toolId: "claude", name: "Xiaomi MiMo", websiteUrl: "https://platform.xiaomimimo.com", apiKeyUrl: "https://platform.xiaomimimo.com/#/console/api-keys", category: "cn_official", badge: "国产", baseUrl: "https://api.xiaomimimo.com/anthropic", model: "mimo-v2-pro", authField: "ANTHROPIC_AUTH_TOKEN" },
    // === 聚合 ===
    { id: "claude-aihubmix", toolId: "claude", name: "AiHubMix", websiteUrl: "https://aihubmix.com", apiKeyUrl: "https://aihubmix.com", category: "aggregator", badge: "聚合", baseUrl: "https://aihubmix.com", model: "", authField: "ANTHROPIC_API_KEY", endpointCandidates: ["https://aihubmix.com", "https://api.aihubmix.com"] },
    { id: "claude-siliconflow", toolId: "claude", name: "SiliconFlow", websiteUrl: "https://siliconflow.cn", apiKeyUrl: "https://cloud.siliconflow.cn", category: "aggregator", badge: "聚合", baseUrl: "https://api.siliconflow.cn", model: "Pro/MiniMaxAI/MiniMax-M2.7", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-siliconflow-en", toolId: "claude", name: "SiliconFlow en", websiteUrl: "https://siliconflow.com", category: "aggregator", badge: "聚合", baseUrl: "https://api.siliconflow.com", model: "MiniMaxAI/MiniMax-M2.7", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-dmxapi", toolId: "claude", name: "DMXAPI", websiteUrl: "https://www.dmxapi.cn", apiKeyUrl: "https://www.dmxapi.cn", category: "aggregator", badge: "聚合", baseUrl: "https://www.dmxapi.cn", model: "", authField: "ANTHROPIC_AUTH_TOKEN", endpointCandidates: ["https://www.dmxapi.cn", "https://api.dmxapi.cn"] },
    { id: "claude-openrouter", toolId: "claude", name: "OpenRouter", websiteUrl: "https://openrouter.ai", apiKeyUrl: "https://openrouter.ai/keys", category: "aggregator", badge: "聚合", baseUrl: "https://openrouter.ai/api", model: "anthropic/claude-sonnet-4.6", authField: "ANTHROPIC_AUTH_TOKEN" },
    { id: "claude-novita", toolId: "claude", name: "Novita AI", websiteUrl: "https://novita.ai", apiKeyUrl: "https://novita.ai", category: "aggregator", badge: "聚合", baseUrl: "https://api.novita.ai/anthropic", model: "zai-org/glm-5", authField: "ANTHROPIC_AUTH_TOKEN", apiFormat: "openai_chat", endpointCandidates: ["https://api.novita.ai/anthropic"] },
    { id: "claude-nvidia", toolId: "claude", name: "Nvidia", websiteUrl: "https://build.nvidia.com", apiKeyUrl: "https://build.nvidia.com/settings/api-keys", category: "aggregator", badge: "聚合", baseUrl: "https://integrate.api.nvidia.com", model: "moonshotai/kimi-k2.5", authField: "ANTHROPIC_AUTH_TOKEN", apiFormat: "openai_chat" },
    { id: "claude-compshare", toolId: "claude", name: "Compshare", websiteUrl: "https://www.compshare.cn", apiKeyUrl: "https://www.compshare.cn/coding-plan", category: "aggregator", badge: "聚合", baseUrl: "https://api.modelverse.cn", model: "", authField: "ANTHROPIC_AUTH_TOKEN", endpointCandidates: ["https://api.modelverse.cn"] },
    // === 云厂商 ===
    { id: "claude-github-copilot", toolId: "claude", name: "GitHub Copilot", websiteUrl: "https://github.com/features/copilot", category: "third_party", badge: "Copilot", baseUrl: "https://api.githubcopilot.com", model: "claude-opus-4.6", authField: "ANTHROPIC_AUTH_TOKEN", apiFormat: "openai_chat", providerType: "github_copilot", requiresOAuth: true },
    { id: "claude-aws-bedrock", toolId: "claude", name: "AWS Bedrock (AKSK)", websiteUrl: "https://aws.amazon.com/bedrock/", category: "cloud_provider", badge: "AWS", baseUrl: "https://bedrock-runtime.${AWS_REGION}.amazonaws.com", model: "global.anthropic.claude-opus-4-6-v1", authField: "ANTHROPIC_AUTH_TOKEN", templateValues: { AWS_REGION: { label: "AWS Region", placeholder: "us-west-2" }, AWS_ACCESS_KEY_ID: { label: "Access Key ID", placeholder: "AKIA..." }, AWS_SECRET_ACCESS_KEY: { label: "Secret Access Key", placeholder: "your-secret-key" } } },
    // === 自定义 ===
    { id: "claude-custom", toolId: "claude", name: "自定义", category: "custom", baseUrl: "", model: "", authField: "ANTHROPIC_AUTH_TOKEN" },
  ],
  codex: [
    // === 官方 ===
    { id: "codex-official", toolId: "codex", name: "OpenAI Official", websiteUrl: "https://chatgpt.com/codex", category: "official", badge: "官方", featured: true, baseUrl: "", model: "", codexWireApi: "responses", codexReasoningEffort: "high" },
    { id: "codex-azure", toolId: "codex", name: "Azure OpenAI", websiteUrl: "https://learn.microsoft.com/azure/ai-foundry/openai/how-to/codex", category: "third_party", badge: "Azure", featured: true, baseUrl: "https://YOUR_RESOURCE_NAME.openai.azure.com/openai", model: "gpt-5.4", codexWireApi: "responses", codexReasoningEffort: "high", endpointCandidates: ["https://YOUR_RESOURCE_NAME.openai.azure.com/openai"] },
    // === 聚合 ===
    { id: "codex-aihubmix", toolId: "codex", name: "AiHubMix", websiteUrl: "https://aihubmix.com", category: "aggregator", badge: "聚合", baseUrl: "https://aihubmix.com/v1", model: "gpt-5.4", codexWireApi: "responses", codexReasoningEffort: "high", endpointCandidates: ["https://aihubmix.com/v1", "https://api.aihubmix.com/v1"] },
    { id: "codex-dmxapi", toolId: "codex", name: "DMXAPI", websiteUrl: "https://www.dmxapi.cn", category: "aggregator", badge: "聚合", baseUrl: "https://www.dmxapi.cn/v1", model: "gpt-5.4", codexWireApi: "responses", codexReasoningEffort: "high", endpointCandidates: ["https://www.dmxapi.cn/v1"] },
    { id: "codex-compshare", toolId: "codex", name: "Compshare", websiteUrl: "https://www.compshare.cn", apiKeyUrl: "https://www.compshare.cn/coding-plan", category: "aggregator", badge: "聚合", baseUrl: "https://api.modelverse.cn/v1", model: "gpt-5.4", codexWireApi: "responses", codexReasoningEffort: "high", endpointCandidates: ["https://api.modelverse.cn/v1"] },
    { id: "codex-openrouter", toolId: "codex", name: "OpenRouter", websiteUrl: "https://openrouter.ai", apiKeyUrl: "https://openrouter.ai/keys", category: "aggregator", badge: "聚合", baseUrl: "https://openrouter.ai/api/v1", model: "gpt-5.4", codexWireApi: "responses", codexReasoningEffort: "high" },
    // === 自定义 ===
    { id: "codex-custom", toolId: "codex", name: "自定义", category: "custom", baseUrl: "", model: "gpt-5.4", codexWireApi: "responses", codexReasoningEffort: "high" },
  ],
  gemini: [
    // === 官方 ===
    { id: "gemini-official", toolId: "gemini", name: "Google Official", websiteUrl: "https://ai.google.dev/", apiKeyUrl: "https://aistudio.google.com/apikey", category: "official", badge: "OAuth", featured: true, baseUrl: "", model: "", requiresOAuth: true, providerType: "google_oauth" },
    // === 聚合 ===
    { id: "gemini-openrouter", toolId: "gemini", name: "OpenRouter", websiteUrl: "https://openrouter.ai", apiKeyUrl: "https://openrouter.ai/keys", category: "aggregator", badge: "聚合", baseUrl: "https://openrouter.ai/api", model: "gemini-3.1-pro" },
    // === 自定义 ===
    { id: "gemini-custom", toolId: "gemini", name: "自定义", category: "custom", baseUrl: "", model: "gemini-3.1-pro" },
  ],
  openclaw: [
    { id: "openclaw-deepseek", toolId: "openclaw", name: "DeepSeek", websiteUrl: "https://platform.deepseek.com", apiKeyUrl: "https://platform.deepseek.com/api_keys", category: "cn_official", badge: "国产", featured: true, baseUrl: "https://api.deepseek.com/v1", model: "deepseek-chat", apiProtocol: "openai-completions", openClawContextWindow: "64000", openClawCostInput: "0.0005", openClawCostOutput: "0.002", suggestedPrimaryModel: "deepseek/deepseek-chat", suggestedFallbackModels: "deepseek/deepseek-reasoner", modelCatalogAlias: "DeepSeek" },
    { id: "openclaw-bedrock", toolId: "openclaw", name: "AWS Bedrock", websiteUrl: "https://aws.amazon.com/bedrock/", category: "cloud_provider", badge: "AWS", featured: true, baseUrl: "https://bedrock-runtime.us-east-1.amazonaws.com", model: "global.anthropic.claude-sonnet-4-6", apiProtocol: "bedrock-converse-stream", openClawContextWindow: "1000000", suggestedPrimaryModel: "anthropic/claude-sonnet-4-6", modelCatalogAlias: "Claude Sonnet 4.6" },
    { id: "openclaw-openrouter", toolId: "openclaw", name: "OpenRouter", websiteUrl: "https://openrouter.ai", apiKeyUrl: "https://openrouter.ai/keys", category: "aggregator", badge: "聚合", baseUrl: "https://openrouter.ai/api/v1", model: "anthropic/claude-sonnet-4", apiProtocol: "openai-responses", modelCatalogAlias: "Claude Sonnet", endpointCandidates: ["https://openrouter.ai/api/v1"] },
    { id: "openclaw-custom", toolId: "openclaw", name: "自定义", category: "custom", baseUrl: "", model: "", apiProtocol: "openai-completions" },
  ],
  opencode: [
    { id: "opencode-openai", toolId: "opencode", name: "OpenAI Responses", websiteUrl: "https://platform.openai.com/", apiKeyUrl: "https://platform.openai.com/api-keys", category: "official", badge: "官方", featured: true, baseUrl: "https://api.openai.com/v1", model: "gpt-5.4", npm: "@ai-sdk/openai", openCodeContextLimit: "400000", openCodeOutputLimit: "128000", openCodeInputModalities: "text,image", openCodeOutputModalities: "text", openCodeVariantName: "high", openCodeReasoningEffort: "high" },
    { id: "opencode-gemini", toolId: "opencode", name: "Google (Gemini)", websiteUrl: "https://ai.google.dev/", apiKeyUrl: "https://aistudio.google.com/apikey", category: "official", badge: "官方", featured: true, baseUrl: "https://generativelanguage.googleapis.com/v1beta", model: "gemini-3-flash-preview", npm: "@ai-sdk/google", openCodeContextLimit: "1048576", openCodeOutputLimit: "65536", openCodeInputModalities: "text,image,pdf,video,audio", openCodeOutputModalities: "text", openCodeVariantName: "high", openCodeIncludeThoughts: true, openCodeThinkingLevel: "high" },
    { id: "opencode-anthropic", toolId: "opencode", name: "Anthropic", websiteUrl: "https://www.anthropic.com/api", apiKeyUrl: "https://console.anthropic.com/settings/keys", category: "official", badge: "官方", featured: true, baseUrl: "https://api.anthropic.com", model: "claude-opus-4-6", npm: "@ai-sdk/anthropic", openCodeContextLimit: "1000000", openCodeOutputLimit: "128000", openCodeInputModalities: "text,image,pdf", openCodeOutputModalities: "text", openCodeVariantName: "max", openCodeEffort: "max" },
    { id: "opencode-openrouter", toolId: "opencode", name: "OpenRouter", websiteUrl: "https://openrouter.ai", apiKeyUrl: "https://openrouter.ai/keys", category: "aggregator", badge: "聚合", baseUrl: "https://openrouter.ai/api/v1", model: "anthropic/claude-sonnet-4", npm: "@ai-sdk/openai-compatible", openCodeContextLimit: "200000", openCodeOutputLimit: "64000", openCodeInputModalities: "text,image", openCodeOutputModalities: "text", endpointCandidates: ["https://openrouter.ai/api/v1"] },
    { id: "opencode-custom", toolId: "opencode", name: "自定义", category: "custom", baseUrl: "", model: "", npm: "@ai-sdk/openai-compatible" },
  ],
};

function findTomlValue(content: string, key: string) {
  const pattern = new RegExp(`^\\s*${key}\\s*=\\s*"([^"]*)"`, "m");
  return content.match(pattern)?.[1] || "";
}

function parseBooleanLike(value: unknown): boolean {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    return ["1", "true", "yes", "on"].includes(value.trim().toLowerCase());
  }
  return false;
}

function parseNumberLike(value: string): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function splitList(value: string): string[] {
  return value.split(",").map((item) => item.trim()).filter(Boolean);
}

function stringifyTemplateValues(value: Record<string, TemplateValueConfig> | undefined): string {
  if (!value || Object.keys(value).length === 0) return "";
  return JSON.stringify(value, null, 2);
}

function parseTemplateValues(value: string): Record<string, TemplateValueConfig> | undefined {
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
  return toolId === "claude" || toolId === "codex" || toolId === "gemini" || toolId === "openclaw" || toolId === "opencode";
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
    templateValues: stringifyTemplateValues(preset?.templateValues),
    requiresOAuth: preset?.requiresOAuth || false,
    providerType: preset?.providerType || "",
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
      templateValues: current?.templateValues || "",
      requiresOAuth: current?.requiresOAuth || false,
      providerType: current?.providerType || "",
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
    };
  }

  const defaults = createDefaultStructuredFields(toolId);
  return {
    ...defaults,
    ...current,
    presetId,
    baseUrl: preset.baseUrl,
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
    templateValues: stringifyTemplateValues(preset.templateValues) || current?.templateValues || "",
    requiresOAuth: preset.requiresOAuth || false,
    providerType: preset.providerType || current?.providerType || "",
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
  };
}

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
        templateValues: parseTemplateValues(fields.templateValues),
        requiresOAuth: fields.requiresOAuth,
        providerType: fields.providerType || undefined,
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
    const modelCatalog = fields.modelCatalogAlias.trim() && fields.model.trim()
      ? { [fields.model.trim()]: { alias: fields.modelCatalogAlias.trim() } }
      : undefined;
    const suggestedDefaults = fields.suggestedPrimaryModel.trim() || fields.suggestedFallbackModels.trim()
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
          ...(fields.openCodeThinkingBudget.trim() ? { thinkingBudget: parseNumberLike(fields.openCodeThinkingBudget) ?? fields.openCodeThinkingBudget.trim() } : {}),
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
        requiresOAuth: fields.requiresOAuth,
        providerType: fields.providerType || undefined,
      },
      config: {},
    },
    null,
    2,
  );
}

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
        enableTeammates: env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS === "1" || env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS as any === 1,
        websiteUrl: metadata.websiteUrl || defaults.websiteUrl,
        apiKeyUrl: metadata.apiKeyUrl || defaults.apiKeyUrl,
        category: metadata.category || defaults.category,
        endpointCandidates: Array.isArray(metadata.endpointCandidates) ? metadata.endpointCandidates.join("\n") : defaults.endpointCandidates,
        templateValues: stringifyTemplateValues(metadata.templateValues),
        requiresOAuth: Boolean(metadata.requiresOAuth),
        providerType: metadata.providerType || defaults.providerType,
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
        codexReasoningEffort: (findTomlValue(config, "model_reasoning_effort") as CodexReasoningEffort) || defaults.codexReasoningEffort,
        codexWireApi: (findTomlValue(config, "wire_api") as CodexWireApi) || defaults.codexWireApi,
        websiteUrl: metadata.websiteUrl || defaults.websiteUrl,
        apiKeyUrl: metadata.apiKeyUrl || defaults.apiKeyUrl,
        category: metadata.category || defaults.category,
        endpointCandidates: Array.isArray(metadata.endpointCandidates) ? metadata.endpointCandidates.join("\n") : defaults.endpointCandidates,
      };
    }

    if (toolId === "openclaw") {
      const baseUrl = (parsed.baseUrl as string) || "";
      const apiKey = (parsed.apiKey as string) || "";
      const api = (parsed.api as OpenClawApiProtocol) || "openai-completions";
      const models = Array.isArray(parsed.models) ? parsed.models : [];
      const firstModel = models[0] as { id?: string; name?: string; contextWindow?: number; cost?: ModelCost } | undefined;
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
        openClawContextWindow: firstModel?.contextWindow ? String(firstModel.contextWindow) : "",
        openClawCostInput: firstModel?.cost?.input !== undefined ? String(firstModel.cost.input) : "",
        openClawCostOutput: firstModel?.cost?.output !== undefined ? String(firstModel.cost.output) : "",
        suggestedPrimaryModel: suggestedDefaults.primary || "",
        suggestedFallbackModels: Array.isArray(suggestedDefaults.fallbacks) ? suggestedDefaults.fallbacks.join(", ") : "",
        modelCatalogAlias: (firstModel?.id && modelCatalog[firstModel.id]?.alias) || "",
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
        openCodeContextLimit: firstModel.contextLimit !== undefined ? String(firstModel.contextLimit) : "",
        openCodeOutputLimit: firstModel.outputLimit !== undefined ? String(firstModel.outputLimit) : "",
        openCodeInputModalities: Array.isArray(modalities.input) ? modalities.input.join(",") : "",
        openCodeOutputModalities: Array.isArray(modalities.output) ? modalities.output.join(",") : "",
        openCodeVariantName: firstVariantName,
        openCodeIncludeThoughts: parseBooleanLike(thinkingConfig.includeThoughts),
        openCodeThinkingBudget: thinkingConfig.thinkingBudget !== undefined ? String(thinkingConfig.thinkingBudget) : thinkingConfig.budgetTokens !== undefined ? String(thinkingConfig.budgetTokens) : "",
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
      endpointCandidates: Array.isArray(metadata.endpointCandidates) ? metadata.endpointCandidates.join("\n") : defaults.endpointCandidates,
      requiresOAuth: Boolean(metadata.requiresOAuth),
      providerType: metadata.providerType || defaults.providerType,
    };
  } catch {
    return defaults;
  }
}
