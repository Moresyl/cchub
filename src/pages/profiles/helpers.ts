/* eslint-disable @typescript-eslint/no-explicit-any */
import { Monitor, Code, Sparkles, Globe, Cat, Terminal } from "lucide-react";
import type {
  CodexReasoningEffort,
  CodexWireApi,
  OpenClawApiProtocol,
  OpenCodeNpmPackage,
  OpenCodeThinkingLevel,
  StructuredDraftFields,
} from "../../lib/configProfiles";

export type { StructuredDraftFields };

export interface ConfigProfile {
  id: string;
  name: string;
  tool_id: string;
  config_snapshot: string;
  sort_order: number;
  source_type?: string | null;
  source_key?: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface ProviderConfigFragment {
  id: string;
  name: string;
  targetTools: string[];
  fields: Partial<StructuredDraftFields>;
  createdAt: string;
  updatedAt: string;
}

export interface ProviderPingResult {
  profile_id: string;
  tool_id: string;
  provider_name: string;
  base_url: string | null;
  status: string;
  latency_ms: number | null;
  http_status: number | null;
  checked_at: string;
  message: string;
}

export interface ProviderStreamCheckResult {
  profile_id: string;
  tool_id: string;
  provider_name: string;
  base_url: string | null;
  status: string;
  latency_ms: number | null;
  http_status: number | null;
  checked_at: string;
  message: string;
}

export interface DetectedTool {
  id: string;
  name: string;
  installed: boolean;
}

export const TOOL_ICONS: Record<string, typeof Monitor> = {
  claude: Terminal,
  codex: Code,
  gemini: Sparkles,
  opencode: Globe,
  openclaw: Cat,
  hermes: Monitor,
};

export const OPENCLAW_PROTOCOL_OPTIONS: OpenClawApiProtocol[] = [
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
  "bedrock-converse-stream",
];

export const OPENCODE_NPM_OPTIONS: OpenCodeNpmPackage[] = [
  "@ai-sdk/openai",
  "@ai-sdk/openai-compatible",
  "@ai-sdk/anthropic",
  "@ai-sdk/amazon-bedrock",
  "@ai-sdk/google",
];

export const CODEX_REASONING_OPTIONS: CodexReasoningEffort[] = ["low", "medium", "high", "xhigh"];
export const CODEX_WIRE_API_OPTIONS: CodexWireApi[] = ["responses", "chat"];
export const THINKING_LEVEL_OPTIONS: OpenCodeThinkingLevel[] = ["minimal", "low", "medium", "high"];
export const SECTION_TITLE_STYLE = {
  fontSize: 13,
  fontWeight: 600,
  color: "var(--text-muted)",
  marginBottom: 12,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
} as const;
export const FIELD_STACK_STYLE = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
} as const;
export const SMALL_INPUT_STYLE = { fontSize: 13 } as const;
export const TWO_COLUMN_GRID_STYLE = { display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 } as const;

export function formatTime(value: string | null) {
  if (!value) return "";
  return value.replace("T", " ").slice(0, 19);
}

export function prettyJson(content: string): string {
  try {
    return JSON.stringify(JSON.parse(content), null, 2);
  } catch {
    return content;
  }
}

export function getConfigLanguage(toolId: string, content: string): "json" | "toml" {
  if (toolId === "codex") {
    try {
      JSON.parse(content);
      return "json";
    } catch {
      return "toml";
    }
  }
  return "json";
}

