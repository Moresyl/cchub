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
  url?: string;
  envKeys: string[];
  env: Record<string, string>;
  headers: Record<string, string>;
  transport: string;
}

export type DeepLinkRiskKind = "envHijack" | "privateEndpoint" | "shellCommand";

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
  mergeRequest: (request: DeepLinkImportRequest) =>
    invoke<DeepLinkImportRequest>("merge_deeplink_request", { request }),
  importMcp: (request: DeepLinkImportRequest) =>
    invoke<DeepLinkMcpImportResult>("import_mcp_servers_from_deeplink", { request }),
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

  // URL query parsing turns '+' into a space. Restore it before accepting both
  // standard and RFC 4648 URL-safe Base64 payloads.
  const normalized = trimmed.replace(/ /g, "+").replace(/-/g, "+").replace(/_/g, "/");
  const padded = normalized + "=".repeat((4 - (normalized.length % 4)) % 4);
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
  return (
    trimmed.startsWith("{") ||
    trimmed.startsWith("[") ||
    trimmed.startsWith("#") ||
    trimmed.includes("\n") ||
    trimmed.includes("\r\n") ||
    trimmed.includes(" = ") ||
    trimmed.includes('="') ||
    trimmed.includes("[model_providers")
  );
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

    if (typeof parsed.command === "string" || typeof parsed.url === "string") {
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
  const command = typeof record.command === "string" ? record.command : "";
  const url = typeof record.url === "string" ? record.url : undefined;
  if (!command && !url) return null;
  const env = parseStringMap(record.env);
  const headers = parseStringMap(record.headers);
  return {
    name,
    command,
    args: Array.isArray(record.args) ? record.args.filter((item): item is string => typeof item === "string") : [],
    url,
    envKeys: [...new Set([...Object.keys(env), ...Object.keys(headers)])],
    env,
    headers,
    transport: typeof record.type === "string" ? record.type : url ? "http" : "stdio",
  };
}

function parseStringMap(value: unknown): Record<string, string> {
  if (!value || typeof value !== "object") return {};
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .filter(([, item]) => typeof item === "string")
      .map(([key, item]) => [key, item as string]),
  );
}

const SENSITIVE_CONFIG_KEY_MARKERS = ["TOKEN", "KEY", "SECRET", "PASSWORD", "AUTHORIZATION", "COOKIE", "CREDENTIAL"];
const ENV_HIJACK_PATTERNS = [
  /^LD_/i,
  /^DYLD_/i,
  /^NODE_OPTIONS$/i,
  /^NODE_EXTRA_CA_CERTS$/i,
  /^PYTHONPATH$/i,
  /^PYTHONSTARTUP$/i,
  /^RUBYOPT$/i,
  /^PERL5OPT$/i,
  /^JAVA_TOOL_OPTIONS$/i,
  /^BASH_ENV$/i,
  /^ENV$/i,
  /^IFS$/i,
  /^PATH$/i,
  /^HTTPS?_PROXY$/i,
];
const SHELL_INTERPRETERS = new Set([
  "sh",
  "bash",
  "zsh",
  "dash",
  "ksh",
  "fish",
  "csh",
  "tcsh",
  "cmd",
  "cmd.exe",
  "powershell",
  "powershell.exe",
  "pwsh",
  "pwsh.exe",
]);

export function isSensitiveConfigKey(key: string): boolean {
  const normalized = key.toUpperCase();
  return (
    normalized === "AUTH" ||
    normalized === "BEARER" ||
    SENSITIVE_CONFIG_KEY_MARKERS.some((marker) => normalized.includes(marker))
  );
}

export function maskConfigValue(key: string, value: string): string {
  if (!isSensitiveConfigKey(key)) return value;
  return value.length > 8 ? `${value.slice(0, 4)}${"*".repeat(12)}` : "****";
}

export function classifyDeepLinkEndpoint(value: string): DeepLinkRiskKind | null {
  try {
    const host = new URL(value).hostname.toLowerCase().replace(/^\[|\]$/g, "");
    if (
      host === "localhost" ||
      host.endsWith(".localhost") ||
      host.endsWith(".local") ||
      host.endsWith(".internal") ||
      host === "::1" ||
      host === "::" ||
      host === "0.0.0.0"
    )
      return "privateEndpoint";
    const parts = host.split(".").map(Number);
    if (parts.length === 4 && parts.every((part) => Number.isInteger(part) && part >= 0 && part <= 255)) {
      const [a, b] = parts;
      if (
        a === 0 ||
        a === 10 ||
        a === 127 ||
        (a === 172 && b >= 16 && b <= 31) ||
        (a === 192 && b === 168) ||
        (a === 169 && b === 254)
      )
        return "privateEndpoint";
    }
    if (/^f[cd][0-9a-f]{2}:|^fe[89ab][0-9a-f]:/.test(host)) return "privateEndpoint";
  } catch {
    return null;
  }
  return null;
}

export function classifyDeepLinkEnvKey(key: string): DeepLinkRiskKind | null {
  return ENV_HIJACK_PATTERNS.some((pattern) => pattern.test(key)) ? "envHijack" : null;
}

export function classifyDeepLinkCommand(command: string, args: string[]): DeepLinkRiskKind | null {
  const base = command.split(/[\\/]/).pop()?.toLowerCase() || "";
  if (!SHELL_INTERPRETERS.has(base)) return null;
  return args.some(
    (arg) =>
      /^\/[ck]\b/i.test(arg) ||
      /^-[a-z]*c[a-z]*$/i.test(arg) ||
      ["-command", "-encodedcommand", "-e", "-ec"].includes(arg.toLowerCase()),
  )
    ? "shellCommand"
    : null;
}

function addUsageMetadata(snapshot: string, request: DeepLinkImportRequest): string {
  const hasUsage =
    request.usageScript !== undefined ||
    request.usageEnabled !== undefined ||
    request.usageApiKey !== undefined ||
    request.usageBaseUrl !== undefined ||
    request.usageAccessToken !== undefined ||
    request.usageUserId !== undefined ||
    request.usageAutoInterval !== undefined;
  if (!hasUsage) return snapshot;
  try {
    const document = JSON.parse(snapshot) as Record<string, unknown>;
    const metadata =
      document.metadata && typeof document.metadata === "object" ? (document.metadata as Record<string, unknown>) : {};
    metadata.usageScript = {
      enabled: request.usageEnabled === true,
      language: "javascript",
      code: decodeDeepLinkText(request.usageScript),
      apiKey: request.usageApiKey?.trim() || undefined,
      baseUrl: request.usageBaseUrl?.trim() || undefined,
      accessToken: request.usageAccessToken?.trim() || undefined,
      userId: request.usageUserId?.trim() || undefined,
      autoQueryInterval: request.usageAutoInterval,
    };
    document.metadata = metadata;
    return JSON.stringify(document, null, 2);
  } catch {
    return snapshot;
  }
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
    if (
      request.codexReasoningEffort &&
      CODEX_REASONING.includes(request.codexReasoningEffort as CodexReasoningEffort)
    ) {
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
    configSnapshot: addUsageMetadata(buildStructuredConfig(toolId, fields), request),
  };
}
