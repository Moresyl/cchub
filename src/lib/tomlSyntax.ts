import { parseTOML, ParseError } from "toml-eslint-parser";

export function parseTomlSource(source: string) {
  return parseTOML(source.startsWith("\uFEFF") ? ` ${source.slice(1)}` : source, { tomlVersion: "1.0.0" });
}

export function getTomlSyntaxError(source: string) {
  try {
    parseTomlSource(source);
    return null;
  } catch (error) {
    return {
      index: error instanceof ParseError ? error.index : 0,
      message:
        error instanceof ParseError
          ? `TOML (${error.lineNumber}:${error.column + 1}): ${error.message}`
          : "Invalid TOML syntax",
    };
  }
}
