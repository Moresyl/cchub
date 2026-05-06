import { describe, expect, it } from "vitest";
import {
  isCodexConfigToml,
  parseCodexStructuredConfig,
  repairCodexConfigContent,
  updateCodexStructuredContent,
  validateCodexStructuredConfig,
} from "./codexConfig";

const sampleConfig = `model_provider = "custom"
model = "gpt-5.4"
model_reasoning_effort = "high"
disable_response_storage = true
mcp_servers = ["bad"]

[model_providers.custom]
name = "Custom"
base_url = "https://example.test/v1"
wire_api = "responses"

[mcp_servers.filesystem]
command = "npx"
`;

describe("codexConfig", () => {
  it("detects Codex config.toml paths", () => {
    expect(isCodexConfigToml("codex", "C:/Users/me/.codex/config.toml")).toBe(true);
    expect(isCodexConfigToml("claude", "C:/Users/me/.codex/config.toml")).toBe(false);
    expect(isCodexConfigToml("codex", "C:/Users/me/.codex/auth.json")).toBe(false);
  });

  it("parses structured config fields and malformed MCP marker", () => {
    const parsed = parseCodexStructuredConfig(sampleConfig);

    expect(parsed.modelProvider).toBe("custom");
    expect(parsed.providerLabel).toBe("Custom");
    expect(parsed.baseUrl).toBe("https://example.test/v1");
    expect(parsed.model).toBe("gpt-5.4");
    expect(parsed.reasoningEffort).toBe("high");
    expect(parsed.disableResponseStorage).toBe(true);
    expect(parsed.mcpServers).toEqual(["filesystem"]);
    expect(parsed.malformedMcpServers).toBe(true);
  });

  it("repairs malformed top-level MCP assignment", () => {
    const repaired = repairCodexConfigContent(sampleConfig);

    expect(repaired).not.toContain("mcp_servers = [");
    expect(repaired).toContain("[mcp_servers.filesystem]");
  });

  it("updates structured config while preserving MCP sections", () => {
    const updated = updateCodexStructuredContent(sampleConfig, {
      model: "gpt-5.5",
      baseUrl: "https://api.example.test/v1",
      modelContextWindow: "400,000",
      modelAutoCompactTokenLimit: "bad-value",
    });

    expect(updated).toContain('model = "gpt-5.5"');
    expect(updated).toContain('base_url = "https://api.example.test/v1"');
    expect(updated).toContain("model_context_window = 400000");
    expect(updated).not.toContain("model_auto_compact_token_limit");
    expect(updated).toContain("[mcp_servers.filesystem]");
  });

  it("validates required model and integer fields", () => {
    const parsed = parseCodexStructuredConfig(sampleConfig);
    const validation = validateCodexStructuredConfig({
      ...parsed,
      model: "",
      modelContextWindow: "40k",
    });

    expect(validation.errors).toContain("Model is required.");
    expect(validation.errors).toContain("Context window must be an integer.");
    expect(validation.warnings).toContain(
      "Detected a malformed top-level mcp_servers assignment. Repairing will normalize it to a TOML table.",
    );
  });
});
