import { invoke } from "@tauri-apps/api/core";
import {
  buildStructuredConfig,
  createDefaultStructuredFields,
  type ApiFormat,
  type ClaudeAuthField,
  type CodexReasoningEffort,
  type CodexWireApi,
  type OpenClawApiProtocol,
  type OpenCodeNpmPackage,
  type StructuredConfigTool,
} from "./configProfiles";

export type DeepLinkResource = "provider" | "prompt" | "mcp" | "skill";
export type DeepLinkApp = StructuredConfigTool;

export interface DeepLinkImportRequest {
  version: string;
  resource: DeepLinkResource | string;
  app?: DeepLinkApp;
  name?: string;
  enabled?: boolean;
  homepage?: string;
  endpoint?: string;
  apiKey?: string;
  icon?: string;
  model?: string;
  notes?: string;
  haikuModel?: string;
  sonnetModel?: string;
  opusModel?: string;
  apiFormat?: string;
  authField?: string;
  codexWireApi?: string;
  codexReasoningEffort?: string;
  apiProtocol?: string;
  npm?: string;
  content?: string;
  description?: string;
  apps?: string;
  repo?: string;
  directory?: string;
  branch?: string;
  config?: string;
  configFormat?: string;
  configUrl?: string;
  usageEnabled?: boolean;
  usageScript?: string;
  usageApiKey?: string;
  usageBaseUrl?: string;
  usageAccessToken?: string;
  usageUserId?: string;
  usageAutoInterval?: number;
}

export interface DeepLinkErrorPayload {
  url: string;
  error: string;
}

export interface DeepLinkImportFailure {
  id: string;
  error: string;
}

export interface DeepLinkMcpImportResult {
  importedCount: number;
  importedIds: string[];
  failed: DeepLinkImportFailure[];
}

export interface ParsedMcpPreviewServer {
  name: string;
  command: string;
  args: string[];
  envKeys: string[];
  transport: string;
}

const CLAUDE_AUTH_FIELDS: ClaudeAuthField[] = ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"];
const API_FORMATS: ApiFormat[] = ["anthropic", "openai_chat", "openai_responses"];
const CODEX_WIRE_APIS: CodexWireApi[] = ["responses", "chat"];
const CODEX_REASONING: CodexReasoningEffort[] = ["low", "medium", "high", "xhigh"];
const OPENCLAW_PROTOCOLS: OpenClawApiProtocol[] = [
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
  "bedrock-converse-stream",
];
const OPENCODE_NPMS: OpenCodeNpmPackage[] = [
  "@ai-sdk/openai",
  "@ai-sdk/openai-compatible",
  "@ai-sdk/anthropic",
  "@ai-sdk/amazon-bedrock",
  "@ai-sdk/google",
];

export const deeplinkApi = {
  takePendingImports: () => invoke<DeepLinkImportRequest[]>("take_pending_deeplink_imports"),
  takePendingErrors: () => invoke<DeepLinkErrorPayload[]>("take_pending_deeplink_errors"),
  mergeRequest: (request: DeepLinkImportRequest) => invoke<DeepLinkImportRequest>("merge_deeplink_request", { request }),
  importMcp: (request: DeepLinkImportRequest) => invoke<DeepLinkMcpImportResult>("import_mcp_servers_from_deeplink", { request }),
};

export function splitDeepLinkEndpoints(endpoint: string | undefined): string[] {
  return (endpoint || "")
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}

export function getPrimaryDeepLinkEndpoint(request: DeepLinkImportRequest): string {
  return splitDeepLinkEndpoints(request.endpoint)[0] || "";
}

export function maskSecret(value: string | undefined): string {
  if (!value) return "";
  if (value.length <= 6) {
    return `${value.slice(0, 2)}***`;
  }
  return `${value.slice(0, 4)}${"*".repeat(Math.min(16, Math.max(4, value.length - 4)))}`;
}

export function decodeDeepLinkText(value: string | undefined): string {
  const trimmed = value?.trim() || "";
  if (!trimmed) return "";
  if (looksLikePlainText(trimmed)) return trimmed;

  const normalized = trimmed.replace(/-/g, "+").replace(/_/g, "/");
  const padded = normalized + "=".repeat((4 - normalized.length % 4) % 4);
  try {
    const binary = atob(padded);
    const bytes = Uint8Array.from(binary, (char) => char.codePointAt(0) || 0);
    return new TextDecoder().decode(bytes);
  } catch {
    return trimmed;
  }
}

