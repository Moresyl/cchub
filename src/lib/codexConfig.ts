import { CodexTomlDocument } from "./codexConfig/document";

export interface CodexStructuredConfig {
  modelProvider: string;
  providerLabel: string;
  baseUrl: string;
  wireApi: string;
  model: string;
  reasoningEffort: string;
  personality: string;
  disableResponseStorage: boolean;
  modelContextWindow: string;
  modelAutoCompactTokenLimit: string;
  mcpServers: string[];
  malformedMcpServers: boolean;
}

export interface CodexStructuredValidation {
  errors: string[];
  warnings: string[];
}

export function isCodexConfigToml(activeRoot: string, activeFile: string | null) {
  return activeRoot === "codex" && Boolean(activeFile && /[\\/]config\.toml$/i.test(activeFile));
}

export function parseCodexStructuredConfig(content: string): CodexStructuredConfig {
  const doc = new CodexTomlDocument(content);
  const scalar = (path: string[], kind: "string" | "integer" | "boolean", fallback: string | boolean = "") => {
    const node = doc.get(path);
    if (!node && !doc.has(path)) return fallback;
    if (!node || node.type !== "TOMLValue" || node.kind !== kind) {
      throw new Error(`${path.join(".")} must be a TOML ${kind}`);
    }
    return node.kind === "integer" ? node.bigint.toString() : node.value;
  };
  const modelProvider = String(scalar(["model_provider"], "string", "custom"));
  for (const path of [["model_providers"], ["model_providers", modelProvider]]) {
    if (doc.has(path) && !doc.isTable(path)) throw new Error(`${path.join(".")} must be a TOML table`);
  }
  const provider = (key: string, fallback = "") =>
    String(scalar(["model_providers", modelProvider, key], "string", fallback));
  return {
    modelProvider,
    providerLabel: provider("name", modelProvider),
    baseUrl: provider("base_url"),
    wireApi: provider("wire_api", "responses"),
    model: String(scalar(["model"], "string")),
    reasoningEffort: String(scalar(["model_reasoning_effort"], "string", "medium")),
    personality: String(scalar(["personality"], "string", "pragmatic")),
    disableResponseStorage: Boolean(scalar(["disable_response_storage"], "boolean", false)),
    modelContextWindow: String(scalar(["model_context_window"], "integer")),
    modelAutoCompactTokenLimit: String(scalar(["model_auto_compact_token_limit"], "integer")),
    mcpServers: doc.isTable(["mcp_servers"]) ? doc.children(["mcp_servers"]) : [],
    malformedMcpServers: doc.has(["mcp_servers"]) && !doc.isTable(["mcp_servers"]),
  };
}

export function normalizeCodexInteger(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return "";
  if (!/^(?:\d+|\d{1,3}(?:,\d{3})+|\d+(?:_\d+)+)$/.test(trimmed)) return null;
  const number = BigInt(trimmed.replace(/[_,]/g, ""));
  return number > 0n && number <= 9223372036854775807n ? number.toString() : null;
}

/** Only add an absent table. Never discard existing malformed or valid MCP data. */
export function repairCodexConfigContent(content: string) {
  const doc = new CodexTomlDocument(content);
  if (doc.has(["mcp_servers"])) {
    if (!doc.isTable(["mcp_servers"]))
      throw new Error("Repair mcp_servers in the raw editor; automatic repair would discard its contents");
    return content;
  }
  const next = `${content}${content && !content.endsWith("\n") ? doc.newline : ""}[mcp_servers]${doc.newline}`;
  new CodexTomlDocument(next);
  return next;
}

export function updateCodexStructuredContent(content: string, patch: Partial<CodexStructuredConfig>) {
  const current = parseCodexStructuredConfig(content);
  const providerKey = patch.modelProvider ?? current.modelProvider;
  let next = content;
  const fields: Partial<Record<keyof CodexStructuredConfig, string[]>> = {
    modelProvider: ["model_provider"],
    model: ["model"],
    reasoningEffort: ["model_reasoning_effort"],
    personality: ["personality"],
    disableResponseStorage: ["disable_response_storage"],
    modelContextWindow: ["model_context_window"],
    modelAutoCompactTokenLimit: ["model_auto_compact_token_limit"],
    providerLabel: ["model_providers", providerKey, "name"],
    baseUrl: ["model_providers", providerKey, "base_url"],
    wireApi: ["model_providers", providerKey, "wire_api"],
  };
  for (const [field, value] of Object.entries(patch)) {
    const key = field as keyof CodexStructuredConfig;
    const path = fields[key];
    const sameProvider = path?.[0] !== "model_providers" || providerKey === current.modelProvider;
    if (!path || value === undefined || (sameProvider && value === current[key])) continue;
    let rendered: string | null;
    if (key === "modelContextWindow" || key === "modelAutoCompactTokenLimit") {
      const integer = normalizeCodexInteger(String(value));
      if (integer === null) throw new Error("Token limits must be positive 64-bit integers");
      rendered = integer || null;
    } else {
      rendered = JSON.stringify(value);
    }
    next = new CodexTomlDocument(next).set(path, rendered);
  }
  // Re-parse the result before publishing any patch to the editor.
  parseCodexStructuredConfig(next);
  return next;
}

export function validateCodexStructuredConfig(config: CodexStructuredConfig): CodexStructuredValidation {
  const errors: string[] = [];
  const warnings: string[] = [];
  if (!config.modelProvider.trim()) errors.push("Model provider is required.");
  if (!config.baseUrl.trim() && config.modelProvider !== "openai") {
    warnings.push("Base URL is empty. Check the selected provider's endpoint before use.");
  }
  if (normalizeCodexInteger(config.modelContextWindow) === null)
    errors.push("Context window must be a positive 64-bit integer.");
  if (normalizeCodexInteger(config.modelAutoCompactTokenLimit) === null)
    errors.push("Auto compact token limit must be a positive 64-bit integer.");
  if (config.malformedMcpServers)
    warnings.push(
      "mcp_servers must be a TOML table. Correct it in the raw editor; its existing contents will be preserved.",
    );
  return { errors, warnings };
}
