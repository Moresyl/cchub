import { useMemo } from "react";

export interface McpWizardDraft {
  name: string;
  transport: "stdio" | "http" | "sse";
  command: string;
  argsText: string;
  envText: string;
}

export interface McpValidationResult {
  isValid: boolean;
  errors: string[];
  warnings: string[];
  parsedArgs: string[];
  parsedEnv: Record<string, string>;
}

function parseArgs(argsText: string) {
  const trimmed = argsText.trim();
  if (!trimmed) return { parsedArgs: [] as string[], errors: [] as string[] };

  if (trimmed.startsWith("[")) {
    try {
      const parsed = JSON.parse(trimmed) as unknown;
      if (!Array.isArray(parsed) || parsed.some((item) => typeof item !== "string")) {
        return { parsedArgs: [], errors: ["Arguments JSON must be a string array."] };
      }
      return { parsedArgs: parsed, errors: [] };
    } catch {
      return { parsedArgs: [], errors: ["Arguments JSON is invalid."] };
    }
  }

  return {
    parsedArgs: trimmed
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean),
    errors: [],
  };
}

function parseEnv(envText: string, headers: boolean) {
  const trimmed = envText.trim();
  if (!trimmed) return { parsedEnv: {} as Record<string, string>, errors: [] as string[] };

  if (trimmed.startsWith("{")) {
    try {
      const parsed = JSON.parse(trimmed) as unknown;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        return { parsedEnv: {}, errors: ["Environment JSON must be an object."] };
      }
      const entries = Object.entries(parsed as Record<string, unknown>);
      if (entries.some(([key, value]) => !key || typeof value !== "string")) {
        return { parsedEnv: {}, errors: ["Environment JSON values must be strings."] };
      }
      return {
        parsedEnv: Object.fromEntries(entries as [string, string][]),
        errors: [],
      };
    } catch {
      return { parsedEnv: {}, errors: ["Environment JSON is invalid."] };
    }
  }

  const parsedEnv: Record<string, string> = {};
  const errors: string[] = [];
  for (const line of trimmed.split("\n")) {
    const value = line.trim();
    if (!value || value.startsWith("#")) continue;
    const separator = value.indexOf("=");
    if (separator <= 0) {
      errors.push(`Invalid env line: ${value}`);
      continue;
    }
    const key = value.slice(0, separator).trim();
    const envValue = value.slice(separator + 1).trim();
    const validKey = headers ? /^[A-Za-z0-9_-]+$/.test(key) : /^[A-Za-z_][A-Za-z0-9_]*$/.test(key);
    if (!validKey) {
      errors.push(`Invalid ${headers ? "header" : "env"} key: ${key}`);
      continue;
    }
    parsedEnv[key] = envValue;
  }

  return { parsedEnv, errors };
}

export function useMcpValidation(draft: McpWizardDraft): McpValidationResult {
  return useMemo(() => {
    const errors: string[] = [];
    const warnings: string[] = [];

    const name = draft.name.trim();
    const command = draft.command.trim();

    if (!name) {
      errors.push("Server name is required.");
    } else if (!/^[A-Za-z0-9._-]+$/.test(name)) {
      errors.push("Server name may only contain letters, numbers, dot, underscore, and dash.");
    }

    if (!command) {
      errors.push(draft.transport === "stdio" ? "Command is required." : "Remote URL is required.");
    } else if (draft.transport === "stdio" && command.includes(" ")) {
      warnings.push("Command contains spaces. Move extra tokens into the arguments field when possible.");
    } else if (draft.transport !== "stdio") {
      try {
        const url = new URL(command);
        if (!["http:", "https:"].includes(url.protocol)) errors.push("Remote URL must use HTTP or HTTPS.");
      } catch {
        errors.push("Remote URL is invalid.");
      }
    }

    const parsedArgsResult = draft.transport === "stdio" ? parseArgs(draft.argsText) : { parsedArgs: [], errors: [] };
    const parsedEnvResult = parseEnv(draft.envText, draft.transport !== "stdio");
    errors.push(...parsedArgsResult.errors, ...parsedEnvResult.errors);

    if (draft.transport === "stdio" && parsedArgsResult.parsedArgs.length === 0) {
      warnings.push("No arguments provided. Add at least the MCP package or entrypoint if required.");
    }

    return {
      isValid: errors.length === 0,
      errors,
      warnings,
      parsedArgs: parsedArgsResult.parsedArgs,
      parsedEnv: parsedEnvResult.parsedEnv,
    };
  }, [draft.argsText, draft.command, draft.envText, draft.name, draft.transport]);
}
