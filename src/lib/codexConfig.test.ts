import { describe, expect, it } from "vitest";
import fixtures from "./codexConfig/fixtures.json";
import {
  isCodexConfigToml,
  normalizeCodexInteger,
  parseCodexStructuredConfig,
  repairCodexConfigContent,
  updateCodexStructuredContent,
  validateCodexStructuredConfig,
} from "./codexConfig";

const sampleConfig = `model_provider = "custom"
model = "gpt-5.4"
model_reasoning_effort = "high"
disable_response_storage = true

[model_providers.custom]
name = "Custom"
base_url = "https://example.test/v1"
wire_api = "responses"

[mcp_servers.filesystem]
command = "npx"
`;

describe("codexConfig", () => {
  it.each(fixtures)("matches shared native-save fixture: $name", ({ source, patch, expected, fields }) => {
    expect(updateCodexStructuredContent(source, patch)).toBe(expected);
    expect(parseCodexStructuredConfig(expected)).toMatchObject(fields);
  });
  it("detects Codex config.toml paths", () => {
    expect(isCodexConfigToml("codex", "C:/Users/me/.codex/config.toml")).toBe(true);
    expect(isCodexConfigToml("claude", "C:/Users/me/.codex/config.toml")).toBe(false);
    expect(isCodexConfigToml("codex", "C:/Users/me/.codex/auth.json")).toBe(false);
  });

  it("parses structured config fields and MCP tables", () => {
    const parsed = parseCodexStructuredConfig(sampleConfig);

    expect(parsed.modelProvider).toBe("custom");
    expect(parsed.providerLabel).toBe("Custom");
    expect(parsed.baseUrl).toBe("https://example.test/v1");
    expect(parsed.model).toBe("gpt-5.4");
    expect(parsed.reasoningEffort).toBe("high");
    expect(parsed.disableResponseStorage).toBe(true);
    expect(parsed.mcpServers).toEqual(["filesystem"]);
    expect(parsed.malformedMcpServers).toBe(false);
  });

  it("only adds an absent MCP table and preserves valid or malformed data", () => {
    expect(repairCodexConfigContent(sampleConfig)).toBe(sampleConfig);
    expect(repairCodexConfigContent('model = "a"\r\n')).toBe('model = "a"\r\n[mcp_servers]\r\n');
    const malformed = 'model = "a"\nmcp_servers = ["keep-me"]\n';
    expect(() => repairCodexConfigContent(malformed)).toThrow("discard");
    expect(updateCodexStructuredContent(malformed, { model: "b" })).toBe(malformed.replace('"a"', '"b"'));
  });

  it("updates structured config while preserving MCP sections", () => {
    const updated = updateCodexStructuredContent(sampleConfig, {
      model: "gpt-5.5",
      baseUrl: "https://api.example.test/v1",
      modelContextWindow: "400,000",
      modelAutoCompactTokenLimit: "90000",
    });

    expect(updated).toContain('model = "gpt-5.5"');
    expect(updated).toContain('base_url = "https://api.example.test/v1"');
    expect(updated).toMatch(/"?model_context_window"? = 400000/);
    expect(updated).toMatch(/"?model_auto_compact_token_limit"? = 90000/);
    expect(updated).toContain("[mcp_servers.filesystem]");
  });

  it("validates provider and token limits without requiring an explicitly configured model", () => {
    const parsed = parseCodexStructuredConfig(sampleConfig);
    const validation = validateCodexStructuredConfig({
      ...parsed,
      model: "",
      modelProvider: "",
      modelContextWindow: "40k",
    });

    expect(validation.errors).toContain("Model provider is required.");
    expect(validation.errors).toContain("Context window must be a positive 64-bit integer.");
  });

  it("decodes escaped, quoted and dotted provider keys with inline MCP tables", () => {
    const source = `model_provider = 'team.example' # keep
model = "a\\"b" # model
model_providers = { 'team.example' = { name = "Team", base_url = 'https://example.test/a#b', requires_openai_auth = false } }
mcp_servers = { 'fs.local' = { command = 'npx', args = ['keep'] } }
unknown = """first
[not.a.section]
last"""
`;
    const parsed = parseCodexStructuredConfig(source);
    expect(parsed.model).toBe('a"b');
    expect(parsed.baseUrl).toBe("https://example.test/a#b");
    expect(parsed.mcpServers).toEqual(["fs.local"]);
    expect(parsed.malformedMcpServers).toBe(false);
    expect(updateCodexStructuredContent(source, { baseUrl: "https://new.test" })).toBe(
      source.replace("'https://example.test/a#b'", '"https://new.test"'),
    );
    expect(updateCodexStructuredContent(source, { model: parsed.model })).toBe(source);
  });

  it("preserves BOM, CRLF, comments, unrelated authentication policy and exact integers", () => {
    const source =
      '\uFEFFmodel_provider = "team.example"\r\nmodel = \'old\' # model\r\nmodel_context_window = 9_007_199_254_740_993 # exact\r\n[model_providers."team.example"] # provider\r\nbase_url = "https://old.test" # url\r\nrequires_openai_auth = false\r\n[extra]\r\nmodel = "leave alone"\r\n';
    const parsed = parseCodexStructuredConfig(source);
    expect(parsed.modelContextWindow).toBe("9007199254740993");
    expect(updateCodexStructuredContent(source, { model: "new" })).toBe(source.replace("'old'", '"new"'));
    expect(updateCodexStructuredContent(source, { baseUrl: "https://new.test" })).toBe(
      source.replace("https://old.test", "https://new.test"),
    );
    expect(updateCodexStructuredContent(source, { modelContextWindow: "" })).toContain(" # exact\r\n");
  });

  it("adds only the requested field in existing, implicit and inline tables", () => {
    for (const source of [
      'model_provider = "custom"\n[model_providers.custom] # header',
      'model_provider = "custom"\n[model_providers.custom.headers]\nkey = "keep"\n',
      'model_provider = "custom"\nmodel_providers = {custom = {name = "Keep"}}\n',
      'model_provider = "custom"\nmodel_providers = {}\n',
      "",
    ]) {
      const updated = updateCodexStructuredContent(source, { baseUrl: "https://new.test" });
      expect(parseCodexStructuredConfig(updated).baseUrl).toBe("https://new.test");
      expect(updated).not.toContain("requires_openai_auth");
      expect(updated).not.toContain("mcp_servers");
      expect(updated).not.toContain("model_reasoning_effort");
      if (source.includes('key = "keep"')) expect(updated).toContain('key = "keep"');
    }
  });

  it("retains the old provider when switching and creates requested fields even if their values match the old provider", () => {
    const switched = updateCodexStructuredContent(sampleConfig, {
      modelProvider: "new.team",
      baseUrl: "https://new.test",
      providerLabel: "Custom",
    });
    expect(parseCodexStructuredConfig(switched).baseUrl).toBe("https://new.test");
    expect(parseCodexStructuredConfig(switched).providerLabel).toBe("Custom");
    expect(switched).toContain("[model_providers.custom]");
    expect(switched).toContain("https://example.test/v1");
  });

  it.each([
    'model = "a"\nmodel = "b"',
    'model = "unfinished',
    'model_provider = ["custom"]',
    'model_context_window = "400000"',
    "model_providers = []",
    "model_providers = {custom = []}",
    '[[model_providers.custom]]\nname = "wrong"',
  ])("refuses to regenerate malformed or incorrectly typed source: %s", (source) => {
    expect(() => parseCodexStructuredConfig(source)).toThrow();
    expect(() => updateCodexStructuredContent(source, { model: "changed" })).toThrow();
  });

  it("does not traverse inherited object keys when indexing configuration", () => {
    const source =
      'model_provider = "__proto__"\n[model_providers."__proto__"]\nbase_url = "https://safe.test"\n[mcp_servers.constructor]\ncommand = "safe"\n';
    expect(parseCodexStructuredConfig(source).baseUrl).toBe("https://safe.test");
    expect(updateCodexStructuredContent(source, { baseUrl: "https://changed.test" })).toContain("https://changed.test");
    expect(Object.prototype).not.toHaveProperty("base_url");
  });

  it.each(["bad", "40k", "4,00", "1 000", "0", "-1", "9223372036854775808"])(
    "rejects invalid token limits without dropping the existing value: %s",
    (value) => {
      expect(normalizeCodexInteger(value)).toBeNull();
      expect(() => updateCodexStructuredContent(sampleConfig, { modelContextWindow: value })).toThrow();
    },
  );
  it.each([
    ["400,000", "400000"],
    ["1_000_000", "1000000"],
    ["9007199254740993", "9007199254740993"],
    ["9223372036854775807", "9223372036854775807"],
    ["", ""],
  ])("normalizes valid integer input %s precisely", (value, expected) => {
    expect(normalizeCodexInteger(value)).toBe(expected);
  });
});
