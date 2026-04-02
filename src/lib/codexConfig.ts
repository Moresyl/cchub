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
  mcpServers: string[];
  malformedMcpServers: boolean;
}

export interface CodexStructuredValidation {
  errors: string[];
  warnings: string[];
}

const SECTION_RE = /^\s*\[([^\]]+)\]\s*$/;

function stripQuotes(value: string) {
  const trimmed = value.trim();
  if (
    (trimmed.startsWith("\"") && trimmed.endsWith("\""))
    || (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

function normalizeContent(content: string) {
  return content.replace(/\r\n/g, "\n");
}

function splitLines(content: string) {
  return normalizeContent(content).split("\n");
}

function firstSectionIndex(lines: string[]) {
  return lines.findIndex((line) => SECTION_RE.test(line));
}

function findTopLevelAssignment(lines: string[], key: string) {
  const boundary = firstSectionIndex(lines);
  const end = boundary === -1 ? lines.length : boundary;
  const pattern = new RegExp(`^\\s*${escapeRegExp(key)}\\s*=\\s*(.+)$`);
  for (let index = 0; index < end; index += 1) {
    const match = lines[index]?.match(pattern);
    if (match) {
      return { index, value: match[1] };
    }
  }
  return null;
}

function findSectionRange(lines: string[], header: string) {
  const target = `[${header}]`;
  const start = lines.findIndex((line) => line.trim() === target);
  if (start === -1) return null;

  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (SECTION_RE.test(lines[index] || "")) {
      end = index;
      break;
    }
  }
  return { start, end };
}

function findSectionAssignment(lines: string[], header: string, key: string) {
  const range = findSectionRange(lines, header);
  if (!range) return null;

  const pattern = new RegExp(`^\\s*${escapeRegExp(key)}\\s*=\\s*(.+)$`);
  for (let index = range.start + 1; index < range.end; index += 1) {
    const match = lines[index]?.match(pattern);
    if (match) {
      return { index, value: match[1] };
    }
  }
  return null;
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function upsertTopLevelAssignment(lines: string[], key: string, renderedValue: string) {
  const assignment = `${key} = ${renderedValue}`;
  const existing = findTopLevelAssignment(lines, key);
  if (existing) {
    lines[existing.index] = assignment;
    return lines;
  }

  const boundary = firstSectionIndex(lines);
  const insertAt = boundary === -1 ? lines.length : boundary;
  lines.splice(insertAt, 0, assignment);
  return lines;
}

function removeTopLevelAssignment(lines: string[], key: string) {
  const existing = findTopLevelAssignment(lines, key);
  if (!existing) return lines;
  lines.splice(existing.index, 1);
  return lines;
}

function appendSection(lines: string[], header: string, assignments: string[]) {
  while (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop();
  }
  if (lines.length > 0) {
    lines.push("");
  }
  lines.push(`[${header}]`);
  lines.push(...assignments);
  return lines;
}

function upsertSectionAssignment(lines: string[], header: string, key: string, renderedValue: string) {
  const assignment = `${key} = ${renderedValue}`;
  const existing = findSectionAssignment(lines, header, key);
  if (existing) {
    lines[existing.index] = assignment;
    return lines;
  }

  const range = findSectionRange(lines, header);
  if (!range) {
    return appendSection(lines, header, [assignment]);
  }

  lines.splice(range.end, 0, assignment);
  return lines;
}

function ensureMcpServersTable(lines: string[]) {
  const hasAnyMcpSection = lines.some((line) => /^\s*\[mcp_servers(\]|\.)/.test(line));
  if (hasAnyMcpSection) return lines;
  return appendSection(lines, "mcp_servers", []);
}

function readTopLevelString(lines: string[], key: string, fallback = "") {
  const assignment = findTopLevelAssignment(lines, key);
  return assignment ? stripQuotes(assignment.value) : fallback;
}

function readTopLevelBoolean(lines: string[], key: string, fallback = false) {
  const assignment = findTopLevelAssignment(lines, key);
  if (!assignment) return fallback;
  return assignment.value.trim().toLowerCase() === "true";
}

function readTopLevelInteger(lines: string[], key: string) {
  const assignment = findTopLevelAssignment(lines, key);
  return assignment ? assignment.value.trim() : "";
}

function readSectionString(lines: string[], header: string, key: string, fallback = "") {
  const assignment = findSectionAssignment(lines, header, key);
  return assignment ? stripQuotes(assignment.value) : fallback;
}

export function isCodexConfigToml(activeRoot: string, activeFile: string | null) {
  return activeRoot === "codex" && Boolean(activeFile && /[\\/]config\.toml$/i.test(activeFile));
}

export function parseCodexStructuredConfig(content: string): CodexStructuredConfig {
  const lines = splitLines(content);
  const modelProvider = readTopLevelString(lines, "model_provider", "custom") || "custom";
  const providerSection = `model_providers.${modelProvider}`;
  const mcpServers = Array.from(
    new Set(
      normalizeContent(content)
        .match(/^\s*\[mcp_servers\.([^\]]+)\]\s*$/gm)
        ?.map((line) => line.replace(/^\s*\[mcp_servers\.([^\]]+)\]\s*$/, "$1").replace(/^"(.*)"$/, "$1"))
        || [],
    ),
  );

  return {
    modelProvider,
    providerLabel: readSectionString(lines, providerSection, "name", modelProvider),
    baseUrl: readSectionString(lines, providerSection, "base_url"),
    wireApi: readSectionString(lines, providerSection, "wire_api", "responses"),
    model: readTopLevelString(lines, "model"),
    reasoningEffort: readTopLevelString(lines, "model_reasoning_effort", "medium"),
    personality: readTopLevelString(lines, "personality", "pragmatic"),
    disableResponseStorage: readTopLevelBoolean(lines, "disable_response_storage", false),
    modelContextWindow: readTopLevelInteger(lines, "model_context_window"),
    mcpServers,
    malformedMcpServers: Boolean(findTopLevelAssignment(lines, "mcp_servers")),
  };
}

function renderString(value: string) {
  return JSON.stringify(value);
}

function renderBoolean(value: boolean) {
  return value ? "true" : "false";
}

function normalizeIntegerLike(value: string) {
  const trimmed = value.trim().replace(/[_\s,]/g, "");
  return /^\d+$/.test(trimmed) ? trimmed : "";
}

export function repairCodexConfigContent(content: string) {
  const lines = splitLines(content);
  removeTopLevelAssignment(lines, "mcp_servers");
  ensureMcpServersTable(lines);
  return `${lines.join("\n").replace(/\n{3,}/g, "\n\n").trimEnd()}\n`;
}

export function updateCodexStructuredContent(
  content: string,
  patch: Partial<CodexStructuredConfig>,
) {
  const current = parseCodexStructuredConfig(content);
  const next = { ...current, ...patch };
  const lines = splitLines(content);
  const providerKey = next.modelProvider.trim() || "custom";
  const providerSection = `model_providers.${providerKey}`;

  upsertTopLevelAssignment(lines, "model_provider", renderString(providerKey));
  upsertTopLevelAssignment(lines, "model", renderString(next.model.trim()));
  upsertTopLevelAssignment(lines, "model_reasoning_effort", renderString(next.reasoningEffort.trim() || "medium"));
  upsertTopLevelAssignment(lines, "personality", renderString(next.personality.trim() || "pragmatic"));
  upsertTopLevelAssignment(lines, "disable_response_storage", renderBoolean(next.disableResponseStorage));

  const normalizedContextWindow = normalizeIntegerLike(next.modelContextWindow);
  if (normalizedContextWindow) {
    upsertTopLevelAssignment(lines, "model_context_window", normalizedContextWindow);
  } else {
    removeTopLevelAssignment(lines, "model_context_window");
  }

  upsertSectionAssignment(lines, providerSection, "name", renderString(next.providerLabel.trim() || providerKey));
  upsertSectionAssignment(lines, providerSection, "base_url", renderString(next.baseUrl.trim()));
  upsertSectionAssignment(lines, providerSection, "wire_api", renderString(next.wireApi.trim() || "responses"));
  upsertSectionAssignment(lines, providerSection, "requires_openai_auth", renderBoolean(true));

  const repaired = repairCodexConfigContent(lines.join("\n"));
  return repaired;
}

export function validateCodexStructuredConfig(config: CodexStructuredConfig): CodexStructuredValidation {
  const errors: string[] = [];
  const warnings: string[] = [];

  if (!config.model.trim()) {
    errors.push("Model is required.");
  }
  if (!config.modelProvider.trim()) {
    errors.push("Model provider is required.");
  }
  if (!config.baseUrl.trim()) {
    warnings.push("Base URL is empty. Local auth-only providers may fail without a configured endpoint.");
  }
  const contextWindow = normalizeIntegerLike(config.modelContextWindow);
  if (config.modelContextWindow.trim() && !contextWindow) {
    errors.push("Context window must be an integer.");
  }
  if (config.malformedMcpServers) {
    warnings.push("Detected a malformed top-level mcp_servers assignment. Repairing will normalize it to a TOML table.");
  }

  return { errors, warnings };
}