function looksLikePlainText(value: string): boolean {
  const trimmed = value.trimStart();
  return trimmed.startsWith("{")
    || trimmed.startsWith("[")
    || trimmed.startsWith("#")
    || trimmed.includes("\n")
    || trimmed.includes("\r\n")
    || trimmed.includes(" = ")
    || trimmed.includes("=\"")
    || trimmed.includes("[model_providers");
}

export function parseMcpPreviewServers(request: DeepLinkImportRequest): ParsedMcpPreviewServer[] {
  const config = decodeDeepLinkText(request.config);
  if (!config) return [];

  try {
    const parsed = JSON.parse(config) as Record<string, unknown>;
    if (parsed.mcpServers && typeof parsed.mcpServers === "object") {
      return Object.entries(parsed.mcpServers as Record<string, unknown>).flatMap(([name, value]) => {
        const server = parseMcpServer(name, value);
        return server ? [server] : [];
      });
    }

    if (typeof parsed.command === "string") {
      const server = parseMcpServer(request.name || "imported-mcp", parsed);
      return server ? [server] : [];
    }

    return Object.entries(parsed).flatMap(([name, value]) => {
      const server = parseMcpServer(name, value);
      return server ? [server] : [];
    });
  } catch {
    return [];
  }
}

function parseMcpServer(name: string, value: unknown): ParsedMcpPreviewServer | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  if (typeof record.command !== "string") return null;
  return {
    name,
    command: record.command,
    args: Array.isArray(record.args) ? record.args.filter((item): item is string => typeof item === "string") : [],
    envKeys: record.env && typeof record.env === "object" ? Object.keys(record.env as Record<string, unknown>) : [],
    transport: typeof record.type === "string" ? record.type : "stdio",
  };
}

export function buildProviderProfileFromDeepLink(request: DeepLinkImportRequest) {
  if (request.resource !== "provider" || !request.app) {
    throw new Error("Deep link resource is not a provider");
  }

  const toolId = request.app;
  const fields = createDefaultStructuredFields(toolId);
  const endpoints = splitDeepLinkEndpoints(request.endpoint);

  fields.category = "custom";
  fields.baseUrl = endpoints[0] || fields.baseUrl;
  fields.endpointCandidates = endpoints.join("\n");
  fields.apiKey = request.apiKey?.trim() || "";
  fields.model = request.model?.trim() || fields.model;
  fields.modelName = request.model?.trim() || fields.modelName;
  fields.websiteUrl = request.homepage?.trim() || fields.websiteUrl;
  fields.apiKeyUrl = "";

  if (toolId === "claude") {
    if (request.authField && CLAUDE_AUTH_FIELDS.includes(request.authField as ClaudeAuthField)) {
      fields.authField = request.authField as ClaudeAuthField;
    }
    if (request.apiFormat && API_FORMATS.includes(request.apiFormat as ApiFormat)) {
      fields.apiFormat = request.apiFormat as ApiFormat;
    }
    fields.reasoningModel = request.model?.trim() || fields.reasoningModel;
    fields.haikuModel = request.haikuModel?.trim() || fields.haikuModel;
    fields.sonnetModel = request.sonnetModel?.trim() || fields.sonnetModel;
    fields.opusModel = request.opusModel?.trim() || fields.opusModel;
  }

  if (toolId === "codex") {
    if (request.codexWireApi && CODEX_WIRE_APIS.includes(request.codexWireApi as CodexWireApi)) {
      fields.codexWireApi = request.codexWireApi as CodexWireApi;
    }
    if (request.codexReasoningEffort && CODEX_REASONING.includes(request.codexReasoningEffort as CodexReasoningEffort)) {
      fields.codexReasoningEffort = request.codexReasoningEffort as CodexReasoningEffort;
    }
  }

  if (toolId === "openclaw") {
    if (request.apiProtocol && OPENCLAW_PROTOCOLS.includes(request.apiProtocol as OpenClawApiProtocol)) {
      fields.apiProtocol = request.apiProtocol as OpenClawApiProtocol;
    }
  }

  if (toolId === "hermes") {
    const providerMatch = request.notes?.match(/provider=([a-z0-9-]+)/i);
    fields.hermesProvider = providerMatch?.[1] || "custom";
  }

  if (toolId === "opencode") {
    if (request.npm && OPENCODE_NPMS.includes(request.npm as OpenCodeNpmPackage)) {
      fields.npm = request.npm as OpenCodeNpmPackage;
    }
  }

  return {
    toolId,
    name: request.name?.trim() || "Imported Provider",
    configSnapshot: buildStructuredConfig(toolId, fields),
  };
}
