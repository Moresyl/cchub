export interface McpOrigin {
  native_name: string;
  revision: string;
  bindings: Array<{ tool: string; path: string; role: string }>;
}
export interface McpToolStatus {
  state: "source" | "linked" | "unowned" | "missing" | "conflict";
  disabled: boolean;
}
export type McpToolStatuses = Record<string, McpToolStatus>;

export function readMcpStatuses(value: unknown, ids: string[]): Record<string, McpToolStatuses> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid MCP status result");
  const result = value as Record<string, McpToolStatuses>;
  for (const id of ids) {
    const tools = result[id];
    if (!tools || typeof tools !== "object" || Array.isArray(tools)) throw new Error("Missing MCP status result");
    for (const status of Object.values(tools)) {
      if (
        !status ||
        !["source", "linked", "unowned", "missing", "conflict"].includes(status.state) ||
        typeof status.disabled !== "boolean"
      )
        throw new Error("Invalid MCP tool status");
    }
  }
  return result;
}

export function knownMcpStates(states: McpToolStatuses): Record<string, boolean> {
  return Object.fromEntries(
    Object.entries(states).flatMap<[string, boolean]>(([tool, status]) => {
      if (status.state === "source" || status.state === "linked") return [[tool, true]];
      if (status.state === "missing") return [[tool, false]];
      return [];
    }),
  );
}
