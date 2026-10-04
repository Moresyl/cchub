import { invoke } from "@tauri-apps/api/core";
import { readMcpStatuses } from "../../lib/mcpCatalog";
import type { InstalledMcpServer, RegistryEntry } from "./helpers";

export async function resolveInstalledMcp(
  entry: RegistryEntry,
  tool: string,
  servers: InstalledMcpServer[],
): Promise<InstalledMcpServer> {
  const candidates = servers.filter(
    (server) => server.origin && (server.id === entry.id || server.name === entry.name),
  );
  const ids = candidates.map((server) => server.id);
  if (!ids.length) throw new Error("Installed MCP source not found");
  const statuses = readMcpStatuses(await invoke("get_mcp_sync_statuses", { serverIds: ids }), ids);
  const matches = candidates.filter((server) => ["source", "linked"].includes(statuses[server.id][tool]?.state));
  if (matches.length !== 1) throw new Error("Select an unambiguous MCP source in MCP services before continuing");
  return matches[0];
}
