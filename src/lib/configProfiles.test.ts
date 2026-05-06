import { describe, expect, it } from "vitest";
import {
  buildStructuredConfig,
  createDefaultStructuredFields,
  parseStructuredConfig,
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
