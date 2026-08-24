import {
  Activity,
  ArrowRightLeft,
  BarChart3,
  Bot,
  Brain,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  LayoutDashboard,
  Layers,
  MessageSquareText,
  Monitor,
  Plug,
  Settings,
  Shield,
  Store,
  Terminal,
  Webhook,
  Wrench,
  Zap,
  type LucideIcon,
} from "lucide-react";
import type { RoutePath } from "./routes";

export type NavigationSectionKey = "overview" | "ecosystem" | "automation" | "advanced" | "settings";

export interface NavigationItem {
  path: RoutePath;
  labelKey:
    | "dashboard"
    | "usage"
    | "sessions"
    | "logs"
    | "mcpServers"
    | "mcpClients"
    | "skills"
    | "marketplace"
    | "workspaces"
    | "profiles"
    | "prompts"
    | "workflows"
    | "autopilot"
    | "hooks"
    | "claudeMd"
    | "configFiles"
    | "tools"
    | "security"
    | "hermesMemory"
    | "hermesProviders"
    | "openClaw"
    | "proxyAdvanced"
    | "settings";
  icon: LucideIcon;
}

export interface NavigationSection {
  key: NavigationSectionKey;
  icon: LucideIcon;
  defaultPath: RoutePath;
  items: readonly NavigationItem[];
}

export const navigationSections: readonly NavigationSection[] = [
  {
    key: "overview",
    icon: LayoutDashboard,
    defaultPath: "/",
    items: [
      { path: "/", labelKey: "dashboard", icon: LayoutDashboard },
      { path: "/usage", labelKey: "usage", icon: BarChart3 },
      { path: "/sessions", labelKey: "sessions", icon: History },
      { path: "/logs", labelKey: "logs", icon: Activity },
    ],
  },
  {
    key: "ecosystem",
    icon: Plug,
    defaultPath: "/profiles",
    items: [
      { path: "/profiles", labelKey: "profiles", icon: ArrowRightLeft },
      { path: "/mcp-servers", labelKey: "mcpServers", icon: Plug },
      { path: "/mcp-clients", labelKey: "mcpClients", icon: Monitor },
      { path: "/skills", labelKey: "skills", icon: Zap },
      { path: "/marketplace", labelKey: "marketplace", icon: Store },
      { path: "/workspaces", labelKey: "workspaces", icon: Layers },
    ],
  },
  {
    key: "automation",
    icon: GitBranch,
    defaultPath: "/prompts",
    items: [
      { path: "/prompts", labelKey: "prompts", icon: MessageSquareText },
      { path: "/workflows", labelKey: "workflows", icon: GitBranch },
      { path: "/autopilot", labelKey: "autopilot", icon: Bot },
      { path: "/hooks", labelKey: "hooks", icon: Webhook },
    ],
  },
  {
    key: "advanced",
    icon: Wrench,
    defaultPath: "/tools",
    items: [
      { path: "/tools", labelKey: "tools", icon: Wrench },
      { path: "/config-files", labelKey: "configFiles", icon: FolderOpen },
      { path: "/claude-md", labelKey: "claudeMd", icon: FileText },
      { path: "/security", labelKey: "security", icon: Shield },
      { path: "/proxy-advanced", labelKey: "proxyAdvanced", icon: ArrowRightLeft },
      { path: "/hermes-memory", labelKey: "hermesMemory", icon: Brain },
      { path: "/hermes-providers", labelKey: "hermesProviders", icon: Brain },
      { path: "/openclaw", labelKey: "openClaw", icon: Terminal },
    ],
  },
  {
    key: "settings",
    icon: Settings,
    defaultPath: "/settings",
    items: [{ path: "/settings", labelKey: "settings", icon: Settings }],
  },
] as const;

export function getNavigationSection(pathname: string): NavigationSection {
  return (
    navigationSections.find((section) => section.items.some((item) => item.path === pathname)) ?? navigationSections[0]
  );
}
