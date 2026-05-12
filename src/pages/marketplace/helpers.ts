/* eslint-disable @typescript-eslint/no-explicit-any */
import { Monitor, Sparkles, Globe, Terminal } from "lucide-react";
import type { MarketplaceMcpCardEntry } from "../../components/MarketplaceMcpCard";
import type { MarketplaceSkillCardEntry } from "../../components/MarketplaceSkillCard";

export type RegistryEntry = MarketplaceMcpCardEntry;
export type SkillEntry = MarketplaceSkillCardEntry;

export const TOOL_ICONS: Record<string, typeof Monitor> = {
  claude: Terminal,
  codex: Monitor,
  gemini: Sparkles,
  opencode: Globe,
  hermes: Monitor,
};

// scan_mcp_servers / scan_skills 按工具返回每条记录；同名 skill/server 装到
// 多个工具时会出现多条。列表视图按 name.toLowerCase() 去重，per-tool 的
// "已安装"状态另由 installedIdsByTool / installedSkillsByTool 驱动。
export function dedupByName<T extends { name: string }>(arr: T[]): T[] {
  const seen = new Set<string>();
  const out: T[] = [];
  for (const item of arr) {
    const key = item.name.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(item);
  }
  return out;
}

export interface InstalledMcpServer {
  id: string;
  name: string;
  command: string | null;
  args: string;
  env: string;
  status: string;
  transport: string;
  source: string;
  package_name: string | null;
  version: string | null;
  config_path: string | null;
}

export interface InstalledSkillRecord {
  id: string;
  name: string;
  description: string | null;
  tool_id: string | null;
  plugin_id: string | null;
  trigger_command: string | null;
  file_path: string | null;
}

export const MCP_CATEGORY_ZH: Record<string, string> = {
  local: "本地",
  search: "搜索",
  database: "数据库",
  ai: "AI",
  "dev-tools": "开发工具",
  browser: "浏览器",
  filesystem: "文件系统",
  cloud: "云服务",
  productivity: "效率",
  npm: "npm",
  "official-plugin": "官方插件",
  "community-plugin": "社区插件",
};
