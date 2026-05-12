import type { McpServerCardServer } from "../../components/McpServerCard";

export type McpServer = McpServerCardServer;

export interface HealthCheckResult {
  server_id: string;
  server_name: string;
  status: string;
  command_exists: boolean;
  can_start: boolean;
  error_message: string | null;
  latency_ms: number | null;
  checked_at: string;
}

export interface RuntimeDepStatus {
  name: string;
  display_name: string;
  installed: boolean;
  version: string | null;
}

export interface WizardPreset {
  id: string;
  labelZh: string;
  labelEn: string;
  command: string;
  args: string[];
}

export const WIZARD_PRESETS: WizardPreset[] = [
  {
    id: "npx",
    labelZh: "Node / npx",
    labelEn: "Node / npx",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-example"],
  },
  { id: "uvx", labelZh: "Python / uvx", labelEn: "Python / uvx", command: "uvx", args: ["mcp-server-example"] },
  {
    id: "docker",
    labelZh: "Docker",
    labelEn: "Docker",
    command: "docker",
    args: ["run", "--rm", "mcp/server-example"],
  },
  { id: "node", labelZh: "本地脚本", labelEn: "Local Script", command: "node", args: ["/path/to/server.js"] },
];

export function formatJson(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}