export function extractConfigSummary(
  toolId: string,
  content: string,
): { baseUrl?: string; model?: string; iconUrl?: string } {
  try {
    const parsed = JSON.parse(content) as Record<string, any>;
    const metadata = (parsed.metadata || {}) as Record<string, string>;
    if (toolId === "claude") {
      const env = (parsed.env || {}) as Record<string, string>;
      return {
        baseUrl: env.ANTHROPIC_BASE_URL,
        model: env.ANTHROPIC_MODEL || env.ANTHROPIC_DEFAULT_SONNET_MODEL,
        iconUrl: metadata.iconUrl,
      };
    }
    if (toolId === "gemini") {
      const env = (parsed.env || {}) as Record<string, string>;
      return {
        baseUrl: env.GOOGLE_GEMINI_BASE_URL,
        model: env.GEMINI_MODEL,
        iconUrl: metadata.iconUrl,
      };
    }
    if (toolId === "codex") {
      const config = typeof parsed.config === "string" ? parsed.config : "";
      const modelMatch = config.match(/^model\s*=\s*"([^"]*)"/m);
      const urlMatch = config.match(/^base_url\s*=\s*"([^"]*)"/m);
      return {
        baseUrl: urlMatch?.[1],
        model: modelMatch?.[1],
        iconUrl: metadata.iconUrl,
      };
    }
    if (toolId === "openclaw") {
      const models = Array.isArray(parsed.models) ? parsed.models : [];
      const firstModel = models[0] as { id?: string } | undefined;
      return {
        baseUrl: parsed.baseUrl as string | undefined,
        model: firstModel?.id,
        iconUrl: metadata.iconUrl,
      };
    }
    if (toolId === "hermes") {
      const config = (parsed.config || {}) as Record<string, any>;
      const model = (config.model || {}) as Record<string, string>;
      return {
        baseUrl: model.base_url,
        model: model.default,
        iconUrl: metadata.iconUrl,
      };
    }
    if (toolId === "opencode") {
      const options = (parsed.options || {}) as Record<string, string>;
      const modelsObj = (parsed.models || {}) as Record<string, unknown>;
      const firstModelId = Object.keys(modelsObj)[0];
      return {
        baseUrl: options.baseURL,
        model: firstModelId,
        iconUrl: metadata.iconUrl,
      };
    }
  } catch (error) {
    console.debug("Failed to extract config profile summary", error);
  }
  return {};
}

export const MODEL_FETCH_SUPPORTED_TOOLS = ["claude", "codex", "gemini", "openclaw", "opencode"] as const;

export function supportsModelFetch(toolId: string) {
  return MODEL_FETCH_SUPPORTED_TOOLS.includes(toolId as (typeof MODEL_FETCH_SUPPORTED_TOOLS)[number]);
}

export function formatModelFetchError(
  error: unknown,
  localeText: (zhText: string, enText: string, jaText?: string) => string,
) {
  const message = String(error);
  const normalized = message.toLowerCase();

  if (normalized.includes("401") || normalized.includes("403")) {
    return localeText(
      "模型拉取失败：API Key 无效或没有权限",
      "Failed to fetch models: invalid API key or insufficient permission",
      "モデル取得に失敗しました: API Key が無効か権限が不足しています",
    );
  }
  if (normalized.includes("404")) {
    return localeText(
      "模型拉取失败：当前端点不支持模型列表，请检查 Base URL 或完整端点设置",
      "Failed to fetch models: this endpoint does not expose model listing. Check the base URL or full endpoint setting.",
      "モデル取得に失敗しました: このエンドポイントはモデル一覧を提供していません。Base URL または完全なエンドポイント設定を確認してください。",
    );
  }
  if (normalized.includes("timed out") || normalized.includes("timeout")) {
    return localeText(
      "模型拉取超时：请检查网络、代理或供应商响应速度",
      "Fetching models timed out. Check your network, proxy, or provider latency.",
      "モデル取得がタイムアウトしました。ネットワーク、プロキシ、またはプロバイダーの応答速度を確認してください。",
    );
  }
  if (normalized.includes("parse")) {
    return localeText(
      "模型拉取失败：供应商返回了无法识别的响应格式",
      "Failed to fetch models: the provider returned an unrecognized response format",
      "モデル取得に失敗しました: プロバイダーが認識できないレスポンス形式を返しました",
    );
  }

  return localeText(
    `模型拉取失败：${message}`,
    `Failed to fetch models: ${message}`,
    `モデル取得に失敗しました: ${message}`,
  );
}
