import { describe, expect, it } from "vitest";
import {
  buildStructuredConfig,
  createDefaultStructuredFields,
  parseStructuredConfig,
  normalizeEndpointList,
  normalizeRequestHeaders,
  normalizeCustomUserAgent,
  supportsStructuredConfig,
} from "./configProfiles";

describe("configProfiles", () => {
  it("detects tools that support structured configuration", () => {
    expect(supportsStructuredConfig("claude")).toBe(true);
    expect(supportsStructuredConfig("codex")).toBe(true);
    expect(supportsStructuredConfig("unknown-tool")).toBe(false);
  });

  it("builds and parses Claude structured config", () => {
    const fields = {
      ...createDefaultStructuredFields("claude"),
      baseUrl: "https://api.example.test/anthropic",
      apiKey: "sk-test",
      model: "claude-sonnet-4.6",
      reasoningModel: "claude-opus-4.6",
      category: "official",
      endpointCandidates: "https://api.example.test/anthropic\nhttps://backup.example.test/anthropic",
      hideAttribution: true,
      effortHigh: true,
      enableTeammates: true,
    };

    const content = buildStructuredConfig("claude", fields);
    const parsedJson = JSON.parse(content) as {
      env: Record<string, string>;
      metadata: { endpointCandidates: string[] };
      attribution: { commit: string; pr: string };
      effortLevel: string;
    };

    expect(parsedJson.env.ANTHROPIC_AUTH_TOKEN).toBe("sk-test");
    expect(parsedJson.env.ANTHROPIC_BASE_URL).toBe("https://api.example.test/anthropic");
    expect(parsedJson.env.CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS).toBe("1");
    expect(parsedJson.metadata.endpointCandidates).toEqual([
      "https://api.example.test/anthropic",
      "https://backup.example.test/anthropic",
    ]);
    expect(parsedJson.attribution).toEqual({ commit: "", pr: "" });
    expect(parsedJson.effortLevel).toBe("high");

    const parsedFields = parseStructuredConfig("claude", content);
    expect(parsedFields.baseUrl).toBe(fields.baseUrl);
    expect(parsedFields.apiKey).toBe(fields.apiKey);
    expect(parsedFields.model).toBe(fields.model);
    expect(parsedFields.reasoningModel).toBe(fields.reasoningModel);
    expect(parsedFields.hideAttribution).toBe(true);
    expect(parsedFields.effortHigh).toBe(true);
    expect(parsedFields.enableTeammates).toBe(true);
  });

  it("round-trips custom endpoints and rejects unsafe values", () => {
    const fields = {
      ...createDefaultStructuredFields("claude"),
      customEndpoints: ["https://api.example.test", "https://backup.example.test"],
    };
    const content = buildStructuredConfig("claude", fields);
    expect(JSON.parse(content).customEndpoints).toEqual(fields.customEndpoints);
    expect(parseStructuredConfig("claude", content).customEndpoints).toEqual(fields.customEndpoints);
    expect(
      normalizeEndpointList(["https://api.example.test///", "file:///tmp/no", { url: "https://backup.example.test" }]),
    ).toEqual(["https://api.example.test", "https://backup.example.test"]);
  });

  it("round-trips transport overrides and filters unsafe headers", () => {
    const fields = {
      ...createDefaultStructuredFields("claude"),
      customUserAgent: " CCHub-Test/1.0 ",
      requestHeaders: {
        "X-Trace": "abc",
        Authorization: "must-not-be-sent",
        "bad header": "ignored",
      },
    };
    const content = buildStructuredConfig("claude", fields);
    const parsed = JSON.parse(content) as {
      metadata: { customUserAgent: string; requestHeaders: Record<string, string> };
    };
    expect(parsed.metadata.customUserAgent).toBe("CCHub-Test/1.0");
    expect(parsed.metadata.requestHeaders).toEqual({ "X-Trace": "abc" });
    expect(parseStructuredConfig("claude", content).requestHeaders).toEqual({ "X-Trace": "abc" });
    expect(normalizeCustomUserAgent("bad\nagent")).toBe("");
    expect(normalizeRequestHeaders({ "X-Test": "ok", "bad header": "no" })).toEqual({ "X-Test": "ok" });
  });

  it("preserves imported usage scripts while rebuilding structured config", () => {
    const fields = {
      ...createDefaultStructuredFields("claude"),
      usageScript: {
        enabled: true,
        code: "return { remaining: 1 };",
        timeout: 1200,
      },
    };
    const content = buildStructuredConfig("claude", fields);
    expect(JSON.parse(content).metadata.usageScript).toEqual(fields.usageScript);
    expect(parseStructuredConfig("claude", content).usageScript).toEqual(fields.usageScript);
  });

  it("round-trips local proxy JSON overrides without stream control", () => {
    const fields = {
      ...createDefaultStructuredFields("codex"),
      requestHeaderOverrides: JSON.stringify({ "X-Provider-Tag": "cchub", Authorization: "ignored" }),
      requestBodyOverrides: JSON.stringify({ temperature: 0.2, stream: false }),
    };
    const content = buildStructuredConfig("codex", fields);
    const parsed = parseStructuredConfig("codex", content);
    expect(JSON.parse(parsed.requestHeaderOverrides)).toEqual({ "X-Provider-Tag": "cchub" });
    expect(JSON.parse(parsed.requestBodyOverrides)).toEqual({ temperature: 0.2 });
  });

  it("builds and parses OpenClaw model metadata", () => {
    const fields = {
      ...createDefaultStructuredFields("openclaw"),
      baseUrl: "https://api.example.test/v1",
      apiKey: "openclaw-key",
      apiProtocol: "openai-responses" as const,
      model: "anthropic/claude-sonnet-4",
      modelName: "Claude Sonnet",
      openClawContextWindow: "200000",
      openClawCostInput: "0.003",
      openClawCostOutput: "0.015",
      modelCatalogAlias: "Claude",
      suggestedPrimaryModel: "anthropic/claude-sonnet-4",
      suggestedFallbackModels: "anthropic/claude-haiku-4, anthropic/claude-opus-4",
    };

    const content = buildStructuredConfig("openclaw", fields);
    const parsedJson = JSON.parse(content) as {
      baseUrl: string;
      apiKey: string;
      api: string;
      models: Array<{ id: string; name: string; contextWindow: number; cost: { input: number; output: number } }>;
      modelCatalog: Record<string, { alias: string }>;
      suggestedDefaults: { primary: string; fallbacks: string[] };
    };

    expect(parsedJson.baseUrl).toBe(fields.baseUrl);
    expect(parsedJson.apiKey).toBe(fields.apiKey);
    expect(parsedJson.api).toBe("openai-responses");
    expect(parsedJson.models[0]).toEqual({
      id: "anthropic/claude-sonnet-4",
      name: "Claude Sonnet",
      contextWindow: 200000,
      cost: { input: 0.003, output: 0.015 },
    });
    expect(parsedJson.modelCatalog["anthropic/claude-sonnet-4"]).toEqual({ alias: "Claude" });
    expect(parsedJson.suggestedDefaults.fallbacks).toEqual(["anthropic/claude-haiku-4", "anthropic/claude-opus-4"]);

    const parsedFields = parseStructuredConfig("openclaw", content);
    expect(parsedFields.baseUrl).toBe(fields.baseUrl);
    expect(parsedFields.apiKey).toBe(fields.apiKey);
    expect(parsedFields.apiProtocol).toBe(fields.apiProtocol);
    expect(parsedFields.model).toBe(fields.model);
    expect(parsedFields.modelName).toBe(fields.modelName);
    expect(parsedFields.openClawContextWindow).toBe(fields.openClawContextWindow);
    expect(parsedFields.openClawCostInput).toBe(fields.openClawCostInput);
    expect(parsedFields.openClawCostOutput).toBe(fields.openClawCostOutput);
    expect(parsedFields.modelCatalogAlias).toBe(fields.modelCatalogAlias);
  });
});
