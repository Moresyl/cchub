import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useMcpValidation } from "./useMcpValidation";

describe("useMcpValidation", () => {
  it("localizes errors and never repeats malformed secret values", () => {
    const { result } = renderHook(() =>
      useMcpValidation(
        {
          name: "中文",
          transport: "http",
          command: "invalid",
          argsText: "",
          envText: "secret-token-without-equals",
        },
        true,
      ),
    );
    expect(result.current.isValid).toBe(false);
    expect(result.current.errors).toContain("服务名称只能包含英文字母、数字、点、下划线和连字符。");
    expect(result.current.errors).toContain("环境变量或请求头的每一行必须使用 KEY=value 格式。");
    expect(result.current.errors.join(" ")).not.toContain("secret-token");
  });
  it("accepts a remote HTTP endpoint and header names", () => {
    const { result } = renderHook(() =>
      useMcpValidation({
        name: "remote-search",
        transport: "http",
        command: "https://example.com/mcp",
        argsText: "ignored for remote transports",
        envText: "Authorization=Bearer token\nX-API-Key=secret",
      }),
    );

    expect(result.current.isValid).toBe(true);
    expect(result.current.parsedArgs).toEqual([]);
    expect(result.current.parsedEnv).toEqual({ Authorization: "Bearer token", "X-API-Key": "secret" });
  });

  it("rejects non-HTTP remote URLs", () => {
    const { result } = renderHook(() =>
      useMcpValidation({
        name: "unsafe-remote",
        transport: "sse",
        command: "file:///tmp/server",
        argsText: "",
        envText: "",
      }),
    );

    expect(result.current.isValid).toBe(false);
    expect(result.current.errors).toContain("Remote URL must use HTTP or HTTPS.");
  });
});
