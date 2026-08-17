import { describe, expect, it } from "vitest";
import {
  buildProviderProfileFromDeepLink,
  classifyDeepLinkCommand,
  classifyDeepLinkEndpoint,
  classifyDeepLinkEnvKey,
  decodeDeepLinkText,
  maskConfigValue,
  parseMcpPreviewServers,
  type DeepLinkImportRequest,
} from "./deeplink";

function encoded(value: string) {
  return btoa(value);
}

describe("deep link payloads", () => {
  it("decodes URL-safe and unpadded Base64 payloads", () => {
    const payload = encoded('{"name":"remote"}').replace(/=/g, "").replace(/\+/g, "-").replace(/\//g, "_");
    expect(decodeDeepLinkText(payload)).toBe('{"name":"remote"}');
  });

  it("renders remote MCP URLs, headers, and stdio arguments", () => {
    const request: DeepLinkImportRequest = {
      version: "v1",
      resource: "mcp",
      apps: "grokbuild",
      config: encoded(
        JSON.stringify({
          mcpServers: {
            remote: { url: "http://127.0.0.1:9000/mcp", headers: { Authorization: "Bearer secret" } },
            shell: { command: "sh", args: ["-lc", "echo hello"], env: { NODE_OPTIONS: "--require ./hook.js" } },
          },
        }),
      ),
    };
    const servers = parseMcpPreviewServers(request);
    expect(servers).toHaveLength(2);
    expect(servers[0]).toMatchObject({
      url: "http://127.0.0.1:9000/mcp",
      transport: "http",
      headers: { Authorization: "Bearer secret" },
    });
    expect(servers[1].args).toEqual(["-lc", "echo hello"]);
  });

  it("persists usage metadata disabled unless explicitly requested", () => {
    const profile = buildProviderProfileFromDeepLink({
      version: "v1",
      resource: "provider",
      app: "claude",
      name: "Usage test",
      endpoint: "https://api.example.com",
      usageScript: encoded("return { remaining: 1 };"),
    });
    const snapshot = JSON.parse(profile.configSnapshot) as {
      metadata?: { usageScript?: { enabled?: boolean; code?: string } };
    };
    expect(snapshot.metadata?.usageScript).toMatchObject({ enabled: false, code: "return { remaining: 1 };" });
  });

  it("marks risky MCP values and masks credentials", () => {
    expect(classifyDeepLinkEndpoint("http://127.0.0.1:11434")).toBe("privateEndpoint");
    expect(classifyDeepLinkEnvKey("NODE_OPTIONS")).toBe("envHijack");
    expect(classifyDeepLinkCommand("/bin/sh", ["-c", "echo hi"])).toBe("shellCommand");
    expect(maskConfigValue("Authorization", "Bearer secret")).toBe("Bear************");
    expect(maskConfigValue("MODEL", "gpt-5")).toBe("gpt-5");
  });
});
