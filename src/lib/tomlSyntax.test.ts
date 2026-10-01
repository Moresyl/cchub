import { describe, expect, it } from "vitest";
import { getTomlSyntaxError, parseTomlSource } from "./tomlSyntax";

describe("TOML syntax diagnostics", () => {
  it("keeps BOM and CRLF offsets consistent with the raw editor", () => {
    const source = '\uFEFFmodel = "ok"\r\nbroken = "unfinished';
    const error = getTomlSyntaxError(source)!;
    expect(error.index).toBe(source.length);
    expect(error.message).toContain("TOML (2:21)");
    expect(error.message).not.toContain("unfinished");
  });
  it.each(["", '\uFEFFmodel = "ok"\r\n', "mcp_servers = {keep = {command = 'node'}}"])(
    "accepts valid TOML without a fake error: %s",
    (source) => {
      expect(getTomlSyntaxError(source)).toBeNull();
      expect(parseTomlSource(source).type).toBe("Program");
    },
  );
  it("rejects duplicate assignments and TOML 1.1-only inline table syntax", () => {
    expect(getTomlSyntaxError('model = "a"\nmodel = "b"')).not.toBeNull();
    expect(getTomlSyntaxError("mcp_servers = {keep = {},}")).not.toBeNull();
  });
});
